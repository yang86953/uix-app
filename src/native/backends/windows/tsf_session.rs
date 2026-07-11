//! Windows TSF session — `ITfThreadMgr` + `AssociateFocus`（P6 phonetic 前置）。
//!
//! 现代 TIP（微软拼音等）优先走 TSF；将 HWND 与 DocumentMgr 关联后，IMM32
//! `WM_IME_*` 桥接更可靠。完整 `ITfTextEditSink` / `ITextStoreACP` 仍待后续切片；
//! composition → `UiEvent` 辅助见 [`tsf_composition_events`]。

#![cfg(windows)]

use std::collections::VecDeque;

use windows::core::{Interface, Type};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::TextServices::{
    CLSID_TF_ThreadMgr, ITfContext, ITfDocumentMgr, ITfThreadMgr, TF_POPF_ALL,
};

use crate::core::{Errc, Error, Result};
use crate::native::backends::windows::util::windows_diag;
use crate::native::shared::ime_events::{
    on_committed_text, on_marked_text, on_unmark_text, ImeCompositionState,
};
use crate::native::traits::event::UiEvent;

/// TSF 线程/文档焦点会话（与 IMM32 `ITextInput` 并存）。
pub(crate) struct TsfSession {
    thread_mgr: ITfThreadMgr,
    doc_mgr: ITfDocumentMgr,
    hwnd: HWND,
    client_id: u32,
}

impl TsfSession {
    pub(crate) fn client_id(&self) -> u32 {
        self.client_id
    }

    pub(crate) fn activate(hwnd: *mut std::ffi::c_void) -> Result<Self> {
        if hwnd.is_null() {
            return Err(Error::new(Errc::InvalidArgument, "TSF: null HWND"));
        }
        ensure_com_apartment();

        // SAFETY: CLSID_TF_ThreadMgr is a system in-proc COM class.
        let thread_mgr: ITfThreadMgr = unsafe {
            CoCreateInstance(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER).map_err(|err| {
                windows_diag(
                    Errc::PlatformError,
                    &format!("TSF: CoCreateInstance(ThreadMgr) failed: {err}"),
                )
            })?
        };

        let client_id = unsafe { thread_mgr.Activate() }.map_err(|err| {
            windows_diag(
                Errc::PlatformError,
                &format!("TSF: ThreadMgr::Activate failed: {err}"),
            )
        })?;

        let doc_mgr = unsafe { thread_mgr.CreateDocumentMgr() }.map_err(|err| {
            let _ = unsafe { thread_mgr.Deactivate() };
            windows_diag(
                Errc::PlatformError,
                &format!("TSF: CreateDocumentMgr failed: {err}"),
            )
        })?;

        let mut context: Option<ITfContext> = None;
        let mut edit_cookie = 0u32;
        // 空 context（无 ITextStoreACP）：先完成 focus 关联；sink/store 后续切片补齐。
        if let Err(err) = unsafe {
            doc_mgr.CreateContext(client_id, 0, None, &mut context, &mut edit_cookie)
        } {
            let _ = unsafe { thread_mgr.Deactivate() };
            return Err(windows_diag(
                Errc::PlatformError,
                &format!("TSF: CreateContext failed: {err}"),
            ));
        }
        let Some(context) = context else {
            let _ = unsafe { thread_mgr.Deactivate() };
            return Err(Error::new(
                Errc::PlatformError,
                "TSF: CreateContext returned null context",
            ));
        };
        if let Err(err) = unsafe { doc_mgr.Push(&context) } {
            let _ = unsafe { thread_mgr.Deactivate() };
            return Err(windows_diag(
                Errc::PlatformError,
                &format!("TSF: DocumentMgr::Push failed: {err}"),
            ));
        }

        let hwnd = HWND(hwnd);
        if let Err(err) = associate_focus(&thread_mgr, hwnd, Some(&doc_mgr)) {
            let _ = unsafe { doc_mgr.Pop(TF_POPF_ALL) };
            let _ = unsafe { thread_mgr.Deactivate() };
            return Err(err);
        }
        if let Err(err) = unsafe { thread_mgr.SetFocus(&doc_mgr) } {
            let _ = associate_focus(&thread_mgr, hwnd, None);
            let _ = unsafe { doc_mgr.Pop(TF_POPF_ALL) };
            let _ = unsafe { thread_mgr.Deactivate() };
            return Err(windows_diag(
                Errc::PlatformError,
                &format!("TSF: SetFocus failed: {err}"),
            ));
        }

        Ok(Self {
            thread_mgr,
            doc_mgr,
            hwnd,
            client_id,
        })
    }

    pub(crate) fn deactivate(self) {
        let _ = associate_focus(&self.thread_mgr, self.hwnd, None);
        let _ = unsafe { self.doc_mgr.Pop(TF_POPF_ALL) };
        let _ = unsafe { self.thread_mgr.Deactivate() };
    }
}

/// `AssociateFocus` 在无先前关联时返回 null prev；windows-rs `from_abi(null)` 会误报失败。
fn associate_focus(
    thread_mgr: &ITfThreadMgr,
    hwnd: HWND,
    doc_mgr: Option<&ITfDocumentMgr>,
) -> Result<()> {
    unsafe {
        let mut prev = std::ptr::null_mut();
        let new_abi = match doc_mgr {
            Some(doc) => Interface::as_raw(doc),
            None => std::ptr::null_mut(),
        };
        (Interface::vtable(thread_mgr).AssociateFocus)(
            Interface::as_raw(thread_mgr),
            hwnd,
            new_abi,
            &mut prev,
        )
        .ok()
        .map_err(|err| {
            windows_diag(
                Errc::PlatformError,
                &format!("TSF: AssociateFocus failed: {err}"),
            )
        })?;
        if !prev.is_null() {
            // 释放先前 DocumentMgr（若有）。
            let _: ITfDocumentMgr = Type::from_abi(prev).map_err(|err| {
                windows_diag(
                    Errc::PlatformError,
                    &format!("TSF: previous DocumentMgr abi failed: {err}"),
                )
            })?;
        }
        Ok(())
    }
}

fn ensure_com_apartment() {
    // S_OK / S_FALSE → Ok；已是 MTA 等 → Err，忽略（ThreadMgr 仍可能可用）。
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
}

/// TSF sink / TIP 回调侧：把 marked / committed / unmark 收成 `UiEvent` 队列。
///
/// 与 macOS `ime_events` 同形，供未来 `ITfTextEditSink` 复用（[#75](docs/决策.md#d75)）。
pub(crate) fn tsf_composition_events(
    state: &mut ImeCompositionState,
    marked: Option<&str>,
    committed: Option<&str>,
    unmark: bool,
) -> Vec<UiEvent> {
    let queue = std::sync::Arc::new(std::sync::Mutex::new(VecDeque::new()));
    if let Some(text) = marked {
        on_marked_text(&queue, state, text);
    }
    if let Some(text) = committed {
        on_committed_text(&queue, state, text);
    }
    if unmark {
        on_unmark_text(&queue, state);
    }
    queue.lock().map(|mut q| q.drain(..).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::traits::event::UiEventType;

    #[test]
    fn tsf_composition_helpers_match_ime_events_shape() {
        let mut state = ImeCompositionState::default();
        let events = tsf_composition_events(&mut state, Some("zh"), None, false);
        assert!(state.active);
        assert_eq!(events[0].type_, UiEventType::ImeCompositionStart);
        assert_eq!(events[1].type_, UiEventType::ImeCompositionUpdate);

        let events = tsf_composition_events(&mut state, None, Some("中"), false);
        assert!(!state.active);
        assert_eq!(events[0].type_, UiEventType::ImeCompositionEnd);
        assert_eq!(events[1].type_, UiEventType::TextInput);
    }

    #[test]
    fn activate_rejects_null_hwnd() {
        match TsfSession::activate(std::ptr::null_mut()) {
            Err(err) => assert_eq!(err.code(), Errc::InvalidArgument),
            Ok(_) => panic!("null hwnd must fail"),
        }
    }

    #[test]
    fn activate_on_real_window_associates_focus() {
        if std::env::consts::OS != "windows" {
            return;
        }
        let mut platform = crate::native::create_platform().expect("platform");
        let mut window = platform
            .window_manager()
            .create_window("TSF focus test", 320, 240)
            .expect("window");
        let hwnd = window.native_surface_ptr();
        let session = TsfSession::activate(hwnd).expect("TSF activate");
        assert_ne!(session.client_id(), 0, "Activate must assign a client id");
        session.deactivate();
        window.close().expect("close");
    }
}

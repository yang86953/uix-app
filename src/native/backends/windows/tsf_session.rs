//! Windows TSF session — ThreadMgr + DocumentMgr + `ITextStoreACP`（P6 phonetic）。
//!
//! `AssociateFocus` 将 HWND 交给 TIP；composition sink 经共享队列投递 `UiEvent`。

#![cfg(windows)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use windows::core::{Interface, Type};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::TextServices::{
    CLSID_TF_ThreadMgr, ITfContext, ITfDocumentMgr, ITfThreadMgr, TF_POPF_ALL,
};

use crate::core::{Errc, Error, Result, WindowId};
use crate::native::backends::windows::tsf_text_store::{TsfEventSink, TsfStoreState, TsfTextStore};
use crate::native::backends::windows::util::windows_diag;
use crate::native::shared::ime_events::{
    on_committed_text, on_marked_text, on_unmark_text, ImeCompositionState,
};
use crate::native::traits::event::UiEvent;

/// 激活参数：共享事件队列由 `WindowsPlatform` 持有并在 `next_event` 排空。
pub(crate) struct TsfActivateParams {
    pub hwnd: *mut std::ffi::c_void,
    pub window_id: WindowId,
    pub events: Arc<Mutex<VecDeque<UiEvent>>>,
}

/// TSF 线程/文档焦点会话（与 IMM32 `ITextInput` 并存）。
pub(crate) struct TsfSession {
    thread_mgr: ITfThreadMgr,
    doc_mgr: ITfDocumentMgr,
    _context: ITfContext,
    _text_store: windows::core::ComObject<TsfTextStore>,
    store_state: Arc<Mutex<TsfStoreState>>,
    hwnd: HWND,
    client_id: u32,
}

impl TsfSession {
    pub(crate) fn client_id(&self) -> u32 {
        self.client_id
    }

    pub(crate) fn composition_active(&self) -> bool {
        self.store_state
            .lock()
            .map(|s| s.composition_active())
            .unwrap_or(false)
    }

    pub(crate) fn set_cursor_rect(&self, rect: windows::Win32::Foundation::RECT) {
        if let Ok(mut state) = self.store_state.lock() {
            state.set_cursor_rect(rect);
        }
    }

    pub(crate) fn activate(params: TsfActivateParams) -> Result<Self> {
        if params.hwnd.is_null() {
            return Err(Error::new(Errc::InvalidArgument, "TSF: null HWND"));
        }
        ensure_com_apartment();

        let hwnd = HWND(params.hwnd);
        let event_sink = TsfEventSink {
            events: params.events,
            window_id: params.window_id,
            hwnd,
        };
        let (text_store, store_state) = TsfTextStore::create(event_sink);
        let punk: windows::core::IUnknown = text_store.to_interface();

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
        if let Err(err) =
            unsafe { doc_mgr.CreateContext(client_id, 0, &punk, &mut context, &mut edit_cookie) }
        {
            let _ = unsafe { thread_mgr.Deactivate() };
            return Err(windows_diag(
                Errc::PlatformError,
                &format!("TSF: CreateContext(store) failed: {err}"),
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
            _context: context,
            _text_store: text_store,
            store_state,
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
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
}

/// TSF sink / TIP 回调侧：把 marked / committed / unmark 收成 `UiEvent` 队列。
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
    queue
        .lock()
        .map(|mut q| q.drain(..).collect())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "../../../tests/native/backends/windows/tsf_session.rs"]
mod tests;

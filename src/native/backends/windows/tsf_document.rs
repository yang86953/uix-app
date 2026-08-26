//! TSF 文档缓冲与逐窗事件出口。

#![cfg(windows)]

use std::collections::VecDeque;
use std::ops::Range;
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::TextServices::{
    ITextStoreACPSink, TEXT_STORE_LOCK_FLAGS, TS_AE_END, TS_E_INVALIDPOS, TS_LF_READ,
    TS_LF_READWRITE, TS_LF_SYNC, TS_SELECTION_ACP, TS_SELECTIONSTYLE, TS_TEXTCHANGE,
};
use windows::core::{Error as WinError, Result as WinResult};

use crate::core::{Errc, Error, WindowId};
use crate::diagnostics::PendingFailureSource;
use crate::native::windowing::shared::ime_events::ImeCompositionState;
use crate::platform::windowing::event::UiEvent;

#[derive(Clone)]
pub(crate) struct TsfEventSink {
    pub events: Arc<Mutex<VecDeque<UiEvent>>>,
    pub window_id: WindowId,
    pub hwnd: HWND,
    pub pending_failures: PendingFailureSource,
}

impl TsfEventSink {
    pub(crate) fn enqueue_failure(&self, error: Error) {
        let _ = self.pending_failures.enqueue(error);
    }

    pub(crate) fn enqueue_windows_failure(&self, context: &str, error: WinError) {
        self.enqueue_failure(Error::new(
            Errc::PlatformError,
            format!("{context}: {error}"),
        ));
    }

    pub(crate) fn push(&self, events: Vec<UiEvent>) {
        if events.is_empty() {
            return;
        }
        let mut queue = self
            .events
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        for mut event in events {
            event.window_id = Some(self.window_id);
            queue.push_back(event);
        }
        drop(queue);
        // 与 EventLoopWaker 同形；仅唤醒拥有该 HWND 的消息循环。
        // SAFETY：hwnd 由活动文本输入会话持有，在本调用期间未被销毁；消息只携带
        // 0 值参数（无指针载荷），PostMessageW 异步投递，返回后无需保持窗口存活。
        if let Err(error) = unsafe {
            windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                Some(self.hwnd),
                windows::Win32::UI::WindowsAndMessaging::WM_NULL,
                windows::Win32::Foundation::WPARAM(0),
                windows::Win32::Foundation::LPARAM(0),
            )
        } {
            self.enqueue_windows_failure("TSF: PostMessageW(WM_NULL) failed", error);
        }
    }
}

pub(crate) struct TsfStoreState {
    pub(super) text: Vec<u16>,
    pub(super) sel_start: i32,
    pub(crate) sel_end: i32,
    document_lock: TsfDocumentLock,
    pub(super) acp_sink: Option<ITextStoreACPSink>,
    pub(super) sink_mask: u32,
    pub(super) composition: ImeCompositionState,
    pub(super) cursor: RECT,
    pub(super) event_sink: TsfEventSink,
}

impl TsfStoreState {
    pub(crate) fn new(event_sink: TsfEventSink) -> Self {
        Self {
            text: Vec::new(),
            sel_start: 0,
            sel_end: 0,
            document_lock: TsfDocumentLock::default(),
            acp_sink: None,
            sink_mask: 0,
            composition: ImeCompositionState::default(),
            cursor: RECT {
                left: 0,
                top: 0,
                right: 1,
                bottom: 16,
            },
            event_sink,
        }
    }

    pub(crate) fn set_cursor_rect(&mut self, rect: RECT) {
        self.cursor = rect;
    }

    pub(crate) fn cursor_screen_rect(&self) -> WinResult<RECT> {
        self.client_rect_to_screen(self.cursor)
    }

    pub(crate) fn screen_extent(&self) -> WinResult<RECT> {
        let hwnd = self.event_sink.hwnd;
        // 最小化窗口没有可呈现的文本表面，向 TIP 返回空范围。
        // SAFETY：hwnd 由活动文本输入会话持有；IsIconic 只读查询窗口状态，不写内存。
        if unsafe { windows::Win32::UI::WindowsAndMessaging::IsIconic(hwnd) }.as_bool() {
            return Ok(RECT::default());
        }
        let mut rect = RECT::default();
        // SAFETY: rect 在调用期间有效，hwnd 由活动文本输入会话持有。
        unsafe { windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect)? };
        self.client_rect_to_screen(rect)
    }

    fn client_rect_to_screen(&self, rect: RECT) -> WinResult<RECT> {
        let mut top_left = windows::Win32::Foundation::POINT {
            x: rect.left,
            y: rect.top,
        };
        let mut bottom_right = windows::Win32::Foundation::POINT {
            x: rect.right,
            y: rect.bottom,
        };
        // SAFETY: 两个 POINT 在调用期间有效，hwnd 由活动文本输入会话持有。
        let converted = unsafe {
            ClientToScreen(self.event_sink.hwnd, &mut top_left).as_bool()
                && ClientToScreen(self.event_sink.hwnd, &mut bottom_right).as_bool()
        };
        if !converted {
            return Err(WinError::from_thread());
        }
        Ok(RECT {
            left: top_left.x,
            top: top_left.y,
            right: bottom_right.x,
            bottom: bottom_right.y,
        })
    }

    // 测试目标保留 composition 状态观测入口，供 TSF 文档契约测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn composition_active(&self) -> bool {
        self.composition.active
    }

    pub(crate) fn begin_lock(&mut self, flags: u32) -> TsfLockRequest {
        self.document_lock.begin(flags)
    }

    pub(crate) fn complete_lock(&mut self) -> Option<TsfLockKind> {
        self.document_lock.complete()
    }

    pub(crate) fn has_read_lock(&self) -> bool {
        self.document_lock.has_read()
    }

    pub(crate) fn has_write_lock(&self) -> bool {
        self.document_lock.has_write()
    }

    pub(super) fn end_acp(&self) -> i32 {
        self.text.len() as i32
    }

    pub(super) fn contains_range(&self, start: i32, end: i32) -> bool {
        start >= 0 && start <= end && end <= self.end_acp()
    }

    pub(super) fn text_range(&self, start: i32, end: i32) -> WinResult<Range<usize>> {
        if !self.contains_range(start, end) {
            return Err(WinError::from(TS_E_INVALIDPOS));
        }
        Ok(start as usize..end as usize)
    }

    pub(super) fn selection(&self) -> TS_SELECTION_ACP {
        TS_SELECTION_ACP {
            acpStart: self.sel_start,
            acpEnd: self.sel_end,
            style: TS_SELECTIONSTYLE {
                ase: TS_AE_END,
                fInterimChar: false.into(),
            },
        }
    }

    pub(crate) fn replace_range(
        &mut self,
        start: i32,
        end: i32,
        insert: &[u16],
    ) -> WinResult<TS_TEXTCHANGE> {
        let range = self.text_range(start, end)?;
        self.text.splice(range.clone(), insert.iter().copied());
        let new_end = (range.start + insert.len()) as i32;
        self.sel_start = new_end;
        self.sel_end = new_end;
        Ok(TS_TEXTCHANGE {
            acpStart: range.start as i32,
            acpOldEnd: range.end as i32,
            acpNewEnd: new_end,
        })
    }

    pub(crate) fn utf16_string(&self) -> String {
        String::from_utf16_lossy(&self.text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TsfLockKind {
    Read,
    ReadWrite,
}

impl TsfLockKind {
    pub(crate) const fn flags(self) -> TEXT_STORE_LOCK_FLAGS {
        match self {
            Self::Read => TS_LF_READ,
            Self::ReadWrite => TS_LF_READWRITE,
        }
    }

    fn from_request(flags: u32) -> Self {
        if flags & TS_LF_READWRITE.0 == TS_LF_READWRITE.0 {
            Self::ReadWrite
        } else {
            Self::Read
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TsfLockRequest {
    Grant(TsfLockKind),
    PendingWrite,
    RejectSynchronous,
}

#[derive(Debug, Default)]
struct TsfDocumentLock {
    granted: Option<TsfLockKind>,
    pending_write: bool,
}

impl TsfDocumentLock {
    fn begin(&mut self, flags: u32) -> TsfLockRequest {
        let requested = TsfLockKind::from_request(flags);
        let Some(granted) = self.granted else {
            self.granted = Some(requested);
            return TsfLockRequest::Grant(requested);
        };

        let asynchronous_write_upgrade = granted == TsfLockKind::Read
            && requested == TsfLockKind::ReadWrite
            && flags & TS_LF_SYNC == 0;
        if asynchronous_write_upgrade {
            self.pending_write = true;
            TsfLockRequest::PendingWrite
        } else {
            TsfLockRequest::RejectSynchronous
        }
    }

    fn complete(&mut self) -> Option<TsfLockKind> {
        self.granted = None;
        if std::mem::take(&mut self.pending_write) {
            self.granted = Some(TsfLockKind::ReadWrite);
            Some(TsfLockKind::ReadWrite)
        } else {
            None
        }
    }

    fn has_read(&self) -> bool {
        self.granted.is_some()
    }

    fn has_write(&self) -> bool {
        self.granted == Some(TsfLockKind::ReadWrite)
    }
}

#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/native/backends/windows/tsf_document__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;

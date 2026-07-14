//! TSF 文档缓冲与逐窗事件出口。

#![cfg(windows)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::TextServices::{
    ITextStoreACPSink, TS_AE_END, TS_SELECTIONSTYLE, TS_SELECTION_ACP, TS_TEXTCHANGE,
};

use crate::core::WindowId;
use crate::native::shared::ime_events::ImeCompositionState;
use crate::native::traits::event::UiEvent;

#[derive(Clone)]
pub(crate) struct TsfEventSink {
    pub events: Arc<Mutex<VecDeque<UiEvent>>>,
    pub window_id: WindowId,
    pub hwnd: HWND,
}

impl TsfEventSink {
    pub(super) fn push(&self, events: Vec<UiEvent>) {
        if events.is_empty() {
            return;
        }
        if let Ok(mut queue) = self.events.lock() {
            for mut event in events {
                event.window_id = Some(self.window_id);
                queue.push_back(event);
            }
        }
        // 与 EventLoopWaker 同形；仅唤醒拥有该 HWND 的消息循环。
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                Some(self.hwnd),
                windows::Win32::UI::WindowsAndMessaging::WM_NULL,
                windows::Win32::Foundation::WPARAM(0),
                windows::Win32::Foundation::LPARAM(0),
            );
        }
    }
}

pub(crate) struct TsfStoreState {
    pub(super) text: Vec<u16>,
    pub(super) sel_start: i32,
    pub(crate) sel_end: i32,
    pub(super) locked: bool,
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
            locked: false,
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

    pub(crate) fn composition_active(&self) -> bool {
        self.composition.active
    }

    pub(super) fn end_acp(&self) -> i32 {
        self.text.len() as i32
    }

    pub(super) fn clamp_acp(&self, acp: i32) -> i32 {
        acp.clamp(0, self.end_acp())
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

    pub(crate) fn replace_range(&mut self, start: i32, end: i32, insert: &[u16]) -> TS_TEXTCHANGE {
        let start = self.clamp_acp(start) as usize;
        let end = self.clamp_acp(end) as usize;
        let end = end.max(start);
        self.text.splice(start..end, insert.iter().copied());
        let new_end = (start + insert.len()) as i32;
        self.sel_start = new_end;
        self.sel_end = new_end;
        TS_TEXTCHANGE {
            acpStart: start as i32,
            acpOldEnd: end as i32,
            acpNewEnd: new_end,
        }
    }

    pub(crate) fn utf16_string(&self) -> String {
        String::from_utf16_lossy(&self.text)
    }
}

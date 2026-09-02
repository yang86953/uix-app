//! IMM32 composition → `UiEvent` 纯分发（可单测；HWND/HIMC 读取留在 `wnd_proc`）。
//!
//! TSF phonetic sink 落地后复用同一事件形状（[产品](docs/产品.md) · P6）。

#![cfg(windows)]

use crate::platform::windowing::event::UiEvent;

use super::text_input::WindowsImeState;

/// 已读到的 IMM32 字符串（相对某一 GCS_* 标志位）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImmStringRead {
    /// 对应标志位未置位，或读取失败（调用方已记日志）。
    Skipped,
    /// 标志位置位，但 `ImmGetCompositionStringW` 无数据。
    NoData,
    /// 标志位置位且读到字符串（可为空）。
    Text(String),
}

impl ImmStringRead {
    pub(crate) fn from_flagged_result(
        flag_set: bool,
        read: crate::core::Result<Option<String>>,
    ) -> Self {
        if !flag_set {
            return Self::Skipped;
        }
        match read {
            Ok(None) => Self::NoData,
            Ok(Some(text)) => Self::Text(text),
            // 注册表读取失败按无数据处理（Skipped）：下一次消息循环会重试，
            // 不把瞬态读取错误升级为输入失败。
            Err(_) => Self::Skipped,
        }
    }
}

/// `WM_IME_STARTCOMPOSITION`：仅在首次进入 composition 时发出 Start。
pub(crate) fn ime_start_composition_event(state: &mut WindowsImeState) -> Option<UiEvent> {
    state
        .begin_composition()
        .then(UiEvent::ime_composition_start)
}

/// `WM_IME_ENDCOMPOSITION` / kill-focus：仅在仍有 active composition 时发出 End。
pub(crate) fn ime_end_composition_event(state: &mut WindowsImeState) -> Option<UiEvent> {
    state
        .end_composition()
        .then(|| UiEvent::ime_composition_end(""))
}

/// `WM_IME_COMPOSITION`：先 RESULTSTR，再 COMPSTR（与现网 `wnd_proc` 顺序一致）。
///
/// 返回 `(handled, events)`：`handled == false` 时调用方应 `DefWindowProc`。
pub(crate) fn ime_composition_events(
    state: &mut WindowsImeState,
    result: ImmStringRead,
    comp: ImmStringRead,
) -> (bool, Vec<UiEvent>) {
    let mut events = Vec::new();
    let mut handled = false;

    match result {
        ImmStringRead::Skipped | ImmStringRead::NoData => {}
        ImmStringRead::Text(text) => {
            let was_active = state.end_composition();
            if was_active {
                events.push(UiEvent::ime_composition_end(text.clone()));
            }
            if !text.is_empty() {
                events.push(UiEvent::text_input(text));
            }
            handled = true;
        }
    }

    match comp {
        ImmStringRead::Skipped | ImmStringRead::NoData => {}
        ImmStringRead::Text(text) => {
            let should_update = !text.is_empty() || state.composition_active();
            let started = !text.is_empty() && state.begin_composition();
            if started {
                events.push(UiEvent::ime_composition_start());
            }
            if should_update {
                events.push(UiEvent::ime_composition_update(text));
            }
            handled = true;
        }
    }

    (handled, events)
}

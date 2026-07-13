//! IMM32 composition → `UiEvent` 纯分发（可单测；HWND/HIMC 读取留在 `wnd_proc`）。
//!
//! TSF phonetic sink 落地后复用同一事件形状（[产品](docs/产品.md) · P6）。

#![cfg(windows)]

use crate::native::traits::event::{UiEvent, UiEventType};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::traits::event::UiEventPayload;

    fn payload_text(event: &UiEvent) -> Option<&str> {
        match &event.payload {
            UiEventPayload::ImeComposition(data) => Some(data.text.as_str()),
            UiEventPayload::TextInput(data) => Some(data.text.as_str()),
            _ => None,
        }
    }

    #[test]
    fn compstr_emits_start_and_update() {
        let mut state = WindowsImeState::default();
        let (handled, events) = ime_composition_events(
            &mut state,
            ImmStringRead::Skipped,
            ImmStringRead::Text("zh".into()),
        );
        assert!(handled);
        assert!(state.composition_active());
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].type_, UiEventType::ImeCompositionStart);
        assert_eq!(events[1].type_, UiEventType::ImeCompositionUpdate);
        assert_eq!(payload_text(&events[1]), Some("zh"));
    }

    #[test]
    fn resultstr_emits_end_and_text_input() {
        let mut state = WindowsImeState::default();
        assert!(state.begin_composition());
        let (handled, events) = ime_composition_events(
            &mut state,
            ImmStringRead::Text("中".into()),
            ImmStringRead::Skipped,
        );
        assert!(handled);
        assert!(!state.composition_active());
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].type_, UiEventType::ImeCompositionEnd);
        assert_eq!(payload_text(&events[0]), Some("中"));
        assert_eq!(events[1].type_, UiEventType::TextInput);
        assert_eq!(payload_text(&events[1]), Some("中"));
    }

    #[test]
    fn result_before_comp_in_same_message() {
        let mut state = WindowsImeState::default();
        assert!(state.begin_composition());
        let (handled, events) = ime_composition_events(
            &mut state,
            ImmStringRead::Text("中".into()),
            ImmStringRead::Text("pin".into()),
        );
        assert!(handled);
        assert!(state.composition_active());
        assert_eq!(events[0].type_, UiEventType::ImeCompositionEnd);
        assert_eq!(events[1].type_, UiEventType::TextInput);
        assert_eq!(events[2].type_, UiEventType::ImeCompositionStart);
        assert_eq!(events[3].type_, UiEventType::ImeCompositionUpdate);
        assert_eq!(payload_text(&events[3]), Some("pin"));
    }

    #[test]
    fn empty_comp_while_active_still_updates() {
        let mut state = WindowsImeState::default();
        assert!(state.begin_composition());
        let (handled, events) = ime_composition_events(
            &mut state,
            ImmStringRead::Skipped,
            ImmStringRead::Text(String::new()),
        );
        assert!(handled);
        assert!(state.composition_active());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].type_, UiEventType::ImeCompositionUpdate);
        assert_eq!(payload_text(&events[0]), Some(""));
    }

    #[test]
    fn empty_comp_when_inactive_is_handled_but_silent() {
        let mut state = WindowsImeState::default();
        let (handled, events) = ime_composition_events(
            &mut state,
            ImmStringRead::Skipped,
            ImmStringRead::Text(String::new()),
        );
        assert!(handled);
        assert!(!state.composition_active());
        assert!(events.is_empty());
    }

    #[test]
    fn no_data_reads_do_not_mark_handled() {
        let mut state = WindowsImeState::default();
        let (handled, events) =
            ime_composition_events(&mut state, ImmStringRead::NoData, ImmStringRead::NoData);
        assert!(!handled);
        assert!(events.is_empty());
    }

    #[test]
    fn start_and_end_helpers_are_idempotent() {
        let mut state = WindowsImeState::default();
        assert!(ime_start_composition_event(&mut state).is_some());
        assert!(ime_start_composition_event(&mut state).is_none());
        assert!(ime_end_composition_event(&mut state).is_some());
        assert!(ime_end_composition_event(&mut state).is_none());
    }

    #[test]
    fn from_flagged_result_maps_branches() {
        assert_eq!(
            ImmStringRead::from_flagged_result(false, Ok(Some("x".into()))),
            ImmStringRead::Skipped
        );
        assert_eq!(
            ImmStringRead::from_flagged_result(true, Ok(None)),
            ImmStringRead::NoData
        );
        assert_eq!(
            ImmStringRead::from_flagged_result(true, Ok(Some("x".into()))),
            ImmStringRead::Text("x".into())
        );
        assert_eq!(
            ImmStringRead::from_flagged_result(
                true,
                Err(crate::core::Error::new(
                    crate::core::Errc::PlatformError,
                    "boom",
                ))
            ),
            ImmStringRead::Skipped
        );
    }
}

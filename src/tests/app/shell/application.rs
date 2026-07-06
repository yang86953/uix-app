use super::*;
    use crate::native::traits::event::{
        ClipboardData, ImeCompositionData, LocaleChangeData, ThemeChangeData,
    };

    #[test]
    fn map_theme_changed_event() {
        let event = UiEvent::new(
            UiEventType::ThemeChanged,
            UiEventPayload::ThemeChanged(ThemeChangeData { is_dark: true }),
        );

        assert!(matches!(
            map_ui_event(&event),
            Some(SystemEvent::ThemeChanged { is_dark: true })
        ));
    }

    #[test]
    fn map_locale_changed_event() {
        let event = UiEvent::new(
            UiEventType::LocaleChanged,
            UiEventPayload::LocaleChanged(LocaleChangeData {
                locale: "zh-CN".to_string(),
            }),
        );

        assert!(matches!(
            map_ui_event(&event),
            Some(SystemEvent::LocaleChanged { locale }) if locale == "zh-CN"
        ));
    }

    #[test]
    fn map_clipboard_events() {
        assert!(matches!(
            map_ui_event(&UiEvent::copy()),
            Some(SystemEvent::Copy)
        ));
        assert!(matches!(
            map_ui_event(&UiEvent::cut()),
            Some(SystemEvent::Cut)
        ));

        let paste = UiEvent::new(
            UiEventType::Paste,
            UiEventPayload::Clipboard(ClipboardData {
                text: "hello".to_string(),
            }),
        );
        assert!(matches!(
            map_ui_event(&paste),
            Some(SystemEvent::Paste { text }) if text == "hello"
        ));
    }

    #[test]
    fn map_ime_composition_events() {
        assert!(matches!(
            map_ui_event(&UiEvent::ime_composition_start()),
            Some(SystemEvent::ImeCompositionStart)
        ));

        let update = UiEvent::new(
            UiEventType::ImeCompositionUpdate,
            UiEventPayload::ImeComposition(ImeCompositionData {
                text: "zh".to_string(),
            }),
        );
        assert!(matches!(
            map_ui_event(&update),
            Some(SystemEvent::ImeCompositionUpdate { text }) if text == "zh"
        ));

        let end = UiEvent::new(
            UiEventType::ImeCompositionEnd,
            UiEventPayload::ImeComposition(ImeCompositionData {
                text: "中".to_string(),
            }),
        );
        assert!(matches!(
            map_ui_event(&end),
            Some(SystemEvent::ImeCompositionEnd { text }) if text == "中"
        ));
    }

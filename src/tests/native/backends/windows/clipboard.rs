use crate::native::backends::windows::clipboard::{
    global_memory_text_round_trip, priority_result_has_text,
};

#[test]
fn clipboard_priority_result_requires_an_actual_text_format() {
    assert!(!priority_result_has_text(-1));
    assert!(!priority_result_has_text(0));
    assert!(priority_result_has_text(1));
    assert!(priority_result_has_text(13));
}

#[test]
fn clipboard_global_memory_round_trip_preserves_utf16_text() {
    assert_eq!(global_memory_text_round_trip(""), Some(String::new()));
    assert_eq!(
        global_memory_text_round_trip("UIX 剪贴板 🚀"),
        Some("UIX 剪贴板 🚀".to_string())
    );
}

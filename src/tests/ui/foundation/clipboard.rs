use crate::native::traits::input::IClipboard;
use crate::ui::foundation::clipboard::{
    copy_to_clipboard, read_text_from_clipboard, with_clipboard,
};

#[derive(Default)]
struct TestClipboard(String);

impl IClipboard for TestClipboard {
    fn text(&self) -> String {
        self.0.clone()
    }

    fn set_text(&mut self, text: &str) {
        self.0 = text.to_owned();
    }

    fn has_text(&self) -> bool {
        !self.0.is_empty()
    }
}

#[test]
fn clipboard_service_is_only_available_inside_managed_scope() {
    let mut clipboard = TestClipboard::default();

    copy_to_clipboard("outside");
    assert!(clipboard.0.is_empty());

    with_clipboard(&mut clipboard, || {
        copy_to_clipboard("inside");
        assert_eq!(read_text_from_clipboard().as_deref(), Some("inside"));
    });

    assert_eq!(clipboard.0, "inside");
    assert_eq!(read_text_from_clipboard(), None);
    copy_to_clipboard("after");
    assert_eq!(clipboard.0, "inside");
}

#[test]
fn nested_clipboard_scope_restores_outer_service() {
    let mut outer = TestClipboard("outer".to_owned());
    let mut inner = TestClipboard("inner".to_owned());

    with_clipboard(&mut outer, || {
        assert_eq!(read_text_from_clipboard().as_deref(), Some("outer"));
        with_clipboard(&mut inner, || copy_to_clipboard("nested"));
        copy_to_clipboard("restored");
    });

    assert_eq!(outer.0, "restored");
    assert_eq!(inner.0, "nested");
}

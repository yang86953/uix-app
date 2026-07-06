use super::*;
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::{SemanticKind, WidgetCore, WidgetTree};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn pagination_click_emits_change_semantic_event() {
        let mut tree = WidgetTree::new();
        let id = tree.set_root(Box::new(Pagination::new(85, 10)));
        tree.get_mut(id)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 260.0, 40.0));

        let page = Rc::new(RefCell::new(String::new()));
        let page_for_handler = page.clone();
        tree.handler_table()
            .on(id, SemanticKind::Change, move |event| {
                if let Some(value) = event.text_payload() {
                    *page_for_handler.borrow_mut() = value.to_string();
                }
            });

        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(70.0, 16.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        });

        assert_eq!(&*page.borrow(), "2");
    }

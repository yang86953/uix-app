use super::*;
use crate::core::Point;
use crate::ui::component::traits::EventHandler;
use crate::ui::event::SystemEvent;
use crate::ui::{KeyMod, MouseButton};

fn dummy_event() -> SystemEvent {
    SystemEvent::PointerUp {
        pos: Point::new(0.0, 0.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    }
}

#[test]
fn on_link_callback_fires_when_submit_emitted() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let hook = calls.clone();
    let rich = RichText::new().on_link(move |url| hook.borrow_mut().push(url.to_string()));
    rich.pending_submit
        .replace(Some("https://example.com".to_string()));

    let event = rich.semantic_event(ComponentId::default(), &dummy_event());
    assert!(event.is_some(), "应发出 Submit 语义事件");
    assert_eq!(*calls.borrow(), vec!["https://example.com".to_string()]);
}

#[test]
fn on_link_not_called_without_pending_submit() {
    let calls = Rc::new(RefCell::new(0usize));
    let hook = calls.clone();
    let rich = RichText::new().on_link(move |_| *hook.borrow_mut() += 1);

    let event = rich.semantic_event(ComponentId::default(), &dummy_event());
    assert!(event.is_none());
    assert_eq!(*calls.borrow(), 0, "无待提交链接时不应触发回调");
}

#[test]
fn on_link_and_submit_semantic_event_coexist() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let hook = calls.clone();
    let rich = RichText::new().on_link(move |url| hook.borrow_mut().push(url.to_string()));
    rich.pending_submit
        .replace(Some("https://uix.dev/route".to_string()));

    let Some(event) = rich.semantic_event(ComponentId::default(), &dummy_event()) else {
        panic!("Submit 语义事件保留");
    };
    assert_eq!(
        event.kind,
        crate::ui::SemanticKind::Submit,
        "与 SemanticKind::Submit 共存"
    );
    assert!(
        matches!(&event.payload, crate::ui::SemanticPayload::Text(url) if url == "https://uix.dev/route")
    );
    assert_eq!(*calls.borrow(), vec!["https://uix.dev/route".to_string()]);
}

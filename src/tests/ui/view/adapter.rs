use super::*;
use crate::draw::Color;
use crate::ui::view::ViewNode;
use crate::ui::widgets::Container;
use crate::ui::WidgetCore;

#[test]
fn test_build_with_children() {
    let child1 = ViewNode::leaf(Container::new());
    let child2 = ViewNode::leaf(Container::new());
    let node = ViewNode::new(Container::new(), vec![child1, child2]);
    let tree = ViewAdapter::build_nodes(node);
    let root_id = tree.root_id().expect("root node should exist");
    let root = tree.get(root_id).expect("root node should be present");
    let child_ids = root.children().to_vec();
    assert_eq!(child_ids.len(), 2);
}

#[test]
fn test_expand_key_and_zindex() {
    let node = ViewNode::leaf(Container::new())
        .key("my-container")
        .z_index(10);
    let wnode = ViewAdapter::expand(node);
    assert_eq!(wnode.key, Some("my-container".into()));
    assert_eq!(wnode.z_index, 10);
}

#[test]
fn test_apply_style_container() {
    let mut style = Style::default();
    style.background = Some(crate::ui::style::ColorValue::Custom(Color::red()));
    let widget: Box<dyn WidgetComponent> = Box::new(Container::new());
    let styled = ViewAdapter::apply_style(widget, &style);
    if let Some(c) = styled.as_any().downcast_ref::<Container>() {
        assert_eq!(
            c.style.background,
            Some(crate::ui::style::ColorValue::Custom(Color::red()))
        );
    } else {
        panic!("expected Container");
    }
}

#[test]
fn grid_style_tracks_drive_layout() {
    use crate::core::Rect;
    use crate::ui::layout::GridTrack;
    use crate::ui::view::{grid, label};

    let mut tree = ViewAdapter::build(
        grid([label("A"), label("B")])
            .columns(vec![GridTrack::Px(50.0), GridTrack::Px(70.0)])
            .gap(10.0),
    );
    let root_id = tree.root_id().expect("grid root should exist");
    tree.get_mut(root_id)
        .expect("grid root should be present")
        .set_frame(Rect::new(0.0, 0.0, 140.0, 40.0));

    tree.push_layout_invalidation(root_id);
    tree.layout();

    let children = tree
        .get(root_id)
        .expect("grid root should remain present")
        .children()
        .to_vec();
    assert_eq!(children.len(), 2);
    let first = tree.get(children[0]).unwrap().frame();
    let second = tree.get(children[1]).unwrap().frame();
    assert_eq!(first.x, 0.0);
    assert_eq!(first.y, 0.0);
    assert_eq!(first.h, 40.0);
    assert_eq!(second.x, 60.0);
    assert_eq!(second.y, 0.0);
    assert_eq!(second.h, 40.0);
}

#[test]
fn reconcile_reuses_keyed_children_and_updates_label_text() {
    use crate::ui::view::{column, label};
    use crate::ui::widgets::Label;
    use crate::ui::AppState;

    let mut tree = ViewAdapter::build_nodes(column(vec![label("A").key("a"), label("B").key("b")]));
    let app_state = AppState::new();
    tree.set_app_state(app_state.clone());
    tree.layout();
    let root_id = tree.root_id().expect("root should exist");
    let old_children = tree.get(root_id).unwrap().children().to_vec();
    let old_b = old_children[1];
    let old_b_handle = app_state.get_handle(old_b).unwrap();
    assert_eq!(old_b_handle.text().as_deref(), Some("B"));

    ViewAdapter::reconcile_nodes(
        &mut tree,
        column(vec![label("B2").key("b"), label("A2").key("a")]),
    );

    let new_children = tree.get(root_id).unwrap().children().to_vec();
    assert_eq!(new_children, vec![old_children[1], old_children[0]]);
    assert_eq!(old_b_handle.text().as_deref(), Some("B2"));
    let first = tree
        .get(new_children[0])
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Label>()
        .unwrap();
    let second = tree
        .get(new_children[1])
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Label>()
        .unwrap();
    assert_eq!(first.text(), "B2");
    assert_eq!(second.text(), "A2");
}

#[test]
fn reconcile_reregisters_root_handlers() {
    use crate::core::{Point, Rect};
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::view::button;
    use crate::ui::SystemEvent;
    use std::cell::Cell;
    use std::rc::Rc;

    let old_hits = Rc::new(Cell::new(0));
    let new_hits = Rc::new(Cell::new(0));
    let old_for_handler = old_hits.clone();
    let new_for_handler = new_hits.clone();

    let mut tree = ViewAdapter::build(button("Old").on_click(move || {
        old_for_handler.set(old_for_handler.get() + 1);
    }));
    let root_id = tree.root_id().expect("button root should exist");
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 80.0, 32.0));

    let pos = Point::new(4.0, 4.0);
    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    ViewAdapter::reconcile(
        &mut tree,
        button("New").on_click(move || {
            new_for_handler.set(new_for_handler.get() + 1);
        }),
    );

    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(old_hits.get(), 1);
    assert_eq!(new_hits.get(), 1);
    let button = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Button>()
        .unwrap();
    assert_eq!(button.text(), "New");
}

#[test]
fn reconcile_preserves_consumed_once_handler_when_signature_is_unchanged() {
    use crate::core::Point;
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::event::{
        ClickEvent, HandlerOptions, HandlerRegistration, SemanticEvent, SemanticKind,
    };
    use crate::ui::view::button;
    use crate::ui::view::View;
    use std::cell::Cell;
    use std::rc::Rc;

    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_generation(7),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_generation(7),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn reconcile_reregisters_handler_when_generation_changes() {
    use crate::core::Point;
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::event::{
        ClickEvent, HandlerOptions, HandlerRegistration, SemanticEvent, SemanticKind,
    };
    use crate::ui::view::button;
    use crate::ui::view::View;
    use std::cell::Cell;
    use std::rc::Rc;

    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_generation(7),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_generation(8),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn reconcile_preserves_consumed_once_handler_when_state_capture_fingerprint_is_unchanged() {
    use crate::core::Point;
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::event::{
        ClickEvent, HandlerOptions, HandlerRegistration, SemanticEvent, SemanticKind,
    };
    use crate::ui::state::State;
    use crate::ui::view::button;
    use crate::ui::view::View;
    use std::cell::Cell;
    use std::rc::Rc;

    let state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_state_capture(&state),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    state.set(2);
    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_state_capture(&state),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn reconcile_reregisters_handler_when_state_capture_fingerprint_changes() {
    use crate::core::Point;
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::event::{
        ClickEvent, HandlerOptions, HandlerRegistration, SemanticEvent, SemanticKind,
    };
    use crate::ui::state::State;
    use crate::ui::view::button;
    use crate::ui::view::View;
    use std::cell::Cell;
    use std::rc::Rc;

    let first_state = State::new(1);
    let second_state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_state_capture(&first_state),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_state_capture(&second_state),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn button_dsl_state_capture_preserves_handler_when_fingerprint_is_unchanged() {
    use crate::core::Point;
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::state::State;
    use crate::ui::view::button;
    use crate::ui::SystemEvent;
    use std::cell::Cell;
    use std::rc::Rc;

    let state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first_for_handler = first_hits.clone();
    let second_for_handler = second_hits.clone();

    let mut tree = ViewAdapter::build(button("First").on_click_capture(&state, move || {
        first_for_handler.set(first_for_handler.get() + 1);
    }));
    let pos = Point::new(4.0, 4.0);

    state.set(2);
    ViewAdapter::reconcile(
        &mut tree,
        button("Second").on_click_capture(&state, move || {
            second_for_handler.set(second_for_handler.get() + 1);
        }),
    );

    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn input_dsl_state_capture_reregisters_handler_when_fingerprint_changes() {
    use crate::core::{Point, Rect};
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::state::State;
    use crate::ui::view::input;
    use crate::ui::{SystemEvent, WidgetCore};
    use std::cell::RefCell;
    use std::rc::Rc;

    let first_state = State::new(1);
    let second_state = State::new(1);
    let first_value = Rc::new(RefCell::new(String::new()));
    let second_value = Rc::new(RefCell::new(String::new()));
    let first_for_handler = first_value.clone();
    let second_for_handler = second_value.clone();

    let mut tree = ViewAdapter::build(input().on_change_capture(&first_state, move |next| {
        *first_for_handler.borrow_mut() = next.to_string();
    }));
    let root = tree.root_id().unwrap();
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 120.0, 32.0));

    ViewAdapter::reconcile(
        &mut tree,
        input().on_change_capture(&second_state, move |next| {
            *second_for_handler.borrow_mut() = next.to_string();
        }),
    );

    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(8.0, 8.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::TextInput {
        text: "A".to_string(),
    });

    assert_eq!(&*first_value.borrow(), "");
    assert_eq!(&*second_value.borrow(), "A");
}

#[test]
fn view_node_state_capture_preserves_handler_when_fingerprint_is_unchanged() {
    use crate::ui::event::{SemanticEvent, SemanticKind};
    use crate::ui::state::State;
    use crate::ui::view::label;
    use std::cell::Cell;
    use std::rc::Rc;

    let state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first_for_handler = first_hits.clone();
    let second_for_handler = second_hits.clone();

    let mut tree = ViewAdapter::build(label("First").on_semantic_capture(
        SemanticKind::Change,
        &state,
        move |_| {
            first_for_handler.set(first_for_handler.get() + 1);
        },
    ));
    let root_id = tree.root_id().unwrap();

    ViewAdapter::reconcile(
        &mut tree,
        label("Second").on_semantic_capture(SemanticKind::Change, &state, move |_| {
            second_for_handler.set(second_for_handler.get() + 1);
        }),
    );

    let _ = tree.dispatch_semantic(SemanticEvent::change(root_id, "next"));

    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn view_node_state_capture_reregisters_handler_when_fingerprint_changes() {
    use crate::ui::event::{SemanticEvent, SemanticKind};
    use crate::ui::state::State;
    use crate::ui::view::label;
    use std::cell::Cell;
    use std::rc::Rc;

    let first_state = State::new(1);
    let second_state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first_for_handler = first_hits.clone();
    let second_for_handler = second_hits.clone();

    let mut tree = ViewAdapter::build(label("First").on_semantic_capture(
        SemanticKind::Change,
        &first_state,
        move |_| {
            first_for_handler.set(first_for_handler.get() + 1);
        },
    ));
    let root_id = tree.root_id().unwrap();

    ViewAdapter::reconcile(
        &mut tree,
        label("Second").on_semantic_capture(SemanticKind::Change, &second_state, move |_| {
            second_for_handler.set(second_for_handler.get() + 1);
        }),
    );

    let _ = tree.dispatch_semantic(SemanticEvent::change(root_id, "next"));

    assert_eq!(first_hits.get(), 0);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn test_state_auto_dirty() {
    use crate::ui::state::State;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let state = State::new(42);
    let dirty_called = Arc::new(AtomicBool::new(false));
    let dirty_called_clone = dirty_called.clone();
    state.set_dirty_fn(move || dirty_called_clone.store(true, Ordering::SeqCst));
    assert!(!dirty_called.load(Ordering::SeqCst));
    state.set(100);
    assert!(dirty_called.load(Ordering::SeqCst));
}

#[test]
fn captured_state_set_requests_reconcile_and_preserves_paint_invalidation() {
    use crate::core::Rect;
    use crate::ui::state::State;
    use crate::ui::view::dynamic_label;

    let state = State::new(1);
    let mut tree = ViewAdapter::build({
        let state = state.clone();
        dynamic_label(move || format!("value-{}", state.get()))
    });
    let root_id = tree.root_id().expect("root should exist");
    tree.get_mut(root_id)
        .expect("root should be present")
        .set_frame(Rect::new(0.0, 0.0, 80.0, 24.0));
    tree.layout();
    tree.reset_dirty();

    assert!(!tree.take_reconcile_requested());
    assert!(!tree.has_render_work());

    state.set(2);

    assert!(tree.take_reconcile_requested());
    assert!(tree.has_render_work());
}

#[test]
fn button_on_click_is_registered_as_semantic_handler() {
    use crate::core::Point;
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::view::button;
    use crate::ui::SystemEvent;
    use std::cell::Cell;
    use std::rc::Rc;

    let clicks = Rc::new(Cell::new(0));
    let clicks_for_handler = clicks.clone();
    let mut tree = ViewAdapter::build(button("OK").on_click(move || {
        clicks_for_handler.set(clicks_for_handler.get() + 1);
    }));

    let pos = Point::new(2.0, 2.0);
    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(clicks.get(), 1);
}

#[test]
fn input_on_change_is_registered_as_semantic_handler() {
    use crate::core::{Point, Rect};
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::view::input;
    use crate::ui::{SystemEvent, WidgetCore};
    use std::cell::RefCell;
    use std::rc::Rc;

    let value = Rc::new(RefCell::new(String::new()));
    let value_for_handler = value.clone();
    let mut tree = ViewAdapter::build(input().on_change(move |next| {
        *value_for_handler.borrow_mut() = next.to_string();
    }));

    let root = tree.root_id().expect("input root should exist");
    tree.get_mut(root)
        .expect("input root should be present")
        .set_frame(Rect::new(0.0, 0.0, 120.0, 32.0));

    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(8.0, 8.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::TextInput {
        text: "A".to_string(),
    });

    assert_eq!(&*value.borrow(), "A");
}

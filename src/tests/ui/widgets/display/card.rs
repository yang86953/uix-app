use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::display::card::*;
use crate::ui::AccessibilityRole;

struct FixedChild(Size);

impl WidgetComponent for FixedChild {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT)
    }

    crate::wc_upcast!(FixedChild; WidgetLayout);
}

impl WidgetLayout for FixedChild {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(self.0)
    }
}

struct CountingChild {
    size: Size,
    measure_calls: Rc<Cell<usize>>,
}

impl WidgetComponent for CountingChild {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT)
    }

    crate::wc_upcast!(CountingChild; WidgetLayout);
}

impl WidgetLayout for CountingChild {
    fn measure(&self, constraints: Constraints) -> Size {
        self.measure_calls.set(self.measure_calls.get() + 1);
        constraints.clamp(self.size)
    }
}

#[test]
fn layout_children_measure_children_with_inner_constraints() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Card::new()
            .size(80.0, 80.0)
            .padding(16.0)
            .child(FixedChild(Size::new(200.0, 120.0))),
    ));

    tree.layout();

    let child = tree.get(root).unwrap().children()[0];
    let frame = tree.get(child).unwrap().frame();
    assert_eq!(frame.w, 48.0);
    assert_eq!(frame.h, 48.0);
}

#[test]
fn layout_children_reserve_actions_and_zero_exhausted_body() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Card::new()
            .size(80.0, 80.0)
            .padding(16.0)
            .actions(vec!["Save"])
            .child(FixedChild(Size::new(200.0, 120.0))),
    ));

    tree.layout();

    let child = tree.get(root).unwrap().children()[0];
    assert_eq!(
        tree.get(child).unwrap().frame(),
        Rect::new(16.0, 16.0, 48.0, 8.0)
    );

    let narrow_frame = Rect::new(0.0, 0.0, 80.0, 30.0);
    let narrow_root = tree.set_root(Box::new(
        Card::new()
            .title("Title")
            .actions(vec!["Save"])
            .padding(16.0),
    ));
    let narrow_child = tree.add_child(narrow_root, Box::new(FixedChild(Size::new(40.0, 20.0))));
    tree.get_mut(narrow_child)
        .unwrap()
        .set_frame(Rect::new(16.0, 56.0, 48.0, 20.0));

    let widget = tree.get(narrow_root).unwrap().as_layout().unwrap();
    let measured = widget.measure_children(narrow_frame, &[narrow_child], &tree);
    let placements = widget.layout_children(narrow_frame, &measured, &tree);
    assert_eq!(placements[0].1, Rect::new(16.0, 0.0, 0.0, 0.0));
}

#[test]
fn action_rect_stays_inside_short_card() {
    let card = Card::new().actions(vec!["Save", "Cancel"]);

    assert_eq!(
        card.action_rect(Rect::new(4.0, 6.0, 80.0, 24.0)),
        Some(Rect::new(4.0, 6.0, 80.0, 24.0))
    );
}

#[test]
fn card_actions_support_local_pointer_and_keyboard_submission() {
    let mut card = Card::new().title("Profile").actions(vec!["Save", "Cancel"]);
    card.set_frame_for_test(Rect::new(80.0, 40.0, 200.0, 120.0));

    assert_eq!(card.tab_index(), 1);
    assert_eq!(
        card.on_event(&SystemEvent::PointerDown {
            pos: Point::new(150.0, 100.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let event = card
        .semantic_event(ComponentId::new(4), &SystemEvent::FocusIn)
        .expect("pointer action should emit submit");
    assert_eq!(event.kind, SemanticKind::Submit);
    assert_eq!(event.text_payload(), Some("Cancel"));
    assert_eq!(card.focused_action(), Some(1));

    assert_eq!(
        card.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Home,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        card.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let event = card
        .semantic_event(ComponentId::new(4), &SystemEvent::FocusIn)
        .expect("keyboard action should emit submit");
    assert_eq!(event.text_payload(), Some("Save"));
}

#[test]
fn card_normalizes_geometry_and_exposes_focused_action_semantics() {
    let mut card = Card::new()
        .title("Profile")
        .actions(vec!["", "Save"])
        .size(f32::NAN, -1.0)
        .padding(f32::NAN)
        .flex_grow(-2.0);
    assert_eq!(card.action_labels(), &["Save"]);
    assert_eq!(
        card.measure(Constraints::loose(Size::new(500.0, 500.0))),
        Size::new(200.0, 120.0)
    );
    card.set_frame_for_test(Rect::new(0.0, 0.0, 200.0, 120.0));
    let _ = card.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });

    let accessibility = card.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Group);
    assert_eq!(accessibility.name.as_deref(), Some("Profile"));
    assert_eq!(accessibility.state.value_text.as_deref(), Some("Save"));
    assert_eq!(accessibility.state.value_now, Some(1.0));
}

#[test]
fn card_arrange_uses_precomputed_measurements() {
    let calls = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Card::new().size(120.0, 80.0).padding(16.0)));
    let child = tree.add_child(
        root,
        Box::new(CountingChild {
            size: Size::new(200.0, 120.0),
            measure_calls: Rc::clone(&calls),
        }),
    );
    let frame = Rect::new(0.0, 0.0, 120.0, 80.0);
    let widget = tree.get(root).unwrap().as_layout().unwrap();
    let measured = widget.measure_children(frame, &[child], &tree);

    assert_eq!(calls.get(), 1);
    let placements = widget.layout_children(frame, &measured, &tree);

    assert_eq!(calls.get(), 1, "arrange must not measure children again");
    assert_eq!(placements[0].1, Rect::new(16.0, 16.0, 88.0, 48.0));
}

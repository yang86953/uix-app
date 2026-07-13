use crate::tests::common::*;
use crate::component;
use crate::draw::{ Radius };
use crate::ui::children::WidgetChildren;
use crate::ui::layout::{ child_from_tree_with_constraints, flex::compute_flex_layout, FlexChild, FlexInput, LayoutChild };
use crate::ui::widgets::display::card::*;
use crate::ui::core::widget::WidgetCore;

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
fn measure_clamps_card_size() {
    let measured = Card::new()
        .size(240.0, 120.0)
        .measure(Constraints::loose(Size::new(100.0, 60.0)));

    assert_eq!(measured, Size::new(100.0, 60.0));
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

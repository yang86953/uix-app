use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::input::form::*;

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

fn assert_rect_within(parent: Rect, child: Rect) {
    assert!(child.w >= 0.0);
    assert!(child.h >= 0.0);
    assert!(child.x >= parent.x);
    assert!(child.y >= parent.y);
    assert!(child.x + child.w <= parent.x + parent.w);
    assert!(child.y + child.h <= parent.y + parent.h);
}

#[test]
fn measure_clamps_form_item_size() {
    let measured = FormItem::new("Name").measure(Constraints::loose(Size::new(120.0, 20.0)));

    assert_eq!(measured, Size::new(120.0, 20.0));
}

#[test]
fn measure_clamps_form_size() {
    let measured = Form::new().measure(Constraints::loose(Size::new(160.0, 80.0)));

    assert_eq!(measured, Size::new(160.0, 80.0));
}

#[test]
fn form_shell_is_picture_eligible_but_form_item_is_not() {
    assert_eq!(Form::new().picture_policy(), PicturePolicy::Eligible);
    assert_eq!(FormItem::new("Name").picture_policy(), PicturePolicy::Never);
}

#[test]
fn layout_children_measure_children_with_frame_constraints() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Form::new().layout(FormLayout::Horizontal)));
    tree.add_child(root, Box::new(FixedChild(Size::new(240.0, 120.0))));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 80.0, 60.0));

    tree.layout();

    let child = tree.get(root).unwrap().children()[0];
    assert_eq!(
        tree.get(child).unwrap().frame(),
        Rect::new(0.0, 0.0, 80.0, 60.0)
    );
}

#[test]
fn form_layout_children_keep_all_items_within_narrow_frame() {
    let frame = Rect::new(10.0, 20.0, 80.0, 20.0);
    for layout in [
        FormLayout::Vertical,
        FormLayout::Inline,
        FormLayout::Horizontal,
    ] {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(Form::new().layout(layout)));
        let first = tree.add_child(root, Box::new(FixedChild(Size::new(10.0, 10.0))));
        let second = tree.add_child(root, Box::new(FixedChild(Size::new(10.0, 10.0))));

        let widget = tree.get(root).unwrap().as_layout().unwrap();
        let measured = widget.measure_children(frame, &[first, second], &tree);
        let placements = widget.layout_children(frame, &measured, &tree);

        assert_eq!(placements.len(), 2);
        for &(_, rect) in &placements {
            assert_rect_within(frame, rect);
        }
    }
}

#[test]
fn form_item_layout_children_clamp_label_padding_and_status_regions() {
    let frame = Rect::new(10.0, 20.0, 40.0, 10.0);
    for layout in [
        FormLayout::Vertical,
        FormLayout::Inline,
        FormLayout::Horizontal,
    ] {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(FormItem::new("Name").layout(layout)));
        let child = tree.add_child(root, Box::new(FixedChild(Size::new(20.0, 8.0))));

        let widget = tree.get(root).unwrap().as_layout().unwrap();
        let measured = widget.measure_children(frame, &[child], &tree);
        let placements = widget.layout_children(frame, &measured, &tree);

        assert_eq!(placements.len(), 1);
        assert_rect_within(frame, placements[0].1);
    }
}

#[test]
fn form_arrange_uses_precomputed_measurements() {
    let calls = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Form::new().layout(FormLayout::Inline)));
    let child = tree.add_child(
        root,
        Box::new(CountingChild {
            size: Size::new(160.0, 20.0),
            measure_calls: Rc::clone(&calls),
        }),
    );
    let frame = Rect::new(0.0, 0.0, 200.0, 40.0);
    let widget = tree.get(root).unwrap().as_layout().unwrap();
    let measured = widget.measure_children(frame, &[child], &tree);

    assert_eq!(calls.get(), 1);
    let placements = widget.layout_children(frame, &measured, &tree);

    assert_eq!(calls.get(), 1, "arrange must not measure children again");
    assert_eq!(placements[0].1, Rect::new(0.0, 0.0, 160.0, 40.0));
}

#[test]
fn form_item_arrange_uses_precomputed_measurements() {
    let calls = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(FormItem::new("Name")));
    let child = tree.add_child(
        root,
        Box::new(CountingChild {
            size: Size::new(80.0, 32.0),
            measure_calls: Rc::clone(&calls),
        }),
    );
    let frame = Rect::new(0.0, 0.0, 100.0, 44.0);
    let widget = tree.get(root).unwrap().as_layout().unwrap();
    let measured = widget.measure_children(frame, &[child], &tree);

    assert_eq!(calls.get(), 1);
    let placements = widget.layout_children(frame, &measured, &tree);

    assert_eq!(calls.get(), 1, "arrange must not measure children again");
    assert_eq!(placements[0].1, Rect::new(88.0, 2.0, 12.0, 26.0));
}

#[test]
fn custom_validator_is_resolved_from_named_table() {
    let calls = Rc::new(Cell::new(0));
    let validator_calls = Rc::clone(&calls);
    let mut validators = FormValidatorTable::new();
    validators.register("username", move |value| {
        validator_calls.set(validator_calls.get() + 1);
        (value == "ada")
            .then_some(())
            .ok_or_else(|| "unknown user".to_string())
    });
    let mut form = Form::new().with_field(
        FieldDef::new("user", "User")
            .value("grace")
            .rule(ValidationRule::required("required").validator("username")),
    );

    assert_eq!(form.validate(&validators), Ok(false));
    assert_eq!(calls.get(), 1);
    let field = form.field("user").expect("registered field");
    assert_eq!(field.status, ValidateStatus::Error);
    assert_eq!(field.message, "unknown user");

    form.set_field_value("user", "ada");
    assert_eq!(form.validate(&validators), Ok(true));
    assert_eq!(calls.get(), 2);
    assert_eq!(form.field("user").unwrap().status, ValidateStatus::None);
}

#[test]
fn missing_custom_validator_is_a_typed_configuration_error() {
    let key = FormValidatorKey::new("email-domain");
    let mut form = Form::new().with_field(
        FieldDef::new("email", "Email")
            .value("ada@example.test")
            .rule(ValidationRule::required("required").validator(key.clone())),
    );

    assert_eq!(
        form.validate(&FormValidatorTable::new()),
        Err(FormValidationError::MissingValidator {
            field: "email".to_string(),
            key,
        })
    );
    assert_eq!(form.field("email").unwrap().status, ValidateStatus::None);
}

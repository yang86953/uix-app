use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::input::form::*;
use crate::ui::{FieldError, Trigger};

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
fn inline_custom_and_builtin_rules_collect_errors_in_field_order() {
    let calls = Rc::new(Cell::new(0));
    let validator_calls = Rc::clone(&calls);
    let form = Form::new()
        .field("user", "User")
        .default("grace")
        .required("required")
        .custom(move |value| {
            validator_calls.set(validator_calls.get() + 1);
            (value == "ada")
                .then_some(())
                .ok_or_else(|| "unknown user".to_string())
        })
        .field("email", "Email")
        .default("invalid")
        .validate_email("invalid email")
        .field("age", "Age")
        .default(0i32)
        .validate_range(1..=120, "invalid age")
        .field("code", "Code")
        .default("ab")
        .validate_length(3..=8, "invalid length")
        .validate_pattern(r"^[a-z]+$", "invalid pattern")
        .build();

    let errors = form.validate().expect_err("form should be invalid");
    assert_eq!(calls.get(), 1);
    assert_eq!(errors.len(), 4);
    assert_eq!(errors[0].field(), "user");
    assert_eq!(errors[0].message(), "unknown user");
    assert_eq!(errors[1].field(), "email");
    assert_eq!(errors[2].field(), "age");
    assert_eq!(errors[3].field(), "code");
}

#[test]
fn successful_validation_returns_typed_values() {
    let form = Form::new()
        .field("user", "User")
        .default("ada")
        .required("required")
        .field("email", "Email")
        .default("ada@example.test")
        .validate_email("invalid email")
        .field("age", "Age")
        .default(42i32)
        .validate_range(1..=120, "invalid age")
        .field("code", "Code")
        .default("ABC-12")
        .validate_length(3..=8, "invalid length")
        .validate_pattern(r"^[A-Z]+-[0-9]+$", "invalid pattern")
        .build();

    let values = form.validate().expect("form should be valid");
    assert_eq!(values.len(), 4);
    assert_eq!(values.get::<String>("user"), Some(&"ada".to_string()));
    assert_eq!(values.get::<i32>("age"), Some(&42));
    assert!(values.get::<String>("age").is_none());
    assert_eq!(form.field_label("email"), Some("Email"));
}

#[test]
fn set_value_revalidates_without_rebuilding_rules() {
    let mut form = Form::new()
        .field("user", "User")
        .default("")
        .required("required")
        .build();

    assert_eq!(form.validate().unwrap_err()[0].message(), "required");
    assert!(form.set_value("user", "ada"));
    assert!(!form.set_value("missing", "ignored"));
    let values = form.validate().expect("updated form should be valid");
    assert_eq!(values.get::<String>("user"), Some(&"ada".to_string()));
}

#[test]
fn invalid_regex_is_a_field_error_and_optional_empty_builtins_are_skipped() {
    let invalid = Form::new()
        .field("code", "Code")
        .default("")
        .validate_pattern("(", "invalid pattern")
        .build();
    let error = invalid
        .validate()
        .unwrap_err()
        .into_iter()
        .next()
        .expect("pattern error");
    assert_eq!(
        error.into_parts(),
        ("code".to_string(), "invalid pattern".to_string())
    );

    let optional = Form::new()
        .field("email", "Email")
        .default("")
        .validate_email("invalid email")
        .validate_length(2..=20, "invalid length")
        .validate_pattern(r"^[a-z]+$", "invalid pattern")
        .build();
    assert!(optional.validate().is_ok());
}

#[test]
fn duplicate_field_uses_the_last_declaration() {
    let form = Form::new()
        .field("value", "Old")
        .default(1i32)
        .field("value", "New")
        .default("latest")
        .build();

    let values = form.validate().expect("last declaration should be valid");
    assert_eq!(values.len(), 1);
    assert_eq!(values.get::<String>("value"), Some(&"latest".to_string()));
    assert_eq!(form.field_label("value"), Some("New"));
}

#[test]
fn validation_triggers_activate_only_at_the_declared_boundary() {
    let mut form = Form::new()
        .field("submit", "Submit")
        .default("")
        .required("submit required")
        .field("change", "Change")
        .default("ready")
        .required("change required")
        .validate_trigger(Trigger::OnChange)
        .field("blur", "Blur")
        .default("ready")
        .required("blur required")
        .validate_trigger(Trigger::OnBlur)
        .build();

    assert!(form.set_value("submit", ""));
    assert!(form.set_value("change", ""));
    assert!(form.set_value("blur", ""));
    assert_eq!(
        form.errors()
            .into_iter()
            .map(|error| error.field().to_string())
            .collect::<Vec<_>>(),
        vec!["change"]
    );

    assert!(form.blur("blur"));
    assert!(!form.blur("missing"));
    assert_eq!(
        form.errors()
            .into_iter()
            .map(|error| error.field().to_string())
            .collect::<Vec<_>>(),
        vec!["change", "blur"]
    );

    let errors = form.validate().expect_err("submit validates every field");
    assert_eq!(errors.len(), 3);
    assert_eq!(errors[0].field(), "submit");
}

#[test]
fn changing_a_field_replaces_or_clears_its_active_error() {
    let mut form = Form::new()
        .field("change", "Change")
        .default("ready")
        .required("required")
        .validate_trigger(Trigger::OnChange)
        .field("blur", "Blur")
        .default("")
        .required("required")
        .validate_trigger(Trigger::OnBlur)
        .build();

    assert!(form.set_value("change", ""));
    assert_eq!(
        form.field_error("change").as_ref().map(FieldError::message),
        Some("required")
    );
    assert!(form.set_value("change", "valid"));
    assert!(form.field_error("change").is_none());

    assert!(form.blur("blur"));
    assert!(form.field_error("blur").is_some());
    assert!(form.set_value("blur", "changed"));
    assert!(form.field_error("blur").is_none());
}

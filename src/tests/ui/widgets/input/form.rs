use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::view::{column, ViewAdapter};
use crate::ui::widgets::input::form::*;
use crate::ui::{FieldError, Input, State, Trigger};

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
    let form = Form::new()
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
    let form = Form::new()
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
    let form = Form::new()
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

#[test]
fn dependent_validator_reads_typed_values_and_runs_on_submit() {
    let form = Form::new()
        .field("password", "Password")
        .default("secret")
        .field("confirm", "Confirm")
        .default("different")
        .depends_on("password", |value, values| {
            (values.get::<String>("password").map(String::as_str) == Some(value))
                .then_some(())
                .ok_or_else(|| "passwords differ".to_string())
        })
        .build();

    let errors = form.validate().expect_err("confirmation should fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].field(), "confirm");
    assert_eq!(errors[0].message(), "passwords differ");

    assert!(form.set_value("confirm", "secret"));
    let values = form.validate().expect("matching values should pass");
    assert_eq!(values.get::<String>("confirm"), Some(&"secret".to_string()));
}

#[test]
fn changing_a_dependency_cascades_in_declaration_order() {
    let region_calls = Rc::new(Cell::new(0));
    let code_calls = Rc::new(Cell::new(0));
    let region_counter = Rc::clone(&region_calls);
    let code_counter = Rc::clone(&code_calls);
    let form = Form::new()
        .field("country", "Country")
        .default("EU")
        .field("region", "Region")
        .default("EU")
        .depends_on("country", move |value, values| {
            region_counter.set(region_counter.get() + 1);
            (values.get::<String>("country").map(String::as_str) == Some(value))
                .then_some(())
                .ok_or_else(|| "region mismatch".to_string())
        })
        .field("code", "Code")
        .default("EU-1")
        .depends_on("region", move |value, values| {
            code_counter.set(code_counter.get() + 1);
            value
                .starts_with(
                    values
                        .get::<String>("country")
                        .map(String::as_str)
                        .unwrap_or_default(),
                )
                .then_some(())
                .ok_or_else(|| "code mismatch".to_string())
        })
        .build();

    form.validate().expect("initial dependencies should pass");
    region_calls.set(0);
    code_calls.set(0);
    assert!(form.set_value("country", "US"));

    assert_eq!(region_calls.get(), 1);
    assert_eq!(code_calls.get(), 1);
    assert_eq!(
        form.errors()
            .iter()
            .map(FieldError::field)
            .collect::<Vec<_>>(),
        vec!["region", "code"]
    );
}

#[test]
fn cyclic_dependencies_are_revalidated_once_per_change() {
    let left_calls = Rc::new(Cell::new(0));
    let right_calls = Rc::new(Cell::new(0));
    let left_counter = Rc::clone(&left_calls);
    let right_counter = Rc::clone(&right_calls);
    let form = Form::new()
        .field("left", "Left")
        .default("same")
        .depends_on("right", move |_, _| {
            left_counter.set(left_counter.get() + 1);
            Ok(())
        })
        .field("right", "Right")
        .default("same")
        .depends_on("left", move |_, _| {
            right_counter.set(right_counter.get() + 1);
            Ok(())
        })
        .build();

    assert!(form.set_value("left", "changed"));
    assert_eq!(left_calls.get(), 0);
    assert_eq!(right_calls.get(), 1);
}

#[test]
fn form_input_item_binds_string_state_and_reconciles_inline_error() {
    let value = State::new("ready".to_string());
    let form = Form::new()
        .field("name", "Name")
        .default("ready")
        .required("Name is required")
        .validate_trigger(Trigger::OnChange)
        .build();
    let mut tree = ViewAdapter::build(
        form.input_item("name", &value)
            .expect("declared field should build an input item"),
    );
    let root = tree.root_id().expect("form item root");

    let item = tree
        .get(root)
        .and_then(|node| node.component().as_any().downcast_ref::<FormItem>())
        .expect("form item component");
    assert_eq!(item.get_status(), ValidateStatus::None);

    value.set(String::new());
    ViewAdapter::reconcile(
        &mut tree,
        form.input_item("name", &value)
            .expect("declared field should reconcile an input item"),
    );

    let item = tree
        .get(root)
        .and_then(|node| node.component().as_any().downcast_ref::<FormItem>())
        .expect("reconciled form item component");
    assert_eq!(item.get_status(), ValidateStatus::Error);
    assert!(matches!(
        item.snapshot_fields(),
        SnapshotFields::FormItem { help, .. } if help == "Name is required"
    ));
    let input = tree
        .get(root)
        .and_then(|node| node.children().first().copied())
        .and_then(|id| tree.get(id))
        .and_then(|node| node.component().as_any().downcast_ref::<Input>())
        .expect("bound input child");
    assert_eq!(input.current_value(), "");
}

#[test]
fn form_input_item_blur_keeps_error_status_when_help_is_hidden() {
    let value = State::new(String::new());
    let form = Form::new()
        .field("name", "Name")
        .default("")
        .required("Name is required")
        .validate_trigger(Trigger::OnBlur)
        .build();
    let mut tree = ViewAdapter::build(
        form.input_item("name", &value)
            .expect("declared field should build an input item")
            .show_error(false),
    );
    let root = tree.root_id().expect("form item root");
    let input = tree.get(root).expect("form item").children()[0];

    assert_eq!(
        tree.dispatch_to(input, &SystemEvent::FocusOut),
        EventResult::Handled
    );
    ViewAdapter::reconcile(
        &mut tree,
        form.input_item("name", &value)
            .expect("declared field should reconcile an input item")
            .show_error(false),
    );

    let item = tree
        .get(root)
        .and_then(|node| node.component().as_any().downcast_ref::<FormItem>())
        .expect("reconciled form item component");
    assert_eq!(item.get_status(), ValidateStatus::Error);
    assert!(matches!(
        item.snapshot_fields(),
        SnapshotFields::FormItem { help, .. } if help.is_empty()
    ));
}

#[test]
fn cloned_form_model_shares_values_and_rejects_unknown_input_items() {
    let value = State::new("ready".to_string());
    let form = Form::new().field("name", "Name").default("initial").build();
    let cloned = form.clone();

    assert!(form.input_item("missing", &value).is_none());
    assert!(cloned.set_value("name", "updated".to_string()));

    let values = form.validate().expect("shared value should remain valid");
    assert_eq!(
        values.get::<String>("name").map(String::as_str),
        Some("updated")
    );
}

#[test]
fn form_submit_registers_focus_for_the_first_bound_error() {
    let first = State::new(String::new());
    let second = State::new(String::new());
    let form = Form::new()
        .field("first", "First")
        .default("")
        .required("First is required")
        .field("second", "Second")
        .default("")
        .required("Second is required")
        .build();
    let mut tree = ViewAdapter::build(column((
        form.input_item("first", &first),
        form.input_item("second", &second),
    )));
    tree.set_app_state(AppState::new());
    tree.layout();
    let root = tree.root_id().expect("form column root");
    let first_item = tree.get(root).expect("form column").children()[0];
    let first_input = tree.get(first_item).expect("first form item").children()[0];

    let errors = form.submit().expect_err("empty form should fail submit");

    assert_eq!(errors[0].field(), "first");
    assert!(tree.has_app_state_focus_requests());
    assert!(tree.drain_app_state_focus_requests());
    assert_eq!(tree.managers().focus.focused_component(), Some(first_input));
}

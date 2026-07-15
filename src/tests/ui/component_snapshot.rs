use std::any::TypeId;

use crate::component;
use crate::tests::common::*;
use crate::ui::widgets::{Button, Checkbox, Input, Label, QRCode, Slider};
use crate::ui::{
    AccessibilityRole, AccessibilitySnapshot, AriaAttribute, ComponentConfigSnapshot,
    SnapshotFields, SnapshotValue,
};

component! {
    struct SnapshotProbe {
        pub title: String,
        pub count: usize,
        #[snapshot(skip)]
        #[allow(dead_code)]
        pub hover_count: usize,
        _secret: String,
    }

    render => (
        &self,
        _frame: crate::core::Rect,
        _ctx: &mut crate::draw::painting::PaintContext,
        _tree: &crate::ui::core::widget::WidgetTree
    ) {}
}

component! {
    struct MeasureMacroProbe {
        pub label: String,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(72.0, 48.0))
    }

    render => (
        &self,
        _frame: crate::core::Rect,
        _ctx: &mut crate::draw::painting::PaintContext,
        _tree: &crate::ui::core::widget::WidgetTree
    ) {}
}

component! {
    struct CaptureMacroProbe {}

    on_event => (&mut self, _event: &SystemEvent) -> crate::ui::EventResult {
        crate::ui::EventResult::NotHandled
    }

    wants_capture_phase => (&self) -> bool {
        true
    }

    render => (
        &self,
        _frame: crate::core::Rect,
        _ctx: &mut crate::draw::painting::PaintContext,
        _tree: &crate::ui::core::widget::WidgetTree
    ) {}
}

component! {
    name: ComponentMacroProbe,
    struct ComponentMacroProbe {
        pub label: String,
        #[snapshot(skip)]
        #[allow(dead_code)]
        pub runtime_counter: usize,
        #[allow(dead_code)]
        private_note: String,
    }

    render => (
        &self,
        _frame: crate::core::Rect,
        _ctx: &mut crate::draw::painting::PaintContext,
        _tree: &crate::ui::core::widget::WidgetTree
    ) {}
}

#[test]
fn component_config_snapshot_records_id_type_and_fields() {
    let label = Label::new("status").font_size(18.0).size(80.0, 20.0);
    let snapshot = ComponentConfigSnapshot::from_component(ComponentId::new(7), &label);

    assert_eq!(snapshot.id, ComponentId::new(7));
    assert_eq!(snapshot.widget_type, TypeId::of::<Label>());
    assert!(matches!(
        snapshot.fields,
        SnapshotFields::Label {
            ref text,
            font_size: 18.0,
            fixed_width: Some(80.0),
            fixed_height: Some(20.0),
            ..
        } if text == "status"
    ));
}

#[test]
fn component_config_snapshot_dispatches_late_builtin() {
    let qrcode = QRCode::new("uix").size(96.0).error_level(2);
    let snapshot = ComponentConfigSnapshot::from_component(ComponentId::new(8), &qrcode);

    assert_eq!(snapshot.widget_type, TypeId::of::<QRCode>());
    assert_eq!(
        snapshot.fields,
        SnapshotFields::QRCode {
            value: "uix".to_string(),
            size: 96.0,
            error_level: 2,
        }
    );
}

#[test]
fn component_config_snapshot_derives_accessibility_metadata() {
    let button = Button::new("Save").disabled(true);
    let accessibility =
        ComponentConfigSnapshot::from_component(ComponentId::new(10), &button).accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Button);
    assert_eq!(accessibility.name.as_deref(), Some("Save"));
    assert!(accessibility.state.disabled);

    let checkbox = Checkbox::new("Agree").default_checked(true);
    let accessibility =
        ComponentConfigSnapshot::from_component(ComponentId::new(11), &checkbox).accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Checkbox);
    assert_eq!(accessibility.name.as_deref(), Some("Agree"));
    assert_eq!(accessibility.state.checked, Some(true));

    let slider = Slider::new(0.0..=10.0).default_value(4.0);
    let accessibility =
        ComponentConfigSnapshot::from_component(ComponentId::new(12), &slider).accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Slider);
    assert_eq!(accessibility.state.value_now, Some(4.0));
    assert_eq!(accessibility.state.value_min, Some(0.0));
    assert_eq!(accessibility.state.value_max, Some(10.0));
}

#[test]
fn accessibility_snapshot_derives_static_aria_metadata() {
    let button = Button::new("Save").disabled(true);
    let snapshot = ComponentConfigSnapshot::from_component(ComponentId::new(13), &button);
    assert_eq!(snapshot.aria_role(), Some("button"));
    assert_eq!(
        snapshot.aria_attributes(),
        vec![
            AriaAttribute::new("aria-label", "Save"),
            AriaAttribute::new("aria-disabled", "true"),
        ]
    );

    let required_group = AccessibilitySnapshot::named(AccessibilityRole::Group, "Email")
        .with_state(crate::ui::AccessibilityState {
            required: true,
            multiline: true,
            expanded: Some(true),
            selected: Some(false),
            ..crate::ui::AccessibilityState::default()
        })
        .with_attribute(AriaAttribute::new("aria-description", "Primary field"));
    assert_eq!(required_group.aria_role(), Some("group"));
    assert_eq!(
        required_group.aria_attributes(),
        vec![
            AriaAttribute::new("aria-label", "Email"),
            AriaAttribute::new("aria-expanded", "true"),
            AriaAttribute::new("aria-selected", "false"),
            AriaAttribute::new("aria-multiline", "true"),
            AriaAttribute::new("aria-required", "true"),
            AriaAttribute::new("aria-description", "Primary field"),
        ]
    );

    assert_eq!(
        AccessibilitySnapshot::new(AccessibilityRole::Generic).aria_role(),
        None
    );
    assert_eq!(
        AccessibilitySnapshot::new(AccessibilityRole::None).aria_role(),
        None
    );
}

#[test]
fn builtin_snapshots_exclude_runtime_state() {
    let mut button = Button::new("Save").block(true);
    let before = button.snapshot_fields();
    let _ = button.on_event(&SystemEvent::PointerEnter);
    let _ = button.on_event(&SystemEvent::PointerDown {
        button: MouseButton::Left,
        pos: Point::zero(),
        mods: KeyMod::NONE,
    });
    assert_eq!(button.snapshot_fields(), before);

    let input = Input::textarea()
        .placeholder("Search")
        .with_value("runtime text")
        .size(ControlSize::Large)
        .rows(4)
        .max_length(120);
    assert!(matches!(
        input.snapshot_fields(),
        SnapshotFields::Input {
            ref placeholder,
            input_size: ControlSize::Large,
            password: false,
            textarea: true,
            textarea_rows: 4,
            max_length: Some(120),
            ..
        } if placeholder == "Search"
    ));
}

#[test]
fn component_macro_snapshot_captures_public_fields_only() {
    let probe = SnapshotProbe {
        title: "Ready".to_string(),
        count: 3,
        hover_count: 7,
        _secret: "runtime".to_string(),
    };

    let snapshot = ComponentConfigSnapshot::from_component(ComponentId::new(42), &probe);
    let SnapshotFields::Custom { widget, fields } = snapshot.fields else {
        panic!("expected custom snapshot fields");
    };

    assert_eq!(widget, "SnapshotProbe");
    assert_eq!(fields.len(), 2);
    assert!(fields.iter().any(|field| field.name == "title"
        && field.value == SnapshotValue::Debug("\"Ready\"".to_string())));
    assert!(
        fields
            .iter()
            .any(|field| field.name == "count"
                && field.value == SnapshotValue::Debug("3".to_string()))
    );
    assert!(fields.iter().all(|field| field.name != "hover_count"));
    assert!(fields.iter().all(|field| field.name != "_secret"));
}

#[test]
fn component_macro_measure_method_implements_layout() {
    let probe = MeasureMacroProbe {
        label: "measure".to_string(),
    };

    assert!(crate::ui::WidgetComponent::as_layout(&probe).is_some());
    assert_eq!(
        WidgetLayout::measure(&probe, Constraints::loose(Size::new(50.0, 40.0))),
        Size::new(50.0, 40.0)
    );
}

#[test]
fn component_macro_capture_method_implements_event_handler() {
    let mut probe = CaptureMacroProbe {};

    assert!(crate::ui::WidgetComponent::as_event(&probe).is_some());
    assert!(EventHandler::wants_capture_phase(&probe));
    assert_eq!(
        EventHandler::on_event(&mut probe, &SystemEvent::PointerLeave),
        EventResult::NotHandled
    );
}

#[test]
fn component_macro_legacy_name_syntax_reuses_snapshot_metadata() {
    let probe = ComponentMacroProbe {
        label: "Thin".to_string(),
        runtime_counter: 11,
        private_note: "hidden".to_string(),
    };

    let snapshot = ComponentConfigSnapshot::from_component(ComponentId::new(77), &probe);
    let SnapshotFields::Custom { widget, fields } = snapshot.fields else {
        panic!("expected custom snapshot fields");
    };

    assert_eq!(widget, "ComponentMacroProbe");
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "label");
    assert_eq!(
        fields[0].value,
        SnapshotValue::Debug("\"Thin\"".to_string())
    );
}

use std::any::TypeId;

use crate::core::EdgeInsets;
use crate::draw::Color;
use crate::native::traits::input::ControlSize;
use crate::ui::layout::GridTrack;
use crate::ui::widgets::{Button, Container, Grid, Input, Label};
use crate::ui::{
    ComponentConfigSnapshot, EventHandler, SnapshotFields, SnapshotSource, SnapshotValue,
    SystemEvent, WidgetId,
};
use crate::{component, define_widget};

define_widget! {
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
        _tree: &crate::ui::WidgetTree
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
        _tree: &crate::ui::WidgetTree
    ) {}
}

component! {
    struct ComponentStructProbe {
        pub caption: String,
        #[snapshot(skip)]
        #[allow(dead_code)]
        pub runtime_state: usize,
        #[allow(dead_code)]
        cache_key: String,
    }

    render => (
        &self,
        _frame: crate::core::Rect,
        _ctx: &mut crate::draw::painting::PaintContext,
        _tree: &crate::ui::WidgetTree
    ) {}
}

#[test]
fn component_config_snapshot_records_id_type_and_fields() {
    let label = Label::new("status").font_size(18.0).size(80.0, 20.0);
    let snapshot = ComponentConfigSnapshot::from_component(WidgetId::new(7), &label);

    assert_eq!(snapshot.id, WidgetId::new(7));
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
fn button_snapshot_excludes_interaction_state() {
    let mut button = Button::new("Save").block(true);
    let before = button.snapshot_fields();

    let _ = button.on_event(&SystemEvent::PointerEnter);
    let _ = button.on_event(&SystemEvent::PointerDown {
        button: crate::ui::MouseButton::Left,
        pos: crate::core::Point::zero(),
        mods: crate::ui::KeyMod::NONE,
    });

    assert_eq!(button.snapshot_fields(), before);
    assert!(matches!(
        before,
        SnapshotFields::Button {
            ref text,
            disabled: false,
            block: true,
            ..
        } if text == "Save"
    ));
}

#[test]
fn input_snapshot_excludes_live_text_and_cursor_state() {
    let input = Input::new("Search")
        .with_value("runtime text")
        .size(ControlSize::Large)
        .prefix("$")
        .suffix(".rs")
        .password(true)
        .clearable(true)
        .search(true)
        .textarea(true)
        .textarea_rows(4);

    assert_eq!(
        input.snapshot_fields(),
        SnapshotFields::Input {
            placeholder: "Search".to_string(),
            input_size: ControlSize::Large,
            disabled: false,
            prefix: "$".to_string(),
            suffix: ".rs".to_string(),
            addon_before: String::new(),
            addon_after: String::new(),
            password: true,
            password_visible: false,
            clearable: true,
            search: true,
            textarea: true,
            textarea_rows: 4,
        }
    );
}

#[test]
fn container_and_grid_snapshots_capture_layout_config() {
    let container = Container::new()
        .padding(EdgeInsets::uniform(8.0))
        .gap(6.0)
        .w(240.0);
    assert!(matches!(
        container.snapshot_fields(),
        SnapshotFields::Container { ref style }
            if style.padding == EdgeInsets::uniform(8.0)
                && style.gap == 6.0
                && style.width == Some(240.0)
    ));

    let grid = Grid::new()
        .columns(vec![GridTrack::Fr(1.0), GridTrack::Px(120.0)])
        .gap(12.0)
        .bg(Color::blue())
        .size(320.0, 180.0);
    assert!(matches!(
        grid.snapshot_fields(),
        SnapshotFields::Grid {
            ref columns,
            col_gap: 12.0,
            row_gap: 12.0,
            bg_color: Some(color),
            fixed_width: Some(320.0),
            fixed_height: Some(180.0),
            ..
        } if columns == &vec![GridTrack::Fr(1.0), GridTrack::Px(120.0)]
            && color == Color::blue()
    ));
}

#[test]
fn define_widget_auto_snapshot_captures_public_fields_only() {
    let probe = SnapshotProbe {
        title: "Ready".to_string(),
        count: 3,
        hover_count: 7,
        _secret: "runtime".to_string(),
    };

    let snapshot = ComponentConfigSnapshot::from_component(WidgetId::new(42), &probe);

    assert_eq!(snapshot.id, WidgetId::new(42));
    assert_eq!(snapshot.widget_type, TypeId::of::<SnapshotProbe>());
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
fn component_macro_name_struct_syntax_reuses_snapshot_metadata() {
    let probe = ComponentMacroProbe {
        label: "Thin".to_string(),
        runtime_counter: 11,
        private_note: "hidden".to_string(),
    };

    let snapshot = ComponentConfigSnapshot::from_component(WidgetId::new(77), &probe);

    assert_eq!(snapshot.id, WidgetId::new(77));
    assert_eq!(snapshot.widget_type, TypeId::of::<ComponentMacroProbe>());
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

#[test]
fn component_macro_struct_syntax_captures_public_fields() {
    let probe = ComponentStructProbe {
        caption: "Direct".to_string(),
        runtime_state: 9,
        cache_key: "private".to_string(),
    };

    let snapshot = ComponentConfigSnapshot::from_component(WidgetId::new(78), &probe);

    assert_eq!(snapshot.id, WidgetId::new(78));
    assert_eq!(snapshot.widget_type, TypeId::of::<ComponentStructProbe>());
    let SnapshotFields::Custom { widget, fields } = snapshot.fields else {
        panic!("expected custom snapshot fields");
    };

    assert_eq!(widget, "ComponentStructProbe");
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "caption");
    assert_eq!(
        fields[0].value,
        SnapshotValue::Debug("\"Direct\"".to_string())
    );
}

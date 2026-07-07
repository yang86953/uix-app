use std::any::TypeId;

use crate::core::{Constraints, EdgeInsets, Size};
use crate::draw::spatial::PhysicalUnit;
use crate::draw::Color;
use crate::native::traits::input::ControlSize;
use crate::native::traits::system::StatusLevel;
use crate::ui::layout::GridTrack;
use crate::ui::layout::{AlignItems, FlexDirection, JustifyContent};
use crate::ui::widgets::{
    Alert, Avatar, Badge, BadgeStatus, Button, Calendar, Card, Checkbox, Container, Divider,
    DividerDirection, DividerOrientation, Drawer, DrawerPlacement, Empty, FloatButton, Grid, Icon,
    Image, Input, InputNumber, Label, Message, MessageItem, MessagePlacement, Modal,
    NotifPlacement, Notification, Popconfirm, PopconfirmPlacement, Popover, PopoverPlacement,
    PopoverTrigger, ProgressBar, ProgressMode, ProgressType, Radio, RadioDirection, Rate, Skeleton,
    SkeletonShape, Slider, Space, SpaceSize, Spin, SpinSize, Switch, Tag, TagColor, Timeline,
    TimelineItem, Tooltip, TooltipPlacement, TriggerMode, Typography, TypographyType,
};
use crate::ui::{
    ComponentConfigSnapshot, EventHandler, SnapshotFields, SnapshotSource, SnapshotValue,
    SystemEvent, WidgetAnimation, WidgetId, WidgetLayout,
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

define_widget! {
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

component! {
    struct ComponentMeasureProbe {
        pub caption: String,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(96.0, 64.0))
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
fn general_widget_snapshots_capture_static_config() {
    let space = Space::new()
        .vertical()
        .size(SpaceSize::Custom(12.0))
        .wrap(true)
        .justify(JustifyContent::SpaceBetween)
        .align(AlignItems::End)
        .width(320.0)
        .height(48.0)
        .flex_grow(1.0);
    assert_eq!(
        space.snapshot_fields(),
        SnapshotFields::Space {
            direction: FlexDirection::Column,
            space_size: SpaceSize::Custom(12.0),
            wrap: true,
            justify: JustifyContent::SpaceBetween,
            align: AlignItems::End,
            fixed_width: Some(320.0),
            fixed_height: Some(48.0),
            flex_grow: 1.0,
        }
    );

    let divider = Divider::new()
        .with_text("Meta")
        .orientation(DividerOrientation::Left)
        .vertical()
        .color(Color::red())
        .dashed();
    assert_eq!(
        divider.snapshot_fields(),
        SnapshotFields::Divider {
            text: Some("Meta".to_string()),
            orientation: DividerOrientation::Left,
            direction: DividerDirection::Vertical,
            color: Some(Color::red()),
            text_size: 14.0,
            dashed: true,
        }
    );

    assert_eq!(
        Icon::new("settings").size(20.0).snapshot_fields(),
        SnapshotFields::Icon {
            name: "settings".to_string(),
            size: 20.0,
        }
    );
}

#[test]
fn typography_snapshot_excludes_selection_layout_cache() {
    let typography = Typography::heading("Title", 2)
        .disabled(true)
        .mark()
        .code()
        .underline()
        .delete()
        .strong()
        .italic()
        .copyable(true)
        .color(Color::green());

    assert_eq!(
        typography.snapshot_fields(),
        SnapshotFields::Typography {
            content: "Title".to_string(),
            type_: TypographyType::Heading2,
            disabled: true,
            mark: true,
            code: true,
            underline: true,
            delete: true,
            strong: true,
            italic: true,
            copyable: true,
            color_override: Some(Color::green()),
        }
    );
}

#[test]
fn input_control_snapshots_capture_config_not_interaction_state() {
    let mut checkbox = Checkbox::new("Agree").checked(true).disabled(true);
    let before = checkbox.snapshot_fields();
    let _ = checkbox.on_event(&SystemEvent::PointerEnter);
    assert_eq!(checkbox.snapshot_fields(), before);
    assert_eq!(
        before,
        SnapshotFields::Checkbox {
            checked: true,
            disabled: true,
            label: "Agree".to_string(),
        }
    );

    assert_eq!(
        Radio::new()
            .options(vec!["A", "B"])
            .selected(1)
            .disabled(true)
            .vertical()
            .snapshot_fields(),
        SnapshotFields::Radio {
            options: vec!["A".to_string(), "B".to_string()],
            selected: 1,
            disabled: true,
            direction: RadioDirection::Vertical,
            item_h: 24.0,
        }
    );

    let mut switch = Switch::new().checked(true).disabled(true);
    let before = switch.snapshot_fields();
    let _ = switch.on_event(&SystemEvent::PointerEnter);
    assert_eq!(switch.snapshot_fields(), before);
    assert_eq!(
        before,
        SnapshotFields::Switch {
            checked: true,
            disabled: true,
            size: 22.0,
        }
    );
}

#[test]
fn numeric_input_snapshots_exclude_runtime_caches() {
    let mut slider = Slider::new().range(-10.0, 10.0).step(0.5).value(3.0);
    let before = slider.snapshot_fields();
    let _ = slider.on_event(&SystemEvent::PointerMove {
        pos: crate::core::Point::new(20.0, 4.0),
        mods: crate::ui::KeyMod::NONE,
    });
    assert_eq!(slider.snapshot_fields(), before);
    assert_eq!(
        before,
        SnapshotFields::Slider {
            min: -10.0,
            max: 10.0,
            step: 0.5,
            value: 3.0,
        }
    );

    assert_eq!(
        Rate::new()
            .count(7)
            .value(3)
            .allow_half()
            .disabled(true)
            .clearable()
            .character("#")
            .snapshot_fields(),
        SnapshotFields::Rate {
            count: 7,
            value: 3,
            half: true,
            disabled: true,
            clearable: true,
            character: "#".to_string(),
        }
    );

    assert_eq!(
        InputNumber::new("amount")
            .min(-5.0)
            .max(8.0)
            .step(0.25)
            .value(2.5)
            .snapshot_fields(),
        SnapshotFields::InputNumber {
            value: 2.5,
            min: -5.0,
            max: 8.0,
            step: 0.25,
            placeholder: "amount".to_string(),
            disabled: false,
        }
    );
}

#[test]
fn display_widget_snapshots_capture_static_config() {
    assert_eq!(
        Avatar::new("AB")
            .size(40.0)
            .bg(Color::blue())
            .text_color(Color::white())
            .square(true)
            .src("avatar.png")
            .snapshot_fields(),
        SnapshotFields::Avatar {
            text: "AB".to_string(),
            size: 40.0,
            bg_color: Some(Color::blue()),
            text_color: Some(Color::white()),
            square: true,
            src: "avatar.png".to_string(),
        }
    );

    assert_eq!(
        Badge::new()
            .count(120)
            .max(9)
            .dot()
            .color(Color::red())
            .status(BadgeStatus::Warning)
            .show_zero(true)
            .text("hot")
            .offset(2.0, 4.0)
            .offset_unit(PhysicalUnit::Mm(2.0), PhysicalUnit::Pt(3.0))
            .snapshot_fields(),
        SnapshotFields::Badge {
            count: 1,
            max: 9,
            dot: true,
            color: Some(Color::red()),
            size: 16.0,
            status: Some(BadgeStatus::Warning),
            show_zero: true,
            text: "hot".to_string(),
            offset_x: 2.0,
            offset_y: 4.0,
            offset_unit: Some((PhysicalUnit::Mm(2.0), PhysicalUnit::Pt(3.0))),
        }
    );

    assert_eq!(
        Empty::new()
            .description("Nothing")
            .icon("search")
            .image("file")
            .snapshot_fields(),
        SnapshotFields::Empty {
            description: "Nothing".to_string(),
            icon_name: "search".to_string(),
            image: "file".to_string(),
        }
    );
}

#[test]
fn card_and_float_button_snapshots_exclude_hover_state() {
    let mut card = Card::new()
        .title("Panel")
        .bordered(false)
        .hoverable()
        .size(320.0, 180.0)
        .padding(20.0)
        .elevation(3)
        .flex_grow(2.0)
        .actions(vec!["Edit", "Delete"]);
    let before = card.snapshot_fields();
    let _ = card.on_event(&SystemEvent::PointerEnter);
    assert_eq!(card.snapshot_fields(), before);
    assert_eq!(
        before,
        SnapshotFields::Card {
            title: Some("Panel".to_string()),
            bordered: false,
            hoverable: true,
            fixed_width: Some(320.0),
            fixed_height: Some(180.0),
            padding: 20.0,
            elevation: 3,
            flex_grow: 2.0,
            actions: vec!["Edit".to_string(), "Delete".to_string()],
        }
    );

    let mut float_button = FloatButton::new("+")
        .tooltip("Create")
        .badge(7)
        .position(12.0, 24.0)
        .size(48.0);
    let before = float_button.snapshot_fields();
    let _ = float_button.on_event(&SystemEvent::PointerEnter);
    assert_eq!(float_button.snapshot_fields(), before);
    assert_eq!(
        before,
        SnapshotFields::FloatButton {
            icon: "+".to_string(),
            tooltip: "Create".to_string(),
            badge_count: 7,
            size: 48.0,
            x: 12.0,
            y: 24.0,
        }
    );
}

#[test]
fn media_and_tag_snapshots_capture_config() {
    let image = Image::new(120.0, 80.0)
        .src("hero.png")
        .alt("Hero")
        .fallback("Missing")
        .radius(8.0)
        .preview(false)
        .fit(false);
    assert_eq!(
        image.snapshot_fields(),
        SnapshotFields::Image {
            src: "hero.png".to_string(),
            alt: "Hero".to_string(),
            fallback: "Missing".to_string(),
            width: 120.0,
            height: 80.0,
            radius: 8.0,
            preview: false,
            fit: false,
        }
    );
    assert_eq!(
        image.measure(Constraints::loose(Size::new(90.0, 60.0))),
        Size::new(90.0, 60.0)
    );

    assert_eq!(
        Tag::new("stable")
            .color(TagColor::Success)
            .custom_color(Color::green())
            .closable()
            .checkable(true)
            .font_size(14.0)
            .snapshot_fields(),
        SnapshotFields::Tag {
            text: "stable".to_string(),
            color: TagColor::Success,
            closable: true,
            font_size: 14.0,
            custom_color: Some(Color::green()),
            checkable: true,
        }
    );
}

#[test]
fn timeline_calendar_and_skeleton_snapshots_capture_static_config() {
    let item = TimelineItem::new("Build")
        .description("done")
        .color(Color::green());
    let timeline = Timeline::new()
        .add(item.clone())
        .pending(true)
        .reverse(true);
    assert_eq!(
        timeline.snapshot_fields(),
        SnapshotFields::Timeline {
            items: vec![item],
            pending: true,
            reverse: true,
        }
    );
    assert_eq!(
        timeline.measure(Constraints::loose(Size::new(180.0, 40.0))),
        Size::new(180.0, 40.0)
    );

    let mut calendar = Calendar::new().cell_size(32.0).year_jump(true);
    let before = calendar.snapshot_fields();
    let _ = calendar.on_event(&SystemEvent::PointerDown {
        button: crate::ui::MouseButton::Left,
        pos: crate::core::Point::new(8.0, 8.0),
        mods: crate::ui::KeyMod::NONE,
    });
    assert_eq!(calendar.snapshot_fields(), before);
    assert_eq!(
        before,
        SnapshotFields::Calendar {
            cell_size: 32.0,
            year_jump: true,
        }
    );
    assert_eq!(
        calendar.measure(Constraints::loose(Size::new(160.0, 120.0))),
        Size::new(160.0, 120.0)
    );

    assert_eq!(
        Skeleton::new()
            .shape(SkeletonShape::Circle)
            .size(64.0, 64.0)
            .snapshot_fields(),
        SnapshotFields::Skeleton {
            shape: SkeletonShape::Circle,
            width: 64.0,
            height: 64.0,
        }
    );
}

#[test]
fn feedback_snapshots_capture_static_config() {
    assert_eq!(
        Alert::new("Saved")
            .description("Done")
            .type_(StatusLevel::Success)
            .closable()
            .snapshot_fields(),
        SnapshotFields::Alert {
            message: "Saved".to_string(),
            description: "Done".to_string(),
            type_: StatusLevel::Success,
            closable: true,
            show_icon: true,
        }
    );

    assert_eq!(
        Message::new()
            .placement(MessagePlacement::TopRight)
            .snapshot_fields(),
        SnapshotFields::Message {
            placement: MessagePlacement::TopRight,
        }
    );

    assert_eq!(
        Notification::new()
            .placement(NotifPlacement::BottomLeft)
            .snapshot_fields(),
        SnapshotFields::Notification {
            placement: NotifPlacement::BottomLeft,
        }
    );
}

#[test]
fn message_and_notification_snapshots_exclude_runtime_queues() {
    let message = Message::new().placement(MessagePlacement::TopLeft);
    let before = message.snapshot_fields();
    message.add(MessageItem {
        type_: StatusLevel::Warning,
        content: "Queued".to_string(),
        duration_ms: 2500,
        closable: false,
    });
    assert_eq!(message.snapshot_fields(), before);

    let notification = Notification::new().placement(NotifPlacement::TopLeft);
    let before = notification.snapshot_fields();
    notification.open("Deploy", "Done", StatusLevel::Info);
    assert_eq!(notification.snapshot_fields(), before);
}

#[test]
fn progress_and_spin_snapshots_exclude_animation_phase() {
    let mut progress = ProgressBar::new()
        .size(240.0, 12.0)
        .progress(0.4)
        .stroke_color(Color::green())
        .track_color(Color::blue())
        .circle();
    let before = progress.snapshot_fields();
    assert!(!WidgetAnimation::update_animation(&mut progress, 0.25));
    assert_eq!(progress.snapshot_fields(), before);
    assert_eq!(
        before,
        SnapshotFields::ProgressBar {
            progress: 0.4,
            mode: ProgressMode::Determinate(0.4),
            stroke_color: Some(Color::green()),
            track_color: Some(Color::blue()),
            height: 12.0,
            width: 240.0,
            round: true,
            progress_type: ProgressType::Circle,
        }
    );

    let mut spin = Spin::new()
        .large()
        .color(Color::red())
        .spinning(true)
        .tip("Loading")
        .wrapper_mode();
    let before = spin.snapshot_fields();
    assert!(WidgetAnimation::update_animation(&mut spin, 0.25));
    assert_eq!(spin.snapshot_fields(), before);
    assert_eq!(
        before,
        SnapshotFields::Spin {
            size: SpinSize::Large,
            color: Some(Color::red()),
            spinning: true,
            tip: "Loading".to_string(),
            wrapper_mode: true,
        }
    );
}

#[test]
fn floating_feedback_snapshots_exclude_visibility_and_transition_state() {
    let mut tooltip = Tooltip::new("Help")
        .placement(TooltipPlacement::Bottom)
        .trigger(TriggerMode::Click)
        .bg_color(Color::blue())
        .text_color(Color::white())
        .arrow(false)
        .delay_ms(150)
        .timer_id(8);
    let before = tooltip.snapshot_fields();
    tooltip.open();
    assert_eq!(tooltip.snapshot_fields(), before);
    assert_eq!(
        before,
        SnapshotFields::Tooltip {
            text: "Help".to_string(),
            placement: TooltipPlacement::Bottom,
            trigger: TriggerMode::Click,
            bg_color: Some(Color::blue()),
            text_color: Some(Color::white()),
            delay_ms: 150,
            timer_id: 8,
            arrow: false,
        }
    );

    let mut popover = Popover::new("Body")
        .title("Title")
        .placement(PopoverPlacement::RightBottom)
        .trigger(PopoverTrigger::Hover)
        .arrow(false);
    let before = popover.snapshot_fields();
    popover.open();
    assert_eq!(popover.snapshot_fields(), before);

    let mut popconfirm = Popconfirm::new()
        .title("Delete?")
        .confirm_text("Yes")
        .cancel_text("No")
        .placement(PopconfirmPlacement::BottomRight)
        .arrow(false)
        .icon(false);
    let before = popconfirm.snapshot_fields();
    popconfirm.open();
    assert_eq!(popconfirm.snapshot_fields(), before);
}

#[test]
fn modal_and_drawer_snapshots_capture_config_not_runtime_visibility() {
    let mut modal = Modal::new("Dialog")
        .modal_size(ControlSize::Large)
        .size(640.0, 360.0)
        .closable(false)
        .mask_closable(false)
        .footer_visible(false)
        .centered(false)
        .overlay(true);
    let before = modal.snapshot_fields();
    modal.open();
    assert_eq!(modal.snapshot_fields(), before);
    assert_eq!(
        before,
        SnapshotFields::Modal {
            title: "Dialog".to_string(),
            width: 640.0,
            height: 360.0,
            modal_size: ControlSize::Large,
            closable: false,
            mask_closable: false,
            footer_visible: false,
            centered: false,
            overlay: true,
        }
    );

    let mut drawer = Drawer::new("Panel")
        .drawer_size(ControlSize::Small)
        .size(420.0, 260.0)
        .placement(DrawerPlacement::Left)
        .closable(false)
        .mask_closable(false)
        .mask(false)
        .footer_visible(true)
        .extra("More");
    let before = drawer.snapshot_fields();
    drawer.open();
    assert_eq!(drawer.snapshot_fields(), before);
    assert_eq!(
        before,
        SnapshotFields::Drawer {
            title: "Panel".to_string(),
            width: 420.0,
            height: 260.0,
            drawer_size: ControlSize::Small,
            placement: DrawerPlacement::Left,
            closable: false,
            mask_closable: false,
            mask: false,
            footer_visible: true,
            extra: "More".to_string(),
        }
    );
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
fn define_widget_measure_method_implements_layout() {
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

#[test]
fn component_macro_measure_method_implements_layout() {
    let probe = ComponentMeasureProbe {
        caption: "measure".to_string(),
    };

    assert!(crate::ui::WidgetComponent::as_layout(&probe).is_some());
    assert_eq!(
        WidgetLayout::measure(&probe, Constraints::loose(Size::new(80.0, 40.0))),
        Size::new(80.0, 40.0)
    );
}

use std::any::TypeId;
use std::cell::Cell;
use std::rc::Rc;

use crate::component;
use crate::core::{Constraints, EdgeInsets, Point, Size};
use crate::draw::spatial::PhysicalUnit;
use crate::draw::Color;
use crate::native::traits::input::{ControlSize, KeyMod, MouseButton, ScrollDirection};
use crate::native::traits::system::StatusLevel;
use crate::ui::layout::GridTrack;
use crate::ui::layout::{AlignItems, FlexDirection, JustifyContent};
use crate::ui::widgets::{
    Affix, Alert, Anchor, AnchorItem, AutoComplete, Avatar, BackTop, Badge, BadgeStatus, BarChart,
    BarData, Breadcrumb, BreadcrumbItem, Button, Calendar, Card, Carousel, Cascader,
    CascaderOption, Checkbox, Collapse, CollapsePanel, ColorPicker, Container, Content, DatePicker,
    DateValue, Descriptions, DescriptionsItem, Divider, DividerDirection, DividerOrientation,
    Drawer, DrawerPlacement, Dropdown, Empty, FloatButton, Footer, Form, FormItem, FormLayout,
    Grid, Header, Icon, Image, Input, InputNumber, Label, Layout, LineChart, LineData, List,
    Mentions, Menu, MenuItem, MenuMode, Message, MessageItem, MessagePlacement, Modal, NavItem,
    NotifPlacement, Notification, OptGroup, Pagination, PieChart, PieData, Popconfirm,
    PopconfirmPlacement, Popover, PopoverPlacement, PopoverTrigger, ProgressBar, ProgressMode,
    ProgressType, QRCode, Radio, RadioDirection, Rate, Result as ResultWidget, ResultType,
    RichText, RichTextSegment, RichTextStyle, ScrollView, Segmented, Select, SelectableItem,
    SelectableList, Sider, Skeleton, SkeletonShape, Slider, SortDirection, Space, SpaceSize, Spin,
    SpinSize, Splitter, Step, StepStatus, Steps, Switch, Tab, TabPosition, Table, TableColumn,
    Tabs, Tag, TagColor, ThemeToggle, TimePicker, TimeValue, Timeline, TimelineItem, Tooltip,
    TooltipPlacement, Transfer, TransferItem, Tree, TreeNode, TreeSelect, TriggerMode, Typography,
    TypographyType, Upload, ValidateStatus, Watermark,
};
use crate::ui::ComponentId;
use crate::ui::{
    ComponentConfigSnapshot, EventHandler, SnapshotCollapsePanel, SnapshotFields, SnapshotSource,
    SnapshotTableColumn, SnapshotTransferItem, SnapshotTreeNode, SnapshotValue, SystemEvent,
    WidgetAnimation, WidgetLayout,
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
        _tree: &crate::ui::core::widget::WidgetTree
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
fn component_config_snapshot_uses_typed_fields_for_component_builtin() {
    let qrcode = QRCode::new("uix").size(96.0).error_level(2);
    let snapshot = ComponentConfigSnapshot::from_component(ComponentId::new(8), &qrcode);

    assert_eq!(snapshot.id, ComponentId::new(8));
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
        SnapshotFields::Grid { ref style }
            if style.grid_template_columns == vec![GridTrack::Fr(1.0), GridTrack::Px(120.0)]
                && style.grid_column_gap == 12.0
                && style.grid_row_gap == 12.0
                && style.background == Some(crate::ui::style::ColorValue::Custom(Color::blue()))
                && style.width == Some(320.0)
                && style.height == Some(180.0)
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
fn other_widget_snapshots_capture_static_config() {
    let style = RichTextStyle {
        bold: true,
        font_size: Some(18.0),
        color: Some(Color::red()),
        ..RichTextStyle::default()
    };
    let segments = vec![
        RichTextSegment::Text {
            content: "Hello".to_string(),
            style: style.clone(),
        },
        RichTextSegment::Code {
            content: "uix".to_string(),
        },
        RichTextSegment::Link {
            content: "docs".to_string(),
            url: "https://example.test".to_string(),
        },
        RichTextSegment::NewLine,
    ];
    let rich_text = RichText::new()
        .content(segments.clone())
        .font_size(16.0)
        .font_size_unit(PhysicalUnit::Pt(12.0))
        .color(Color::green());
    assert_eq!(
        ComponentConfigSnapshot::from_component(ComponentId::new(91), &rich_text).fields,
        SnapshotFields::RichText {
            segments,
            default_font_size: 16.0,
            default_font_size_unit: Some(PhysicalUnit::Pt(12.0)),
            default_color: Color::green(),
        }
    );

    let mut theme_toggle = ThemeToggle::new().dark(true);
    let before = ComponentConfigSnapshot::from_component(ComponentId::new(9), &theme_toggle).fields;
    let _ = theme_toggle.on_event(&SystemEvent::PointerDown {
        button: MouseButton::Left,
        pos: Point::zero(),
        mods: KeyMod::NONE,
    });
    assert_eq!(
        ComponentConfigSnapshot::from_component(ComponentId::new(9), &theme_toggle).fields,
        before
    );
    assert_eq!(before, SnapshotFields::ThemeToggle { dark: true });

    let watermark = Watermark::new("draft")
        .color(Color::blue())
        .font_size(18.0)
        .opacity(0.25)
        .rotate(-15.0)
        .gap(120.0, 90.0)
        .offset(4.0, 8.0);
    assert_eq!(
        ComponentConfigSnapshot::from_component(ComponentId::new(92), &watermark).fields,
        SnapshotFields::Watermark {
            text: "draft".to_string(),
            color: Color::blue(),
            font_size: 18.0,
            opacity: 0.25,
            rotate: -15.0,
            gap_x: 120.0,
            gap_y: 90.0,
            x_offset: 4.0,
            y_offset: 8.0,
        }
    );
}

#[test]
fn transfer_and_upload_snapshots_exclude_runtime_state() {
    let mut transfer = Transfer::new()
        .source(vec![TransferItem {
            key: "a".to_string(),
            title: "Alpha".to_string(),
            selected: true,
        }])
        .target(vec![TransferItem {
            key: "b".to_string(),
            title: "Beta".to_string(),
            selected: false,
        }]);
    let before = ComponentConfigSnapshot::from_component(ComponentId::new(93), &transfer).fields;
    let _ = transfer.on_event(&SystemEvent::PointerDown {
        button: MouseButton::Left,
        pos: Point::new(228.0, 88.0),
        mods: KeyMod::NONE,
    });
    assert_eq!(
        ComponentConfigSnapshot::from_component(ComponentId::new(93), &transfer).fields,
        before
    );
    assert_eq!(
        before,
        SnapshotFields::Transfer {
            source: vec![SnapshotTransferItem {
                key: "a".to_string(),
                title: "Alpha".to_string(),
            }],
            target: vec![SnapshotTransferItem {
                key: "b".to_string(),
                title: "Beta".to_string(),
            }],
        }
    );

    let mut upload = Upload::new()
        .accept(".png")
        .multiple(true)
        .drag(false)
        .max_count(2);
    let before = ComponentConfigSnapshot::from_component(ComponentId::new(94), &upload).fields;
    upload.add_file("hero.png");
    upload.update_progress(0, 0.5);
    let _ = upload.on_event(&SystemEvent::PointerDown {
        button: MouseButton::Left,
        pos: Point::zero(),
        mods: KeyMod::NONE,
    });
    assert_eq!(
        ComponentConfigSnapshot::from_component(ComponentId::new(94), &upload).fields,
        before
    );
    assert_eq!(
        before,
        SnapshotFields::Upload {
            accept: ".png".to_string(),
            multiple: true,
            drag: false,
            max_count: 2,
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
fn layout_container_snapshots_capture_static_config() {
    assert_eq!(
        Layout::new().bg(Color::blue()).snapshot_fields(),
        SnapshotFields::Layout {
            bg_color: Some(Color::blue()),
        }
    );

    assert_eq!(
        Header::new(64.0).bg(Color::red()).snapshot_fields(),
        SnapshotFields::Header {
            height: 64.0,
            bg_color: Some(Color::red()),
        }
    );

    assert_eq!(
        Sider::new(220.0)
            .bg(Color::green())
            .collapsible(true)
            .collapsed(true)
            .collapsed_width(72.0)
            .snapshot_fields(),
        SnapshotFields::Sider {
            width: 220.0,
            bg_color: Some(Color::green()),
            collapsible: true,
            collapsed: true,
            collapsed_width: 72.0,
        }
    );

    assert_eq!(
        Content::new().bg(Color::white()).snapshot_fields(),
        SnapshotFields::Content {
            bg_color: Some(Color::white()),
        }
    );

    assert_eq!(
        Footer::new(48.0).bg(Color::black()).snapshot_fields(),
        SnapshotFields::Footer {
            height: 48.0,
            bg_color: Some(Color::black()),
        }
    );
}

#[test]
fn container_navigation_snapshots_capture_static_config() {
    assert_eq!(
        Splitter::new()
            .panels(3)
            .vertical(true)
            .min_size(1, 80.0)
            .snapshot_fields(),
        SnapshotFields::Splitter {
            vertical: true,
            panel_count: 3,
            min_sizes: vec![50.0, 80.0, 50.0],
            handle_size: 6.0,
        }
    );

    assert_eq!(
        Affix::new(24.0).snapshot_fields(),
        SnapshotFields::Affix { offset_top: 24.0 }
    );

    assert_eq!(
        BackTop::new().visibility_height(320.0).snapshot_fields(),
        SnapshotFields::BackTop {
            visibility_height: 320.0,
        }
    );

    let items = vec![
        BreadcrumbItem::new("Home"),
        BreadcrumbItem::new("Docs").active(),
    ];
    assert_eq!(
        Breadcrumb::new()
            .items(items.clone())
            .separator(">")
            .snapshot_fields(),
        SnapshotFields::Breadcrumb {
            items,
            separator: ">".to_string(),
        }
    );

    assert_eq!(
        Pagination::new(240, 20)
            .current(3)
            .show_total(false)
            .show_size_changer(true)
            .item_size(32.0)
            .page_size_options(vec![10, 20, 50])
            .snapshot_fields(),
        SnapshotFields::Pagination {
            total: 240,
            page_size: 20,
            show_size_changer: true,
            show_total: false,
            size: 32.0,
            page_size_options: vec![10, 20, 50],
        }
    );
}

#[test]
fn container_navigation_snapshots_exclude_runtime_state() {
    let mut affix = Affix::new(12.0);
    let before = affix.snapshot_fields();
    affix.set_child_bounds(40.0, 80.0);
    affix.update_scroll(32.0);
    assert_eq!(affix.snapshot_fields(), before);

    let mut back_top = BackTop::new().visibility_height(100.0);
    let before = back_top.snapshot_fields();
    back_top.update_visibility(200.0);
    assert_eq!(back_top.snapshot_fields(), before);

    let pagination = Pagination::new(120, 10).current(2);
    let before = pagination.snapshot_fields();
    pagination.set_current(5);
    assert_eq!(pagination.snapshot_fields(), before);
}

#[test]
fn navigation_snapshots_capture_static_config() {
    let anchor_items = vec![
        AnchorItem::new("Intro", "#intro"),
        AnchorItem::new("API", "#api"),
    ];
    assert_eq!(
        Anchor::new(anchor_items.clone())
            .set_offset_top(48.0)
            .bg(Color::white())
            .snapshot_fields(),
        SnapshotFields::Anchor {
            items: anchor_items,
            offset_top: 48.0,
            bg_color: Some(Color::white()),
        }
    );

    let menu_items = vec![
        MenuItem {
            key: "home".to_string(),
            label: "Home".to_string(),
            icon: "house".to_string(),
            disabled: false,
        },
        MenuItem {
            key: "docs".to_string(),
            label: "Docs".to_string(),
            icon: String::new(),
            disabled: true,
        },
    ];
    assert_eq!(
        Menu::new()
            .items(menu_items.clone())
            .mode(MenuMode::Vertical)
            .active_key("home")
            .item_height(40.0)
            .snapshot_fields(),
        SnapshotFields::Menu {
            items: menu_items,
            mode: MenuMode::Vertical,
            item_h: 40.0,
        }
    );

    assert_eq!(
        Dropdown::new("More")
            .items(vec!["Edit", "Delete"])
            .snapshot_fields(),
        SnapshotFields::Dropdown {
            label: "More".to_string(),
            items: vec!["Edit".to_string(), "Delete".to_string()],
        }
    );

    let tabs = vec![
        Tab {
            label: "One".to_string(),
            key: "one".to_string(),
        },
        Tab {
            label: "Two".to_string(),
            key: "two".to_string(),
        },
    ];
    assert_eq!(
        Tabs::new()
            .tabs(tabs.clone())
            .active(1)
            .position(TabPosition::Bottom)
            .size(420.0, 240.0)
            .snapshot_fields(),
        SnapshotFields::Tabs {
            tabs,
            position: TabPosition::Bottom,
            tab_height: 40.0,
            fixed_width: Some(420.0),
            fixed_height: Some(240.0),
        }
    );
}

#[test]
fn navigation_runtime_state_is_excluded_from_snapshots() {
    let mut anchor = Anchor::new(vec![
        AnchorItem::new("Intro", "#intro"),
        AnchorItem::new("API", "#api"),
    ]);
    let before = anchor.snapshot_fields();
    anchor.set_positions(vec![0.0, 200.0]);
    anchor.update_active(240.0);
    assert_eq!(anchor.snapshot_fields(), before);

    let mut menu = Menu::new()
        .add_item(MenuItem {
            key: "home".to_string(),
            label: "Home".to_string(),
            icon: String::new(),
            disabled: false,
        })
        .add_item(MenuItem {
            key: "docs".to_string(),
            label: "Docs".to_string(),
            icon: String::new(),
            disabled: false,
        });
    let before = menu.snapshot_fields();
    menu.set_active_key("docs");
    assert_eq!(menu.snapshot_fields(), before);

    let mut dropdown = Dropdown::new("More").items(vec!["Edit", "Delete"]);
    let before = dropdown.snapshot_fields();
    dropdown.open();
    assert_eq!(dropdown.snapshot_fields(), before);
}

#[test]
fn navigation_and_display_snapshots_capture_static_config() {
    let active = Rc::new(Cell::new(0));
    assert_eq!(
        NavItem::new("Home", 2, active)
            .icon("house")
            .width(180.0)
            .height(44.0)
            .compact(true)
            .snapshot_fields(),
        SnapshotFields::NavItem {
            label: "Home".to_string(),
            icon: "house".to_string(),
            fixed_width: 180.0,
            fixed_height: 44.0,
            index: 2,
            compact: true,
        }
    );

    let steps = vec![
        Step::new("Start").status(StepStatus::Finish),
        Step::new("Ship")
            .description("Deploy")
            .status(StepStatus::Process),
    ];
    assert_eq!(
        Steps::new(steps.clone()).current(1).snapshot_fields(),
        SnapshotFields::Steps {
            steps,
            direction: true,
        }
    );

    let tree_nodes = vec![TreeNode::new("Root", "root")
        .icon("folder")
        .checkable(true)
        .draggable(true)
        .children(vec![TreeNode::new("Leaf", "leaf").disabled(true)])];
    assert_eq!(
        Tree::new(tree_nodes).multiple(true).snapshot_fields(),
        SnapshotFields::Tree {
            nodes: vec![SnapshotTreeNode {
                title: "Root".to_string(),
                key: "root".to_string(),
                icon: "folder".to_string(),
                children: vec![SnapshotTreeNode {
                    title: "Leaf".to_string(),
                    key: "leaf".to_string(),
                    icon: String::new(),
                    children: Vec::new(),
                    disabled: true,
                    checkable: false,
                    draggable: false,
                    is_leaf: true,
                }],
                disabled: false,
                checkable: true,
                draggable: true,
                is_leaf: false,
            }],
            multiple: true,
        }
    );

    assert_eq!(
        List::new()
            .header("Header")
            .footer("Footer")
            .bordered(false)
            .size(ControlSize::Small)
            .items(vec!["One", "Two"])
            .load_more("More")
            .snapshot_fields(),
        SnapshotFields::List {
            header: "Header".to_string(),
            footer: "Footer".to_string(),
            bordered: false,
            list_size: ControlSize::Small,
            items: vec!["One".to_string(), "Two".to_string()],
            load_more_text: "More".to_string(),
        }
    );

    assert_eq!(
        Collapse::new()
            .panels(vec![
                CollapsePanel::new("A", "Alpha").expanded(),
                CollapsePanel::new("B", "Beta"),
            ])
            .accordion()
            .snapshot_fields(),
        SnapshotFields::Collapse {
            panels: vec![
                SnapshotCollapsePanel {
                    header: "A".to_string(),
                    content: "Alpha".to_string(),
                },
                SnapshotCollapsePanel {
                    header: "B".to_string(),
                    content: "Beta".to_string(),
                },
            ],
            accordion: true,
        }
    );

    assert_eq!(
        Carousel::new()
            .show_dots(false)
            .show_arrows(false)
            .snapshot_fields(),
        SnapshotFields::Carousel {
            show_dots: false,
            show_arrows: false,
        }
    );
}

#[test]
fn input_selection_snapshots_capture_static_config() {
    let groups = vec![OptGroup::new("Letters").add("A").add("B")];
    assert_eq!(
        Select::new()
            .options(vec!["Loose"])
            .optgroups(groups.clone())
            .selected(1)
            .placeholder("Pick")
            .disabled(true)
            .multiple(true)
            .search(true)
            .snapshot_fields(),
        SnapshotFields::Select {
            options: vec!["Loose".to_string()],
            optgroups: groups,
            disabled: true,
            placeholder: "Pick".to_string(),
            multiple: true,
            search: true,
        }
    );

    assert_eq!(
        AutoComplete::new()
            .placeholder("City")
            .options(vec!["Paris", "Prague"])
            .snapshot_fields(),
        SnapshotFields::AutoComplete {
            placeholder: "City".to_string(),
            options: vec!["Paris".to_string(), "Prague".to_string()],
        }
    );

    let nodes = vec![TreeNode::new("Root", "root")
        .checkable(true)
        .children(vec![TreeNode::new("Leaf", "leaf").disabled(true)])];
    assert_eq!(
        TreeSelect::new()
            .placeholder("Node")
            .nodes(nodes)
            .snapshot_fields(),
        SnapshotFields::TreeSelect {
            placeholder: "Node".to_string(),
            nodes: vec![SnapshotTreeNode {
                title: "Root".to_string(),
                key: "root".to_string(),
                icon: String::new(),
                children: vec![SnapshotTreeNode {
                    title: "Leaf".to_string(),
                    key: "leaf".to_string(),
                    icon: String::new(),
                    children: Vec::new(),
                    disabled: true,
                    checkable: false,
                    draggable: false,
                    is_leaf: true,
                }],
                disabled: false,
                checkable: true,
                draggable: false,
                is_leaf: false,
            }],
        }
    );

    let cascader_options = vec![CascaderOption::new("Asia", "asia")
        .children(vec![CascaderOption::new("China", "cn").disabled(true)])];
    assert_eq!(
        Cascader::new(cascader_options.clone(), "Region").snapshot_fields(),
        SnapshotFields::Cascader {
            options: cascader_options,
            placeholder: "Region".to_string(),
        }
    );
}

#[test]
fn input_picker_snapshots_capture_static_config() {
    let SnapshotFields::ColorPicker { preset_colors } =
        ColorPicker::new(Color::red()).snapshot_fields()
    else {
        panic!("expected ColorPicker snapshot");
    };
    assert!(!preset_colors.is_empty());

    assert_eq!(
        DatePicker::new("Date")
            .value(DateValue::new(2026, 7, 7))
            .snapshot_fields(),
        SnapshotFields::DatePicker {
            placeholder: "Date".to_string(),
        }
    );

    assert_eq!(
        TimePicker::new("Time")
            .value(TimeValue::new(9, 30))
            .snapshot_fields(),
        SnapshotFields::TimePicker {
            placeholder: "Time".to_string(),
        }
    );

    assert_eq!(
        Mentions::new("Mention")
            .options(vec!["Ada", "Grace"])
            .snapshot_fields(),
        SnapshotFields::Mentions {
            placeholder: "Mention".to_string(),
            options: vec!["Ada".to_string(), "Grace".to_string()],
        }
    );

    assert_eq!(
        Segmented::new()
            .options(vec!["Daily", "Weekly", "Monthly"])
            .selected(2)
            .disabled(true)
            .disable_option(1)
            .snapshot_fields(),
        SnapshotFields::Segmented {
            options: vec![
                "Daily".to_string(),
                "Weekly".to_string(),
                "Monthly".to_string(),
            ],
            disabled: true,
            disabled_options: vec![false, true],
        }
    );

    assert_eq!(
        FormItem::new("Name")
            .name("name")
            .required(true)
            .status(ValidateStatus::Error)
            .help("Required")
            .label_width(96.0)
            .layout(FormLayout::Vertical)
            .snapshot_fields(),
        SnapshotFields::FormItem {
            label: "Name".to_string(),
            name: "name".to_string(),
            required: true,
            help: "Required".to_string(),
            label_width: 96.0,
            layout: FormLayout::Vertical,
        }
    );
}

#[test]
fn input_snapshots_exclude_runtime_selection_popup_and_validation_state() {
    let mut select = Select::new().options(vec!["A", "B"]);
    let before = select.snapshot_fields();
    select.open();
    assert_eq!(select.snapshot_fields(), before);

    let mut autocomplete = AutoComplete::new()
        .placeholder("Search")
        .options(vec!["Ada", "Grace"]);
    let before = autocomplete.snapshot_fields();
    autocomplete.set_value("Ada");
    autocomplete.open();
    assert_eq!(autocomplete.snapshot_fields(), before);

    let mut tree_select = TreeSelect::new().nodes(vec![TreeNode::new("Root", "root")]);
    let before = tree_select.snapshot_fields();
    tree_select.open();
    assert_eq!(tree_select.snapshot_fields(), before);

    let mut cascader = Cascader::new(vec![CascaderOption::new("Root", "root")], "Path");
    let before = cascader.snapshot_fields();
    cascader.open();
    cascader.select_option(0, 0);
    assert_eq!(cascader.snapshot_fields(), before);

    let mut color_picker = ColorPicker::new(Color::red());
    let before = color_picker.snapshot_fields();
    color_picker.open();
    color_picker.set_value(Color::blue());
    assert_eq!(color_picker.snapshot_fields(), before);

    let mut date_picker = DatePicker::new("Date").value(DateValue::new(2026, 7, 7));
    let before = date_picker.snapshot_fields();
    date_picker.set_value(DateValue::new(2026, 8, 1));
    assert_eq!(date_picker.snapshot_fields(), before);

    let mut time_picker = TimePicker::new("Time").value(TimeValue::new(9, 30));
    let before = time_picker.snapshot_fields();
    time_picker.set_value(TimeValue::new(10, 45));
    assert_eq!(time_picker.snapshot_fields(), before);

    let selected_zero = Segmented::new()
        .options(vec!["Daily", "Weekly"])
        .selected(0)
        .snapshot_fields();
    let selected_one = Segmented::new()
        .options(vec!["Daily", "Weekly"])
        .selected(1)
        .snapshot_fields();
    assert_eq!(selected_one, selected_zero);

    let mut form_item = FormItem::new("Email").help("Shown");
    let before = form_item.snapshot_fields();
    form_item.set_status(ValidateStatus::Error);
    assert_eq!(form_item.snapshot_fields(), before);
}

#[test]
fn data_display_and_other_snapshots_capture_static_config() {
    assert_eq!(
        Form::new()
            .label_width(104.0)
            .gap(12.0)
            .layout(FormLayout::Inline)
            .snapshot_fields(),
        SnapshotFields::Form {
            label_width: 104.0,
            gap: 12.0,
            layout: FormLayout::Inline,
        }
    );

    let description_items = vec![
        DescriptionsItem::new("Name", "Ada"),
        DescriptionsItem::new("Role", "Engineer").span(2),
    ];
    assert_eq!(
        Descriptions::new()
            .title("Profile")
            .items(description_items.clone())
            .bordered(true)
            .column(2)
            .label_width(88.0)
            .size(ControlSize::Small)
            .snapshot_fields(),
        SnapshotFields::Descriptions {
            title: "Profile".to_string(),
            items: description_items,
            bordered: true,
            column: 2,
            label_width: 88.0,
            descriptions_size: ControlSize::Small,
        }
    );

    assert_eq!(
        ResultWidget::new(ResultType::Success)
            .title("Done")
            .subtitle("All set")
            .extra_text("Continue")
            .snapshot_fields(),
        SnapshotFields::Result {
            result_type: ResultType::Success,
            title: "Done".to_string(),
            subtitle: "All set".to_string(),
            extra_text: "Continue".to_string(),
        }
    );

    let mut name_col = TableColumn::new("Name", 120.0).sortable(true);
    name_col.sort_direction = SortDirection::Desc;
    name_col.filters = vec![("Active".to_string(), true), ("Paused".to_string(), false)];
    let rows = vec![vec!["Ada".to_string(), "Active".to_string()]];
    assert_eq!(
        Table::new()
            .columns(vec![name_col])
            .rows(rows.clone())
            .row_height(36.0)
            .empty_text("No rows")
            .expandable(72.0, |_idx, _ctx, _rect| {})
            .page_size(8)
            .snapshot_fields(),
        SnapshotFields::Table {
            columns: vec![SnapshotTableColumn {
                title: "Name".to_string(),
                width: 120.0,
                sortable: true,
                filterable: false,
                filters: vec!["Active".to_string(), "Paused".to_string()],
            }],
            rows,
            row_h: 36.0,
            header_h: 32.0,
            expand_height: 72.0,
            empty_text: "No rows".to_string(),
            page_size: 8,
        }
    );

    let mut selectable = SelectableList::new();
    selectable.items = vec![SelectableItem::new("home", "Home").icon("house")];
    selectable.active_index = 2;
    selectable.header_button_text = "Create".to_string();
    selectable.footer_text = "1 item".to_string();
    selectable.item_height = 42.0;
    assert_eq!(
        selectable.snapshot_fields(),
        SnapshotFields::SelectableList {
            items: vec![SelectableItem {
                id: "home".to_string(),
                text: "Home".to_string(),
                icon: Some("house".to_string()),
            }],
            header_button_text: "Create".to_string(),
            footer_text: "1 item".to_string(),
            item_height: 42.0,
        }
    );

    assert_eq!(
        ScrollView::new(ScrollDirection::Both)
            .size(320.0, 180.0)
            .flex_grow(1.0)
            .flex_shrink(0.0)
            .show_scrollbar(false)
            .scroll_to(40.0, 80.0)
            .snapshot_fields(),
        SnapshotFields::ScrollView {
            direction: ScrollDirection::Both,
            fixed_width: Some(320.0),
            fixed_height: Some(180.0),
            flex_grow: 1.0,
            flex_shrink: 0.0,
            show_scrollbar: false,
        }
    );

    assert_eq!(
        QRCode::new("uix")
            .size(96.0)
            .error_level(2)
            .snapshot_fields(),
        SnapshotFields::QRCode {
            value: "uix".to_string(),
            size: 96.0,
            error_level: 2,
        }
    );
}

#[test]
fn chart_snapshots_capture_static_config() {
    let bars = vec![BarData::new("A", 12.0, Color::red())];
    assert_eq!(
        BarChart::new()
            .data(bars.clone())
            .width(320.0)
            .height(180.0)
            .max_value(20.0)
            .show_value(false)
            .bar_radius(4.0)
            .snapshot_fields(),
        SnapshotFields::BarChart {
            data: bars,
            fixed_width: 320.0,
            fixed_height: 180.0,
            max_value: 20.0,
            show_value: false,
            bar_radius: 4.0,
        }
    );

    let points = vec![LineData::new("Mon", 1.0), LineData::new("Tue", 3.0)];
    assert_eq!(
        LineChart::new()
            .data(points.clone())
            .width(360.0)
            .height(160.0)
            .line_color(Color::blue())
            .max_value(4.0)
            .auto_min(true)
            .show_grid(false)
            .show_dots(false)
            .line_width(3.0)
            .snapshot_fields(),
        SnapshotFields::LineChart {
            data: points,
            fixed_width: 360.0,
            fixed_height: 160.0,
            line_color: Some(Color::blue()),
            max_value: 4.0,
            auto_min: true,
            show_grid: false,
            show_dots: false,
            line_width: 3.0,
            dot_radius: 3.0,
        }
    );

    let slices = vec![PieData::new("Used", 70.0, Color::green())];
    assert_eq!(
        PieChart::new()
            .data(slices.clone())
            .size(140.0)
            .donut(0.45)
            .snapshot_fields(),
        SnapshotFields::PieChart {
            data: slices,
            fixed_size: 140.0,
            hole_radius: 0.45,
        }
    );
}

#[test]
fn data_display_and_scroll_snapshots_exclude_runtime_state() {
    let mut form = Form::new();
    let before = form.snapshot_fields();
    form.set_field_value("missing", "ignored");
    form.set_field_status("missing", ValidateStatus::Error, "ignored");
    assert_eq!(form.snapshot_fields(), before);

    let mut sorted_col = TableColumn::new("Name", 120.0).sortable(true);
    sorted_col.sort_direction = SortDirection::Asc;
    sorted_col.filters = vec![("Active".to_string(), true)];
    let mut plain_col = TableColumn::new("Name", 120.0).sortable(true);
    plain_col.filters = vec![("Active".to_string(), false)];
    assert_eq!(
        Table::new().columns(vec![sorted_col]).snapshot_fields(),
        Table::new().columns(vec![plain_col]).snapshot_fields()
    );

    let table = Table::new().rows(vec![vec!["Ada".to_string()]]);
    let before = table.snapshot_fields();
    table.set_selected_row(Some(0));
    assert_eq!(table.snapshot_fields(), before);

    let mut scroll = ScrollView::new(ScrollDirection::Vertical).size(200.0, 100.0);
    let before = scroll.snapshot_fields();
    scroll.set_scroll_y(80.0);
    assert_eq!(scroll.snapshot_fields(), before);
}

#[test]
fn display_snapshots_exclude_runtime_selection_and_animation_state() {
    let mut tree = Tree::new(vec![TreeNode::new("Root", "root")]);
    let before = tree.snapshot_fields();
    tree.set_selected_key("root");
    assert_eq!(tree.snapshot_fields(), before);

    let steps = Steps::new(vec![Step::new("Start"), Step::new("Ship")]).current(0);
    let before = steps.snapshot_fields();
    steps.set_current(1);
    assert_eq!(steps.snapshot_fields(), before);

    let mut collapse = Collapse::new().panels(vec![CollapsePanel::new("A", "Alpha")]);
    let before = collapse.snapshot_fields();
    assert_eq!(
        collapse.on_event(&SystemEvent::PointerDown {
            pos: Point::new(4.0, 4.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        crate::ui::EventResult::Handled
    );
    assert_eq!(collapse.snapshot_fields(), before);
}

#[test]
fn component_auto_snapshot_captures_public_fields_only() {
    let probe = SnapshotProbe {
        title: "Ready".to_string(),
        count: 3,
        hover_count: 7,
        _secret: "runtime".to_string(),
    };

    let snapshot = ComponentConfigSnapshot::from_component(ComponentId::new(42), &probe);

    assert_eq!(snapshot.id, ComponentId::new(42));
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
fn component_measure_method_implements_layout() {
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
fn component_capture_method_implements_event_handler() {
    let mut probe = CaptureMacroProbe {};

    assert!(crate::ui::WidgetComponent::as_event(&probe).is_some());
    assert!(EventHandler::wants_capture_phase(&probe));
    assert_eq!(
        EventHandler::on_event(&mut probe, &SystemEvent::PointerLeave),
        crate::ui::EventResult::NotHandled
    );
}

#[test]
fn component_macro_name_struct_syntax_reuses_snapshot_metadata() {
    let probe = ComponentMacroProbe {
        label: "Thin".to_string(),
        runtime_counter: 11,
        private_note: "hidden".to_string(),
    };

    let snapshot = ComponentConfigSnapshot::from_component(ComponentId::new(77), &probe);

    assert_eq!(snapshot.id, ComponentId::new(77));
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

    let snapshot = ComponentConfigSnapshot::from_component(ComponentId::new(78), &probe);

    assert_eq!(snapshot.id, ComponentId::new(78));
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

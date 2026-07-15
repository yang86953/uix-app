use crate::draw::spatial::PhysicalUnit;
use crate::draw::Color;
use crate::native::traits::input::{ControlSize, ScrollDirection};
use crate::native::traits::system::StatusLevel;
use crate::ui::layout::{AlignItems, FlexDirection, JustifyContent};
use crate::ui::style::{Style, StyleSet};
use crate::ui::widgets::*;
use crate::ui::window_chrome::WindowControl;
use crate::ui::Placement;

use super::{
    AccessibilityRole, AccessibilitySnapshot, AccessibilityState, SelectionSnapshot,
    SnapshotCollapsePanel, SnapshotField, SnapshotTableColumn, SnapshotTableColumnGroup,
    SnapshotTransferItem, SnapshotTreeNode,
};

#[derive(Debug, Clone, PartialEq)]
pub enum SnapshotFields {
    Unknown,
    Custom {
        widget: &'static str,
        fields: Vec<SnapshotField>,
    },
    Button {
        text: String,
        disabled: bool,
        block: bool,
        style_set: StyleSet,
        style: Style,
    },
    WindowControl {
        control: WindowControl,
        accessible_name: String,
    },
    Label {
        text: String,
        font_size: f32,
        font_size_unit: Option<PhysicalUnit>,
        color: Option<Color>,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        style: Option<Style>,
    },
    Input {
        placeholder: String,
        input_size: ControlSize,
        disabled: bool,
        prefix: String,
        suffix: String,
        addon_before: String,
        addon_after: String,
        password: bool,
        password_visible: bool,
        clearable: bool,
        search: bool,
        textarea: bool,
        textarea_rows: usize,
        max_length: Option<usize>,
    },
    Space {
        direction: FlexDirection,
        space_size: SpaceSize,
        wrap: bool,
        justify: JustifyContent,
        align: AlignItems,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        flex_grow: f32,
    },
    Divider {
        text: Option<String>,
        orientation: DividerOrientation,
        direction: DividerDirection,
        color: Option<Color>,
        text_size: f32,
        dashed: bool,
    },
    Icon {
        name: String,
        size: f32,
    },
    Typography {
        content: String,
        type_: TypographyType,
        disabled: bool,
        mark: bool,
        code: bool,
        underline: bool,
        delete: bool,
        strong: bool,
        italic: bool,
        copyable: bool,
        color_override: Option<Color>,
    },
    Checkbox {
        checked: bool,
        disabled: bool,
        label: String,
    },
    Radio {
        group_name: String,
        options: Vec<String>,
        selected: usize,
        disabled: bool,
        direction: RadioDirection,
        item_h: f32,
    },
    Switch {
        checked: bool,
        disabled: bool,
        size: f32,
    },
    Slider {
        min: f64,
        max: f64,
        step: f64,
        value: f64,
    },
    Rate {
        count: usize,
        value: usize,
        half: bool,
        disabled: bool,
        clearable: bool,
        character: String,
    },
    InputNumber {
        value: f64,
        min: f64,
        max: f64,
        step: f64,
        placeholder: String,
        disabled: bool,
    },
    Avatar {
        text: String,
        size: f32,
        bg_color: Option<Color>,
        text_color: Option<Color>,
        square: bool,
        src: String,
    },
    Badge {
        count: i32,
        max: i32,
        dot: bool,
        color: Option<Color>,
        size: f32,
        status: Option<BadgeStatus>,
        show_zero: bool,
        text: String,
        offset_x: f32,
        offset_y: f32,
        offset_unit: Option<(PhysicalUnit, PhysicalUnit)>,
    },
    Card {
        title: Option<String>,
        bordered: bool,
        hoverable: bool,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        padding: f32,
        elevation: u8,
        flex_grow: f32,
        actions: Vec<String>,
    },
    Empty {
        description: String,
        icon_name: String,
        image: String,
    },
    Image {
        src: String,
        alt: String,
        fallback: String,
        width: f32,
        height: f32,
        radius: f32,
        preview: bool,
        fit: bool,
    },
    Tag {
        text: String,
        color: TagColor,
        closable: bool,
        font_size: f32,
        custom_color: Option<Color>,
        checkable: bool,
    },
    Timeline {
        items: Vec<TimelineItem>,
        pending: bool,
        reverse: bool,
    },
    Calendar {
        cell_size: f32,
        year_jump: bool,
    },
    Skeleton {
        shape: SkeletonShape,
        width: f32,
        height: f32,
    },
    FloatButton {
        icon: String,
        tooltip: String,
        badge_count: i32,
        size: f32,
        x: f32,
        y: f32,
    },
    Alert {
        message: String,
        description: String,
        type_: StatusLevel,
        closable: bool,
        show_icon: bool,
    },
    Message {
        placement: Placement,
    },
    Notification {
        placement: Placement,
    },
    ProgressBar {
        progress: f32,
        mode: ProgressMode,
        stroke_color: Option<Color>,
        track_color: Option<Color>,
        height: f32,
        width: f32,
        round: bool,
        progress_type: ProgressType,
    },
    Spin {
        size: SpinSize,
        color: Option<Color>,
        spinning: bool,
        tip: String,
        wrapper_mode: bool,
    },
    Tooltip {
        text: String,
        placement: TooltipPlacement,
        trigger: TriggerMode,
        bg_color: Option<Color>,
        text_color: Option<Color>,
        delay_ms: u32,
        timer_id: u32,
        arrow: bool,
    },
    Popover {
        title: String,
        content: String,
        placement: PopoverPlacement,
        trigger: PopoverTrigger,
        arrow: bool,
    },
    Popconfirm {
        title: String,
        confirm_text: String,
        cancel_text: String,
        placement: PopconfirmPlacement,
        arrow: bool,
        icon: bool,
    },
    Modal {
        title: String,
        width: f32,
        height: f32,
        modal_size: ControlSize,
        closable: bool,
        mask_closable: bool,
        footer_visible: bool,
        centered: bool,
        overlay: bool,
    },
    Drawer {
        title: String,
        width: f32,
        height: f32,
        drawer_size: ControlSize,
        placement: DrawerPlacement,
        closable: bool,
        mask_closable: bool,
        mask: bool,
        footer_visible: bool,
        extra: String,
    },
    Layout {
        bg_color: Option<Color>,
    },
    Header {
        height: f32,
        bg_color: Option<Color>,
    },
    Sider {
        width: f32,
        bg_color: Option<Color>,
        collapsible: bool,
        collapsed: bool,
        collapsed_width: f32,
    },
    Content {
        bg_color: Option<Color>,
    },
    Footer {
        height: f32,
        bg_color: Option<Color>,
    },
    Splitter {
        vertical: bool,
        panel_count: usize,
        min_sizes: Vec<f32>,
        handle_size: f32,
    },
    Affix {
        offset_top: f32,
    },
    BackTop {
        visibility_height: f32,
    },
    Breadcrumb {
        items: Vec<BreadcrumbItem>,
        separator: String,
    },
    Pagination {
        total: usize,
        page_size: usize,
        current: usize,
        show_size_changer: bool,
        show_total: bool,
        size: f32,
        page_size_options: Vec<usize>,
    },
    Anchor {
        items: Vec<AnchorItem>,
        offset_top: f32,
        bg_color: Option<Color>,
    },
    Menu {
        items: Vec<MenuItem>,
        active_key: String,
        mode: MenuMode,
        item_h: f32,
    },
    Dropdown {
        label: String,
        items: Vec<String>,
        open: bool,
        selected_index: Option<usize>,
        highlighted_index: Option<usize>,
    },
    Tabs {
        tabs: Vec<Tab>,
        active_index: usize,
        position: TabPosition,
        tab_height: f32,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
    },
    Steps {
        steps: Vec<Step>,
        direction: bool,
    },
    NavItem {
        label: String,
        icon: String,
        fixed_width: f32,
        fixed_height: f32,
        index: usize,
        compact: bool,
        active: bool,
    },
    Tree {
        nodes: Vec<SnapshotTreeNode>,
        selected_key: String,
        selected_keys: Vec<String>,
        expanded_keys: Vec<String>,
        multiple: bool,
    },
    List {
        header: String,
        footer: String,
        bordered: bool,
        list_size: ControlSize,
        items: Vec<String>,
        load_more_text: String,
    },
    Collapse {
        panels: Vec<SnapshotCollapsePanel>,
        accordion: bool,
    },
    Carousel {
        show_dots: bool,
        show_arrows: bool,
    },
    Select {
        options: Vec<String>,
        optgroups: Vec<OptGroup>,
        selected: usize,
        selected_multi: Vec<usize>,
        open: bool,
        disabled: bool,
        placeholder: String,
        multiple: bool,
        search: bool,
        search_query: String,
    },
    AutoComplete {
        placeholder: String,
        options: Vec<String>,
        value: String,
        open: bool,
    },
    TreeSelect {
        placeholder: String,
        nodes: Vec<SnapshotTreeNode>,
        value: String,
        value_key: String,
        open: bool,
    },
    Cascader {
        options: Vec<CascaderOption>,
        placeholder: String,
        selected_labels: Vec<String>,
        selected_values: Vec<String>,
        open: bool,
    },
    ColorPicker {
        value: Color,
        preset_colors: Vec<Color>,
    },
    DatePicker {
        placeholder: String,
        value: Option<String>,
    },
    DateRangePicker {
        placeholder: String,
        start: Option<String>,
        end: Option<String>,
    },
    TimePicker {
        placeholder: String,
        value: Option<String>,
    },
    Mentions {
        placeholder: String,
        options: Vec<String>,
        value: String,
        suggesting: bool,
    },
    Segmented {
        options: Vec<String>,
        selected: usize,
        disabled: bool,
        disabled_options: Vec<bool>,
    },
    FormItem {
        label: String,
        name: String,
        required: bool,
        help: String,
        label_width: f32,
        layout: FormLayout,
    },
    Form {
        label_width: f32,
        gap: f32,
        layout: FormLayout,
    },
    Descriptions {
        title: String,
        items: Vec<DescriptionsItem>,
        bordered: bool,
        column: usize,
        label_width: f32,
        descriptions_size: ControlSize,
    },
    Result {
        result_type: ResultType,
        title: String,
        subtitle: String,
        extra_text: String,
    },
    Table {
        columns: Vec<SnapshotTableColumn>,
        column_groups: Vec<SnapshotTableColumnGroup>,
        rows: Vec<TableRow>,
        row_h: f32,
        header_h: f32,
        expandable: bool,
        expand_height: f32,
        sortable: bool,
        selection: bool,
        bordered: bool,
        selected_row: Option<usize>,
        checked_rows: Vec<usize>,
        empty_text: String,
        page_size: usize,
        virtual_scroll: bool,
    },
    SelectableList {
        items: Vec<SelectableItem>,
        header_button_text: String,
        footer_text: String,
        item_height: f32,
    },
    ScrollView {
        direction: ScrollDirection,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        flex_grow: f32,
        flex_shrink: f32,
        show_scrollbar: bool,
    },
    BarChart {
        data: Vec<BarData>,
        fixed_width: f32,
        fixed_height: f32,
        max_value: f32,
        show_value: bool,
        bar_radius: f32,
    },
    LineChart {
        data: Vec<LineData>,
        fixed_width: f32,
        fixed_height: f32,
        line_color: Option<Color>,
        max_value: f32,
        auto_min: bool,
        show_grid: bool,
        show_dots: bool,
        line_width: f32,
        dot_radius: f32,
    },
    PieChart {
        data: Vec<PieData>,
        fixed_size: f32,
        hole_radius: f32,
    },
    QRCode {
        value: String,
        size: f32,
        error_level: u8,
    },
    RichText {
        segments: Vec<RichTextSegment>,
        default_font_size: f32,
        default_font_size_unit: Option<PhysicalUnit>,
        default_color: Color,
    },
    ThemeToggle {
        dark: bool,
    },
    Transfer {
        source: Vec<SnapshotTransferItem>,
        target: Vec<SnapshotTransferItem>,
    },
    Upload {
        accept: String,
        multiple: bool,
        drag: bool,
        max_count: usize,
    },
    Watermark {
        text: String,
        color: Color,
        font_size: f32,
        opacity: f32,
        rotate: f32,
        gap_x: f32,
        gap_y: f32,
        x_offset: f32,
        y_offset: f32,
    },
    Container {
        style: Style,
    },
    Grid {
        style: Style,
        breakpoints: Option<Breakpoints>,
        cols: Vec<Col>,
    },
}

impl SnapshotFields {
    pub(super) fn selection(&self) -> Option<SelectionSnapshot> {
        match self {
            Self::Radio {
                options, selected, ..
            } => Some(SelectionSnapshot {
                options: options.clone(),
                selected_indices: (*selected < options.len())
                    .then_some(*selected)
                    .into_iter()
                    .collect(),
                disabled_indices: Vec::new(),
                multiple: false,
                expanded: false,
            }),
            Self::Select {
                options,
                optgroups,
                selected,
                selected_multi,
                multiple,
                open,
                ..
            } => {
                let options = if optgroups.is_empty() {
                    options.clone()
                } else {
                    optgroups
                        .iter()
                        .flat_map(|group| group.options.iter().cloned())
                        .collect()
                };
                let selected_indices = if *multiple {
                    selected_multi
                        .iter()
                        .copied()
                        .filter(|index| *index < options.len())
                        .collect()
                } else {
                    (*selected < options.len())
                        .then_some(*selected)
                        .into_iter()
                        .collect()
                };
                Some(SelectionSnapshot {
                    options,
                    selected_indices,
                    disabled_indices: Vec::new(),
                    multiple: *multiple,
                    expanded: *open,
                })
            }
            Self::Segmented {
                options,
                selected,
                disabled_options,
                ..
            } => Some(SelectionSnapshot {
                options: options.clone(),
                selected_indices: (*selected < options.len())
                    .then_some(*selected)
                    .into_iter()
                    .collect(),
                disabled_indices: disabled_options
                    .iter()
                    .enumerate()
                    .filter_map(|(index, disabled)| disabled.then_some(index))
                    .filter(|index| *index < options.len())
                    .collect(),
                multiple: false,
                expanded: false,
            }),
            _ => None,
        }
    }

    pub fn accessibility(&self) -> AccessibilitySnapshot {
        match self {
            Self::Button { text, disabled, .. } => {
                AccessibilitySnapshot::named(AccessibilityRole::Button, text.clone())
                    .with_state(AccessibilityState::disabled(*disabled))
            }
            Self::WindowControl {
                accessible_name, ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Button, accessible_name.clone()),
            Self::Label { text, .. } => {
                AccessibilitySnapshot::named(AccessibilityRole::Text, text.clone())
            }
            Self::Input {
                placeholder,
                disabled,
                password,
                search,
                textarea,
                ..
            } => {
                let role = if *search {
                    AccessibilityRole::Combobox
                } else {
                    AccessibilityRole::TextBox
                };
                AccessibilitySnapshot::named(role, placeholder.clone()).with_state(
                    AccessibilityState {
                        disabled: *disabled,
                        multiline: *textarea,
                        password: *password,
                        ..AccessibilityState::default()
                    },
                )
            }
            Self::Typography {
                content, disabled, ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Text, content.clone())
                .with_state(AccessibilityState::disabled(*disabled)),
            Self::Checkbox {
                checked,
                disabled,
                label,
            } => AccessibilitySnapshot::named(AccessibilityRole::Checkbox, label.clone())
                .with_state(AccessibilityState {
                    disabled: *disabled,
                    checked: Some(*checked),
                    ..AccessibilityState::default()
                }),
            Self::Radio {
                group_name,
                options,
                selected,
                disabled,
                ..
            } => AccessibilitySnapshot::named(AccessibilityRole::RadioGroup, group_name.clone())
                .with_state(AccessibilityState {
                    disabled: *disabled,
                    value_text: options.get(*selected).cloned(),
                    ..AccessibilityState::default()
                }),
            Self::Switch {
                checked, disabled, ..
            } => AccessibilitySnapshot::new(AccessibilityRole::Switch).with_state(
                AccessibilityState {
                    disabled: *disabled,
                    checked: Some(*checked),
                    ..AccessibilityState::default()
                },
            ),
            Self::Slider {
                min, max, value, ..
            } => AccessibilitySnapshot::new(AccessibilityRole::Slider).with_state(
                AccessibilityState {
                    value_now: Some(*value),
                    value_min: Some(*min),
                    value_max: Some(*max),
                    ..AccessibilityState::default()
                },
            ),
            Self::Rate {
                count,
                value,
                disabled,
                ..
            } => AccessibilitySnapshot::new(AccessibilityRole::Slider).with_state(
                AccessibilityState {
                    disabled: *disabled,
                    value_now: Some(*value as f64),
                    value_min: Some(0.0),
                    value_max: Some(*count as f64),
                    ..AccessibilityState::default()
                },
            ),
            Self::InputNumber {
                value,
                min,
                max,
                placeholder,
                disabled,
                ..
            } => AccessibilitySnapshot::named(AccessibilityRole::SpinButton, placeholder.clone())
                .with_state(AccessibilityState {
                    disabled: *disabled,
                    value_now: Some(*value),
                    value_min: Some(*min),
                    value_max: Some(*max),
                    ..AccessibilityState::default()
                }),
            Self::Image {
                alt, fallback, src, ..
            } => AccessibilitySnapshot::named(
                AccessibilityRole::Image,
                first_non_empty([alt.as_str(), fallback.as_str(), src.as_str()]),
            ),
            Self::ProgressBar { progress, .. } => AccessibilitySnapshot::new(
                AccessibilityRole::ProgressBar,
            )
            .with_state(AccessibilityState {
                value_now: Some((*progress).clamp(0.0, 1.0) as f64),
                value_min: Some(0.0),
                value_max: Some(1.0),
                ..AccessibilityState::default()
            }),
            Self::Alert { message, .. } => {
                AccessibilitySnapshot::named(AccessibilityRole::Alert, message.clone())
            }
            Self::Modal { title, .. } | Self::Drawer { title, .. } => {
                AccessibilitySnapshot::named(AccessibilityRole::Dialog, title.clone())
            }
            Self::Pagination {
                current,
                total,
                page_size,
                ..
            } => AccessibilitySnapshot::new(AccessibilityRole::Navigation).with_state(
                AccessibilityState {
                    value_text: Some(format!(
                        "Page {current} of {}; {page_size} per page",
                        total.div_ceil(*page_size)
                    )),
                    value_now: Some(*current as f64),
                    value_min: Some(1.0),
                    value_max: Some(total.div_ceil(*page_size).max(1) as f64),
                    ..AccessibilityState::default()
                },
            ),
            Self::Anchor { .. } | Self::Breadcrumb { .. } => {
                AccessibilitySnapshot::new(AccessibilityRole::Navigation)
            }
            Self::Menu {
                items, active_key, ..
            } => {
                AccessibilitySnapshot::new(AccessibilityRole::Menu).with_state(AccessibilityState {
                    value_text: items
                        .iter()
                        .find(|item| item.key == *active_key)
                        .map(|item| item.label.clone()),
                    ..AccessibilityState::default()
                })
            }
            Self::Dropdown {
                label,
                items,
                open,
                selected_index,
                ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Menu, label.clone()).with_state(
                AccessibilityState {
                    expanded: Some(*open),
                    value_text: selected_index.and_then(|index| items.get(index)).cloned(),
                    ..AccessibilityState::default()
                },
            ),
            Self::Tabs {
                tabs, active_index, ..
            } => AccessibilitySnapshot::new(AccessibilityRole::TabList).with_state(
                AccessibilityState {
                    value_text: tabs.get(*active_index).map(|tab| tab.label.clone()),
                    ..AccessibilityState::default()
                },
            ),
            Self::Steps { .. } => AccessibilitySnapshot::new(AccessibilityRole::TabList),
            Self::NavItem { label, active, .. } => {
                AccessibilitySnapshot::named(AccessibilityRole::Navigation, label.clone())
                    .with_state(AccessibilityState {
                        selected: Some(*active),
                        ..AccessibilityState::default()
                    })
            }
            Self::Tree { selected_key, .. } => AccessibilitySnapshot::new(AccessibilityRole::Tree)
                .with_state(AccessibilityState {
                    value_text: (!selected_key.is_empty()).then(|| selected_key.clone()),
                    ..AccessibilityState::default()
                }),
            Self::TreeSelect {
                placeholder,
                value,
                open,
                ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder.clone())
                .with_state(AccessibilityState {
                    expanded: Some(*open),
                    value_text: (!value.is_empty()).then(|| value.clone()),
                    ..AccessibilityState::default()
                }),
            Self::List { .. } | Self::SelectableList { .. } | Self::Transfer { .. } => {
                AccessibilitySnapshot::new(AccessibilityRole::List)
            }
            Self::Table { .. } => AccessibilitySnapshot::new(AccessibilityRole::Table),
            Self::Select {
                options,
                optgroups,
                selected,
                selected_multi,
                placeholder,
                disabled,
                multiple,
                open,
                search_query,
                ..
            } => {
                let options = if optgroups.is_empty() {
                    options.iter().map(String::as_str).collect::<Vec<_>>()
                } else {
                    optgroups
                        .iter()
                        .flat_map(|group| group.options.iter().map(String::as_str))
                        .collect()
                };
                let selected_value = if *multiple {
                    let selected = selected_multi
                        .iter()
                        .filter_map(|index| options.get(*index).copied())
                        .collect::<Vec<_>>();
                    (!selected.is_empty()).then(|| selected.join(", "))
                } else {
                    options.get(*selected).map(|option| (*option).to_owned())
                };
                let value_text = if !*open || search_query.is_empty() {
                    selected_value
                } else {
                    Some(search_query.clone())
                };
                AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder.clone())
                    .with_state(AccessibilityState {
                        disabled: *disabled,
                        value_text,
                        ..AccessibilityState::default()
                    })
            }
            Self::DatePicker { placeholder, value } => {
                AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder.clone())
                    .with_state(AccessibilityState {
                        value_text: value.clone(),
                        ..AccessibilityState::default()
                    })
            }
            Self::DateRangePicker {
                placeholder,
                start,
                end,
            } => AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder.clone())
                .with_state(AccessibilityState {
                    value_text: start
                        .as_ref()
                        .zip(end.as_ref())
                        .map(|(start, end)| format!("{start} / {end}")),
                    ..AccessibilityState::default()
                }),
            Self::TimePicker { placeholder, value } => {
                AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder.clone())
                    .with_state(AccessibilityState {
                        value_text: value.clone(),
                        ..AccessibilityState::default()
                    })
            }
            Self::ColorPicker { value, .. } => AccessibilitySnapshot::new(
                AccessibilityRole::Combobox,
            )
            .with_state(AccessibilityState {
                value_text: Some(value.to_string()),
                ..AccessibilityState::default()
            }),
            Self::AutoComplete {
                placeholder,
                value,
                open,
                ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder.clone())
                .with_state(AccessibilityState {
                    expanded: Some(*open),
                    value_text: (!value.is_empty()).then(|| value.clone()),
                    ..AccessibilityState::default()
                }),
            Self::Mentions {
                placeholder,
                value,
                suggesting,
                ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder.clone())
                .with_state(AccessibilityState {
                    expanded: Some(*suggesting),
                    value_text: (!value.is_empty()).then(|| value.clone()),
                    ..AccessibilityState::default()
                }),
            Self::Cascader {
                placeholder,
                selected_labels,
                open,
                ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder.clone())
                .with_state(AccessibilityState {
                    expanded: Some(*open),
                    value_text: (!selected_labels.is_empty()).then(|| selected_labels.join(" / ")),
                    ..AccessibilityState::default()
                }),
            Self::Segmented {
                disabled,
                options,
                selected,
                ..
            } => AccessibilitySnapshot::new(AccessibilityRole::RadioGroup).with_state(
                AccessibilityState {
                    disabled: *disabled,
                    value_text: options.get(*selected).cloned(),
                    ..AccessibilityState::default()
                },
            ),
            Self::FormItem {
                label, required, ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Group, label.clone()).with_state(
                AccessibilityState {
                    required: *required,
                    ..AccessibilityState::default()
                },
            ),
            Self::Result {
                title, subtitle, ..
            } => AccessibilitySnapshot::named(
                AccessibilityRole::Status,
                first_non_empty([title.as_str(), subtitle.as_str()]),
            ),
            Self::Spin { tip, .. } => {
                AccessibilitySnapshot::named(AccessibilityRole::Status, tip.clone())
            }
            _ => AccessibilitySnapshot::new(AccessibilityRole::Generic),
        }
    }
}

fn first_non_empty<const N: usize>(values: [&str; N]) -> String {
    values
        .into_iter()
        .find(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

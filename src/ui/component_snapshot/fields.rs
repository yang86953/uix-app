use std::sync::Arc;

use crate::draw::spatial::PhysicalUnit;
use crate::draw::Color;
use crate::native::traits::input::{ControlSize, ScrollDirection};
use crate::native::traits::system::StatusLevel;
use crate::ui::layout::{AlignItems, FlexDirection, JustifyContent};
use crate::ui::style::{Style, StyleSet};
use crate::ui::widgets::*;
use crate::ui::window_chrome::WindowControl;
use crate::ui::Placement;

use super::accessibility::{
    anchor_accessibility, avatar_accessibility, back_top_accessibility, badge_accessibility,
    breadcrumb_accessibility, calendar_accessibility, card_accessibility, carousel_accessibility,
    chart_accessibility, date_range_accessibility, dropdown_accessibility, first_non_empty,
    image_accessibility, input_number_accessibility, menu_accessibility, pagination_accessibility,
    popconfirm_accessibility, progress_accessibility, result_accessibility,
    rich_text_accessibility, select_accessibility, selectable_list_accessibility,
    splitter_accessibility, steps_accessibility, tabs_accessibility, tag_accessibility,
    theme_toggle_accessibility, timeline_accessibility, transfer_accessibility, tree_accessibility,
    typography_accessibility, upload_accessibility,
};
use super::{
    AccessibilityRole, AccessibilitySnapshot, AccessibilityState, SnapshotCollapsePanel,
    SnapshotField, SnapshotPopconfirm, SnapshotTableColumn, SnapshotTableColumnGroup,
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
        loading: bool,
        icon: String,
        group_position: Option<ButtonGroupPosition>,
        style_set: Arc<StyleSet>,
        style: Arc<Style>,
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
        status: Option<InputStatus>,
        status_message: String,
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
        marks: Vec<(f64, String)>,
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
        keyboard: bool,
        formatted: bool,
        display_value: Option<String>,
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
        adaptive_foreground: bool,
        ribbon: bool,
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
        focused_action: Option<usize>,
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
        preview_open: bool,
        fit: bool,
    },
    Tag {
        text: String,
        color: TagColor,
        closable: bool,
        font_size: f32,
        custom_color: Option<Color>,
        checkable: bool,
        checked: bool,
        visible: bool,
        icon: String,
    },
    Timeline {
        items: Vec<TimelineItem>,
        pending: bool,
        reverse: bool,
    },
    Calendar {
        cell_size: f32,
        year_jump: bool,
        year: i32,
        month: usize,
        selected: Option<Date>,
        focused_day: usize,
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
        reserve_layout_space: bool,
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
        contents: Vec<String>,
    },
    Notification {
        placement: Placement,
        titles: Vec<String>,
        descriptions: Vec<String>,
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
        visible: bool,
    },
    Popconfirm(SnapshotPopconfirm),
    Modal {
        title: String,
        open: bool,
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
        open: bool,
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
        ratios: Vec<f32>,
        active_handle: usize,
    },
    Affix {
        offset_top: f32,
        scroll_y: f32,
        affixed: bool,
    },
    BackTop {
        visibility_height: f32,
        visible: bool,
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
        active_index: usize,
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
        active_key: Option<String>,
        position: TabPosition,
        tab_height: f32,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
    },
    Steps {
        steps: Vec<Step>,
        current: usize,
        direction: StepsDirection,
    },
    NavItem {
        label: String,
        key: String,
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
        focused_header: usize,
    },
    Carousel {
        show_dots: bool,
        show_arrows: bool,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        current: usize,
        slide_count: usize,
    },
    Select {
        options: Vec<String>,
        optgroups: Vec<OptGroup>,
        selected: usize,
        selected_multi: Vec<usize>,
        open: bool,
        disabled: bool,
        loading: bool,
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
        loading_children: Vec<String>,
        searchable: bool,
        search_query: String,
        search_results: Vec<CascaderValue>,
    },
    ColorPicker {
        value: Color,
        preset_colors: Vec<Color>,
        open: bool,
    },
    DatePicker {
        placeholder: String,
        value: Option<String>,
        open: bool,
    },
    DateRangePicker {
        placeholder: String,
        start: Option<String>,
        end: Option<String>,
        open: bool,
    },
    TimePicker {
        placeholder: String,
        value: Option<String>,
        open: bool,
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
        row_keys: Vec<String>,
        view_columns: Vec<usize>,
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
        active_index: usize,
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
        scroll_x: f32,
        scroll_y: f32,
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
        module_count: usize,
        encoding_error: Option<String>,
    },
    RichText {
        segments: Vec<RichTextSegment>,
        default_font_size: f32,
        default_font_size_unit: Option<PhysicalUnit>,
        default_color: Color,
        focused_link: Option<usize>,
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
        files: Vec<UploadFile>,
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
    pub fn accessibility(&self) -> AccessibilitySnapshot {
        match self {
            Self::Button {
                text,
                icon,
                disabled,
                loading,
                ..
            } => AccessibilitySnapshot::named(
                AccessibilityRole::Button,
                first_non_empty([text.as_str(), icon.as_str()]),
            )
            .with_state(AccessibilityState::disabled(*disabled || *loading)),
            Self::WindowControl {
                accessible_name, ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Button, accessible_name.clone()),
            Self::Label { text, .. } => {
                AccessibilitySnapshot::named(AccessibilityRole::Text, text.clone())
            }
            Self::Avatar { text, src, .. } => avatar_accessibility(text, src),
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
                content,
                disabled,
                copyable,
                ..
            } => typography_accessibility(content, *disabled, *copyable),
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
            Self::Splitter {
                ratios,
                active_handle,
                ..
            } => splitter_accessibility(ratios, *active_handle),
            Self::Rate {
                count,
                value,
                half,
                disabled,
                ..
            } => AccessibilitySnapshot::new(AccessibilityRole::Slider).with_state(
                AccessibilityState {
                    disabled: *disabled,
                    value_now: Some(*value as f64),
                    value_min: Some(0.0),
                    value_max: Some(if *half {
                        count.saturating_mul(2) as f64
                    } else {
                        *count as f64
                    }),
                    ..AccessibilityState::default()
                },
            ),
            Self::InputNumber {
                value,
                min,
                max,
                placeholder,
                disabled,
                formatted,
                display_value,
                ..
            } => {
                let accessible_display = if *formatted {
                    display_value.as_deref()
                } else {
                    None
                };
                input_number_accessibility(
                    *value,
                    *min,
                    *max,
                    placeholder,
                    *disabled,
                    accessible_display,
                )
            }
            Self::Empty { description, .. } => AccessibilitySnapshot::named(
                AccessibilityRole::Status,
                if description.is_empty() {
                    crate::ui::locale::use_locale()
                        .empty_description
                        .to_string()
                } else {
                    description.clone()
                },
            ),
            Self::Image {
                alt,
                fallback,
                src,
                preview,
                preview_open,
                ..
            } => image_accessibility(alt, fallback, src, *preview, *preview_open),
            Self::Tag {
                text,
                closable,
                checkable,
                checked,
                ..
            } => tag_accessibility(text, *closable, *checkable, *checked),
            Self::Timeline {
                items,
                pending,
                reverse,
            } => timeline_accessibility(items, *pending, *reverse),
            Self::FloatButton { icon, tooltip, .. } => AccessibilitySnapshot::named(
                AccessibilityRole::Button,
                first_non_empty([tooltip.as_str(), icon.as_str()]),
            ),
            Self::Badge {
                count,
                max,
                dot,
                status,
                show_zero,
                text,
                ..
            } => badge_accessibility(*count, *max, *dot, *status, *show_zero, text),
            Self::ProgressBar { mode, .. } => progress_accessibility(*mode),
            Self::Alert {
                message,
                description,
                ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Alert, message.clone())
                .with_state(AccessibilityState {
                    value_text: (!description.is_empty()).then(|| description.clone()),
                    ..AccessibilityState::default()
                }),
            Self::Message { contents, .. } if !contents.is_empty() => {
                let name = contents
                    .iter()
                    .filter(|content| !content.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("；");
                AccessibilitySnapshot::named(
                    AccessibilityRole::Alert,
                    if name.is_empty() {
                        "Message".to_owned()
                    } else {
                        name
                    },
                )
            }
            Self::Notification {
                titles,
                descriptions,
                ..
            } if !titles.is_empty() || !descriptions.is_empty() => {
                let name = titles
                    .iter()
                    .filter(|title| !title.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("；");
                let value_text = descriptions
                    .iter()
                    .filter(|description| !description.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("；");
                AccessibilitySnapshot::named(
                    AccessibilityRole::Alert,
                    if name.is_empty() {
                        "Notification".to_owned()
                    } else {
                        name
                    },
                )
                .with_state(AccessibilityState {
                    value_text: (!value_text.is_empty()).then_some(value_text),
                    ..AccessibilityState::default()
                })
            }
            Self::Popover {
                title,
                content,
                visible,
                ..
            } => AccessibilitySnapshot::named(
                AccessibilityRole::Button,
                first_non_empty([title, content, "Popover"]),
            )
            .with_state(AccessibilityState {
                expanded: Some(*visible),
                value_text: (!content.is_empty()).then_some(content.clone()),
                ..AccessibilityState::default()
            }),
            Self::Popconfirm(popconfirm) => popconfirm_accessibility(
                &popconfirm.title,
                &popconfirm.confirm_text,
                &popconfirm.cancel_text,
                popconfirm.visible,
                popconfirm.focused_action,
            ),
            Self::Modal { title, open, .. } => {
                let role = if *open {
                    AccessibilityRole::Dialog
                } else {
                    AccessibilityRole::Button
                };
                let name = if *open && title.is_empty() {
                    "Modal".to_owned()
                } else if *open {
                    title.clone()
                } else if title.is_empty() {
                    "打开 Modal".to_owned()
                } else {
                    format!("打开 {title}")
                };
                AccessibilitySnapshot::named(role, name).with_state(AccessibilityState {
                    expanded: Some(*open),
                    ..AccessibilityState::default()
                })
            }
            Self::Drawer { title, open, .. } => {
                let role = if *open {
                    AccessibilityRole::Dialog
                } else {
                    AccessibilityRole::Button
                };
                let name = if *open && title.is_empty() {
                    "Drawer".to_owned()
                } else if *open {
                    title.clone()
                } else if title.is_empty() {
                    "打开 Drawer".to_owned()
                } else {
                    format!("打开 {title}")
                };
                AccessibilitySnapshot::named(role, name).with_state(AccessibilityState {
                    expanded: Some(*open),
                    ..AccessibilityState::default()
                })
            }
            Self::Pagination {
                current,
                total,
                page_size,
                ..
            } => pagination_accessibility(*current, *total, *page_size),
            Self::Anchor {
                items,
                active_index,
                ..
            } => anchor_accessibility(items, *active_index),
            Self::Breadcrumb { items, .. } => breadcrumb_accessibility(items),
            Self::Menu {
                items, active_key, ..
            } => menu_accessibility(items, active_key),
            Self::Dropdown {
                label,
                items,
                open,
                selected_index,
                ..
            } => dropdown_accessibility(label, items, *open, *selected_index),
            Self::Tabs {
                tabs, active_index, ..
            } => tabs_accessibility(tabs, *active_index),
            Self::Steps { steps, current, .. } => steps_accessibility(steps, *current),
            Self::NavItem { label, active, .. } => {
                AccessibilitySnapshot::named(AccessibilityRole::Navigation, label.clone())
                    .with_state(AccessibilityState {
                        selected: Some(*active),
                        ..AccessibilityState::default()
                    })
            }
            Self::Tree {
                nodes,
                selected_key,
                expanded_keys,
                ..
            } => tree_accessibility(nodes, selected_key, expanded_keys),
            Self::Calendar {
                year,
                month,
                selected,
                focused_day,
                ..
            } => calendar_accessibility(*year, *month, *selected, *focused_day),
            Self::Descriptions { title, items, .. } => {
                AccessibilitySnapshot::named(AccessibilityRole::Group, title.clone()).with_state(
                    AccessibilityState {
                        value_text: (!items.is_empty()).then(|| {
                            items
                                .iter()
                                .map(|item| format!("{}: {}", item.label, item.value))
                                .collect::<Vec<_>>()
                                .join("; ")
                        }),
                        ..AccessibilityState::default()
                    },
                )
            }
            Self::Collapse {
                panels,
                focused_header,
                ..
            } => {
                let focused_panel = panels.get(*focused_header);
                AccessibilitySnapshot::named(
                    AccessibilityRole::Button,
                    focused_panel
                        .map(|panel| panel.header.clone())
                        .unwrap_or_default(),
                )
                .with_state(AccessibilityState {
                    value_text: panels
                        .get(*focused_header)
                        .map(|panel| panel.header.clone()),
                    expanded: focused_panel.map(|panel| panel.expanded),
                    ..AccessibilityState::default()
                })
            }
            Self::Carousel {
                current,
                slide_count,
                ..
            } => carousel_accessibility(*current, *slide_count),
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
            Self::SelectableList {
                items,
                active_index,
                ..
            } => selectable_list_accessibility(items, *active_index),
            Self::List { header, items, .. } => {
                AccessibilitySnapshot::named(AccessibilityRole::List, header.clone()).with_state(
                    AccessibilityState {
                        value_text: (!items.is_empty()).then(|| items.join("; ")),
                        ..AccessibilityState::default()
                    },
                )
            }
            Self::Card {
                title,
                actions,
                focused_action,
                ..
            } => card_accessibility(title.as_deref(), actions, *focused_action),
            Self::Transfer { source, target } => transfer_accessibility(source, target),
            Self::Upload { files, .. } => upload_accessibility(files),
            Self::RichText {
                segments,
                focused_link,
                ..
            } => rich_text_accessibility(segments, *focused_link),
            Self::Table { .. } => AccessibilitySnapshot::new(AccessibilityRole::Table),
            Self::BarChart { data, .. } => chart_accessibility(
                "Bar chart",
                data.iter().map(|item| (item.label.as_str(), item.value)),
            ),
            Self::LineChart { data, .. } => chart_accessibility(
                "Line chart",
                data.iter().map(|item| (item.label.as_str(), item.value)),
            ),
            Self::PieChart { data, .. } => chart_accessibility(
                "Pie chart",
                data.iter()
                    .filter(|item| item.value.is_finite() && item.value > 0.0)
                    .map(|item| (item.label.as_str(), item.value)),
            ),
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
            } => select_accessibility(
                options,
                optgroups,
                *selected,
                selected_multi,
                placeholder,
                *disabled,
                *multiple,
                *open,
                search_query,
            ),
            Self::DatePicker {
                placeholder,
                value,
                open,
            } => AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder.clone())
                .with_state(AccessibilityState {
                    expanded: Some(*open),
                    value_text: value.clone(),
                    ..AccessibilityState::default()
                }),
            Self::DateRangePicker {
                placeholder,
                start,
                end,
                open,
            } => date_range_accessibility(placeholder, start.as_ref(), end.as_ref(), *open),
            Self::TimePicker {
                placeholder,
                value,
                open,
            } => AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder.clone())
                .with_state(AccessibilityState {
                    value_text: value.clone(),
                    expanded: Some(*open),
                    ..AccessibilityState::default()
                }),
            Self::ColorPicker { value, open, .. } => AccessibilitySnapshot::new(
                AccessibilityRole::Combobox,
            )
            .with_state(AccessibilityState {
                value_text: Some(value.to_string()),
                expanded: Some(*open),
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
                result_type,
                title,
                subtitle,
                extra_text,
                ..
            } => result_accessibility(*result_type, title, subtitle, extra_text),
            Self::Spin { tip, spinning, .. } => AccessibilitySnapshot::named(
                AccessibilityRole::Status,
                if tip.is_empty() {
                    if *spinning { "加载中" } else { "加载" }.to_string()
                } else {
                    tip.clone()
                },
            ),
            Self::ThemeToggle { dark } => theme_toggle_accessibility(*dark),
            Self::BackTop { visible, .. } => back_top_accessibility(*visible),
            _ => AccessibilitySnapshot::new(AccessibilityRole::Generic),
        }
    }
}

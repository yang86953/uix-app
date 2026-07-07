use std::any::{Any, TypeId};
use std::fmt;

use crate::core::{ComponentId, EdgeInsets};
use crate::draw::spatial::PhysicalUnit;
use crate::draw::Color;
use crate::native::traits::input::{ControlSize, ScrollDirection};
use crate::native::traits::system::StatusLevel;
use crate::ui::layout::{AlignItems, FlexDirection, GridTrack, JustifyContent};
use crate::ui::style::{Style, StyleSet};
use crate::ui::widgets::{
    Affix, Alert, Anchor, AnchorItem, AutoComplete, Avatar, BackTop, Badge, BadgeStatus, BarChart,
    BarData, Breadcrumb, BreadcrumbItem, Button, Calendar, Card, Carousel, Cascader,
    CascaderOption, Checkbox, Collapse, ColorPicker, Container, Content, DatePicker, Descriptions,
    DescriptionsItem, Divider, DividerDirection, DividerOrientation, Drawer, DrawerPlacement,
    Dropdown, Empty, FloatButton, Footer, Form, FormItem, FormLayout, Grid, Header, Icon, Image,
    Input, InputNumber, Label, Layout, LineChart, LineData, List, Mentions, Menu, MenuItem,
    MenuMode, Message, MessagePlacement, Modal, NavItem, NotifPlacement, Notification, OptGroup,
    Pagination, PieChart, PieData, Popconfirm, PopconfirmPlacement, Popover, PopoverPlacement,
    PopoverTrigger, ProgressBar, ProgressMode, ProgressType, QRCode, Radio, RadioDirection, Rate,
    Result, ResultType, RichText, RichTextSegment, ScrollView, Segmented, Select, SelectableItem,
    SelectableList, Sider, Skeleton, SkeletonShape, Slider, Space, SpaceSize, Spin, SpinSize,
    Splitter, Step, Steps, Switch, Tab, TabPosition, Table, TableColumn, TableRow, Tabs, Tag,
    TagColor, ThemeToggle, TimePicker, Timeline, TimelineItem, Tooltip, TooltipPlacement, Transfer,
    TransferItem, Tree, TreeNode, TreeSelect, TriggerMode, Typography, TypographyType, Upload,
    Watermark,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentConfigSnapshot {
    pub id: ComponentId,
    pub widget_type: TypeId,
    pub fields: SnapshotFields,
}

impl ComponentConfigSnapshot {
    pub fn from_component(
        id: ComponentId,
        component: &dyn crate::ui::traits::WidgetComponent,
    ) -> Self {
        Self {
            id,
            widget_type: component.as_any().type_id(),
            fields: component.snapshot_fields(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotField {
    pub name: &'static str,
    pub value: SnapshotValue,
}

impl SnapshotField {
    pub fn debug<T: fmt::Debug>(name: &'static str, value: &T) -> Self {
        Self {
            name,
            value: SnapshotValue::Debug(format!("{value:?}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotTreeNode {
    pub title: String,
    pub key: String,
    pub icon: String,
    pub children: Vec<SnapshotTreeNode>,
    pub disabled: bool,
    pub checkable: bool,
    pub draggable: bool,
    pub is_leaf: bool,
}

impl SnapshotTreeNode {
    pub fn from_tree_node(node: &TreeNode) -> Self {
        Self {
            title: node.title.clone(),
            key: node.key.clone(),
            icon: node.icon.clone(),
            children: node.children.iter().map(Self::from_tree_node).collect(),
            disabled: node.disabled,
            checkable: node.checkable,
            draggable: node.draggable,
            is_leaf: node.is_leaf,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotCollapsePanel {
    pub header: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotTableColumn {
    pub title: String,
    pub width: f32,
    pub sortable: bool,
    pub filterable: bool,
    pub filters: Vec<String>,
}

impl SnapshotTableColumn {
    pub fn from_table_column(column: &TableColumn) -> Self {
        Self {
            title: column.title.clone(),
            width: column.width,
            sortable: column.sortable,
            filterable: column.filterable,
            filters: column
                .filters
                .iter()
                .map(|(label, _active)| label.clone())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotTransferItem {
    pub key: String,
    pub title: String,
}

impl SnapshotTransferItem {
    pub fn from_transfer_item(item: &TransferItem) -> Self {
        Self {
            key: item.key.clone(),
            title: item.title.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotValue {
    Debug(String),
}

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
        min: f32,
        max: f32,
        step: f32,
        value: f32,
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
        placement: MessagePlacement,
    },
    Notification {
        placement: NotifPlacement,
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
        mode: MenuMode,
        item_h: f32,
    },
    Dropdown {
        label: String,
        items: Vec<String>,
    },
    Tabs {
        tabs: Vec<Tab>,
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
    },
    Tree {
        nodes: Vec<SnapshotTreeNode>,
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
        disabled: bool,
        placeholder: String,
        multiple: bool,
        search: bool,
    },
    AutoComplete {
        placeholder: String,
        options: Vec<String>,
    },
    TreeSelect {
        placeholder: String,
        nodes: Vec<SnapshotTreeNode>,
    },
    Cascader {
        options: Vec<CascaderOption>,
        placeholder: String,
    },
    ColorPicker {
        preset_colors: Vec<Color>,
    },
    DatePicker {
        placeholder: String,
    },
    TimePicker {
        placeholder: String,
    },
    Mentions {
        placeholder: String,
        options: Vec<String>,
    },
    Segmented {
        options: Vec<String>,
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
        rows: Vec<TableRow>,
        row_h: f32,
        header_h: f32,
        expand_height: f32,
        empty_text: String,
        page_size: usize,
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
        columns: Vec<GridTrack>,
        rows: Vec<GridTrack>,
        col_gap: f32,
        row_gap: f32,
        padding: EdgeInsets,
        bg_color: Option<Color>,
        border_color: Option<Color>,
        border_width: f32,
        border_radius: f32,
        align_items: AlignItems,
        justify_items: JustifyContent,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
    },
}

pub trait SnapshotSource {
    fn snapshot_fields(&self) -> SnapshotFields;
}

pub fn snapshot_fields_from_any(component: &dyn Any) -> SnapshotFields {
    if let Some(button) = component.downcast_ref::<Button>() {
        return button.snapshot_fields();
    }
    if let Some(label) = component.downcast_ref::<Label>() {
        return label.snapshot_fields();
    }
    if let Some(input) = component.downcast_ref::<Input>() {
        return input.snapshot_fields();
    }
    if let Some(space) = component.downcast_ref::<Space>() {
        return space.snapshot_fields();
    }
    if let Some(divider) = component.downcast_ref::<Divider>() {
        return divider.snapshot_fields();
    }
    if let Some(icon) = component.downcast_ref::<Icon>() {
        return icon.snapshot_fields();
    }
    if let Some(typography) = component.downcast_ref::<Typography>() {
        return typography.snapshot_fields();
    }
    if let Some(checkbox) = component.downcast_ref::<Checkbox>() {
        return checkbox.snapshot_fields();
    }
    if let Some(radio) = component.downcast_ref::<Radio>() {
        return radio.snapshot_fields();
    }
    if let Some(switch) = component.downcast_ref::<Switch>() {
        return switch.snapshot_fields();
    }
    if let Some(slider) = component.downcast_ref::<Slider>() {
        return slider.snapshot_fields();
    }
    if let Some(rate) = component.downcast_ref::<Rate>() {
        return rate.snapshot_fields();
    }
    if let Some(input_number) = component.downcast_ref::<InputNumber>() {
        return input_number.snapshot_fields();
    }
    if let Some(avatar) = component.downcast_ref::<Avatar>() {
        return avatar.snapshot_fields();
    }
    if let Some(badge) = component.downcast_ref::<Badge>() {
        return badge.snapshot_fields();
    }
    if let Some(card) = component.downcast_ref::<Card>() {
        return card.snapshot_fields();
    }
    if let Some(empty) = component.downcast_ref::<Empty>() {
        return empty.snapshot_fields();
    }
    if let Some(image) = component.downcast_ref::<Image>() {
        return image.snapshot_fields();
    }
    if let Some(tag) = component.downcast_ref::<Tag>() {
        return tag.snapshot_fields();
    }
    if let Some(timeline) = component.downcast_ref::<Timeline>() {
        return timeline.snapshot_fields();
    }
    if let Some(calendar) = component.downcast_ref::<Calendar>() {
        return calendar.snapshot_fields();
    }
    if let Some(skeleton) = component.downcast_ref::<Skeleton>() {
        return skeleton.snapshot_fields();
    }
    if let Some(float_button) = component.downcast_ref::<FloatButton>() {
        return float_button.snapshot_fields();
    }
    if let Some(alert) = component.downcast_ref::<Alert>() {
        return alert.snapshot_fields();
    }
    if let Some(message) = component.downcast_ref::<Message>() {
        return message.snapshot_fields();
    }
    if let Some(notification) = component.downcast_ref::<Notification>() {
        return notification.snapshot_fields();
    }
    if let Some(progress) = component.downcast_ref::<ProgressBar>() {
        return progress.snapshot_fields();
    }
    if let Some(spin) = component.downcast_ref::<Spin>() {
        return spin.snapshot_fields();
    }
    if let Some(tooltip) = component.downcast_ref::<Tooltip>() {
        return tooltip.snapshot_fields();
    }
    if let Some(popover) = component.downcast_ref::<Popover>() {
        return popover.snapshot_fields();
    }
    if let Some(popconfirm) = component.downcast_ref::<Popconfirm>() {
        return popconfirm.snapshot_fields();
    }
    if let Some(modal) = component.downcast_ref::<Modal>() {
        return modal.snapshot_fields();
    }
    if let Some(drawer) = component.downcast_ref::<Drawer>() {
        return drawer.snapshot_fields();
    }
    if let Some(layout) = component.downcast_ref::<Layout>() {
        return layout.snapshot_fields();
    }
    if let Some(header) = component.downcast_ref::<Header>() {
        return header.snapshot_fields();
    }
    if let Some(sider) = component.downcast_ref::<Sider>() {
        return sider.snapshot_fields();
    }
    if let Some(content) = component.downcast_ref::<Content>() {
        return content.snapshot_fields();
    }
    if let Some(footer) = component.downcast_ref::<Footer>() {
        return footer.snapshot_fields();
    }
    if let Some(splitter) = component.downcast_ref::<Splitter>() {
        return splitter.snapshot_fields();
    }
    if let Some(affix) = component.downcast_ref::<Affix>() {
        return affix.snapshot_fields();
    }
    if let Some(back_top) = component.downcast_ref::<BackTop>() {
        return back_top.snapshot_fields();
    }
    if let Some(breadcrumb) = component.downcast_ref::<Breadcrumb>() {
        return breadcrumb.snapshot_fields();
    }
    if let Some(pagination) = component.downcast_ref::<Pagination>() {
        return pagination.snapshot_fields();
    }
    if let Some(anchor) = component.downcast_ref::<Anchor>() {
        return anchor.snapshot_fields();
    }
    if let Some(menu) = component.downcast_ref::<Menu>() {
        return menu.snapshot_fields();
    }
    if let Some(dropdown) = component.downcast_ref::<Dropdown>() {
        return dropdown.snapshot_fields();
    }
    if let Some(tabs) = component.downcast_ref::<Tabs>() {
        return tabs.snapshot_fields();
    }
    if let Some(steps) = component.downcast_ref::<Steps>() {
        return steps.snapshot_fields();
    }
    if let Some(nav_item) = component.downcast_ref::<NavItem>() {
        return nav_item.snapshot_fields();
    }
    if let Some(tree) = component.downcast_ref::<Tree>() {
        return tree.snapshot_fields();
    }
    if let Some(list) = component.downcast_ref::<List>() {
        return list.snapshot_fields();
    }
    if let Some(collapse) = component.downcast_ref::<Collapse>() {
        return collapse.snapshot_fields();
    }
    if let Some(carousel) = component.downcast_ref::<Carousel>() {
        return carousel.snapshot_fields();
    }
    if let Some(select) = component.downcast_ref::<Select>() {
        return select.snapshot_fields();
    }
    if let Some(autocomplete) = component.downcast_ref::<AutoComplete>() {
        return autocomplete.snapshot_fields();
    }
    if let Some(tree_select) = component.downcast_ref::<TreeSelect>() {
        return tree_select.snapshot_fields();
    }
    if let Some(cascader) = component.downcast_ref::<Cascader>() {
        return cascader.snapshot_fields();
    }
    if let Some(color_picker) = component.downcast_ref::<ColorPicker>() {
        return color_picker.snapshot_fields();
    }
    if let Some(date_picker) = component.downcast_ref::<DatePicker>() {
        return date_picker.snapshot_fields();
    }
    if let Some(time_picker) = component.downcast_ref::<TimePicker>() {
        return time_picker.snapshot_fields();
    }
    if let Some(mentions) = component.downcast_ref::<Mentions>() {
        return mentions.snapshot_fields();
    }
    if let Some(segmented) = component.downcast_ref::<Segmented>() {
        return segmented.snapshot_fields();
    }
    if let Some(form_item) = component.downcast_ref::<FormItem>() {
        return form_item.snapshot_fields();
    }
    if let Some(form) = component.downcast_ref::<Form>() {
        return form.snapshot_fields();
    }
    if let Some(descriptions) = component.downcast_ref::<Descriptions>() {
        return descriptions.snapshot_fields();
    }
    if let Some(result) = component.downcast_ref::<Result>() {
        return result.snapshot_fields();
    }
    if let Some(table) = component.downcast_ref::<Table>() {
        return table.snapshot_fields();
    }
    if let Some(selectable_list) = component.downcast_ref::<SelectableList>() {
        return selectable_list.snapshot_fields();
    }
    if let Some(scroll_view) = component.downcast_ref::<ScrollView>() {
        return scroll_view.snapshot_fields();
    }
    if let Some(bar_chart) = component.downcast_ref::<BarChart>() {
        return bar_chart.snapshot_fields();
    }
    if let Some(line_chart) = component.downcast_ref::<LineChart>() {
        return line_chart.snapshot_fields();
    }
    if let Some(pie_chart) = component.downcast_ref::<PieChart>() {
        return pie_chart.snapshot_fields();
    }
    if let Some(qrcode) = component.downcast_ref::<QRCode>() {
        return qrcode.snapshot_fields();
    }
    if let Some(rich_text) = component.downcast_ref::<RichText>() {
        return rich_text.snapshot_fields();
    }
    if let Some(theme_toggle) = component.downcast_ref::<ThemeToggle>() {
        return theme_toggle.snapshot_fields();
    }
    if let Some(transfer) = component.downcast_ref::<Transfer>() {
        return transfer.snapshot_fields();
    }
    if let Some(upload) = component.downcast_ref::<Upload>() {
        return upload.snapshot_fields();
    }
    if let Some(watermark) = component.downcast_ref::<Watermark>() {
        return watermark.snapshot_fields();
    }
    if let Some(container) = component.downcast_ref::<Container>() {
        return container.snapshot_fields();
    }
    if let Some(grid) = component.downcast_ref::<Grid>() {
        return grid.snapshot_fields();
    }
    SnapshotFields::Unknown
}

#[cfg(test)]
#[path = "../tests/ui/component_snapshot.rs"]
mod tests;

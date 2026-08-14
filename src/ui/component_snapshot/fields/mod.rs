use std::sync::Arc;

use crate::draw::Color;
use crate::draw::geometry::spatial::PhysicalUnit;
// 反馈 capability 启用时才需要警告提示状态级别。
#[cfg(feature = "feedback")]
use crate::native::capabilities::system::StatusLevel;
use crate::native::windowing::input::{ControlSize, ScrollDirection};
use crate::ui::form::FormLayout;
use crate::ui::layout::{AlignItems, FlexDirection, JustifyContent};
// 引入主题感知颜色值与通用样式快照类型。
use crate::ui::theme::style::{ColorValue, Style, StyleSet};
use crate::ui::widgets::window_chrome::WindowControl;
use crate::ui::widgets::*;
// FloatButton 与可选反馈组件共同使用公开浮层方位。
use crate::ui::Placement;

use super::{SnapshotCollapsePanel, SnapshotField, SnapshotTransferItem};
// 反馈 capability 启用时才引入专属气泡确认框快照模型。
#[cfg(feature = "feedback")]
// 该类型只服务同步门控的 Popconfirm 枚举变体。
use super::SnapshotPopconfirm;
// 树组件 capability 启用时才引入树节点快照模型。
#[cfg(feature = "tree-widgets")]
// 该类型只服务同步门控的 Tree 与 TreeSelect 枚举变体。
use super::SnapshotTreeNode;
// 表格 capability 启用时才引入专属快照列模型。
#[cfg(feature = "table")]
// 两个类型只服务同步门控的 Table 枚举变体。
use super::{SnapshotTableColumn, SnapshotTableColumnGroup};

mod accessibility;

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
        // 保存主题感知的语义文字颜色身份。
        semantic_color: Option<ColorValue>,
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
        tooltip: Option<crate::ui::widgets::TooltipPlacement>,
    },
    RangeSlider {
        min: f64,
        max: f64,
        step: f64,
        start: f64,
        end: f64,
        active_thumb: RangeSliderThumb,
        size: ControlSize,
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
        // 保存声明式小数位精度。
        precision: Option<u8>,
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
        /// 标记 Badge 是否作为透明组合装饰器。
        composite: bool,
        /// 标记运行时是否已经登记唯一真实子节点。
        child_present: bool,
        /// 保存最终装饰绘制边界，不复制真实子节点快照。
        decoration_bounds: crate::core::Rect,
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
    ImageGroup {
        images: Vec<String>,
        start_index: usize,
        current: usize,
        preview_open: bool,
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
        // 保存展开说明 authored config。
        description: String,
        tooltip: String,
        badge_count: i32,
        // 保存圆点徽标 authored config。
        badge_dot: bool,
        size: f32,
        x: f32,
        y: f32,
        // 保存可选窗口放置方向；None 保留 frame-relative 兼容语义。
        placement: Option<Placement>,
        reserve_layout_space: bool,
    },
    FloatButtonGroup {
        button_count: usize,
        trigger: TriggerMode,
        expanded: bool,
    },
    // 反馈 capability 关闭时同步收缩警告提示快照变体。
    #[cfg(feature = "feedback")]
    Alert {
        message: String,
        description: String,
        type_: StatusLevel,
        closable: bool,
        show_icon: bool,
    },
    // 反馈 capability 关闭时同步收缩全局消息快照变体。
    #[cfg(feature = "feedback")]
    Message {
        placement: Placement,
        contents: Vec<String>,
    },
    // 反馈 capability 关闭时同步收缩通知快照变体。
    #[cfg(feature = "feedback")]
    Notification {
        placement: Placement,
        titles: Vec<String>,
        descriptions: Vec<String>,
    },
    // 反馈 capability 关闭时同步收缩进度条快照变体。
    #[cfg(feature = "feedback")]
    ProgressBar {
        progress: f32,
        mode: ProgressMode,
        // 暴露动态输入是否经过安全归一化。
        input_normalized: bool,
        // 暴露动态输入的稳定归一化原因。
        normalization_reason: Option<ProgressNormalizationReason>,
        stroke_color: Option<Color>,
        track_color: Option<Color>,
        height: f32,
        width: f32,
        round: bool,
        progress_type: ProgressType,
    },
    // 反馈 capability 关闭时同步收缩加载指示器快照变体。
    #[cfg(feature = "feedback")]
    Spin {
        size: SpinSize,
        color: Option<Color>,
        spinning: bool,
        tip: String,
        wrapper_mode: bool,
    },
    // 反馈 capability 关闭时同步收缩文字提示快照变体。
    #[cfg(feature = "feedback")]
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
    // 反馈 capability 关闭时同步收缩气泡卡片快照变体。
    #[cfg(feature = "feedback")]
    Popover {
        title: String,
        content: String,
        placement: PopoverPlacement,
        trigger: PopoverTrigger,
        arrow: bool,
        visible: bool,
    },
    // 反馈 capability 关闭时同步收缩气泡确认框快照变体。
    #[cfg(feature = "feedback")]
    Popconfirm(SnapshotPopconfirm),
    // 反馈 capability 关闭时同步收缩对话框快照变体。
    #[cfg(feature = "feedback")]
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
    // 反馈 capability 关闭时同步收缩抽屉快照变体。
    #[cfg(feature = "feedback")]
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
        // 保存会改变直接子区域排列的主轴方向。
        direction: FlexDirection,
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
    // 导航 capability 关闭时同步收缩面包屑快照变体。
    #[cfg(feature = "navigation")]
    Breadcrumb {
        items: Vec<BreadcrumbItem>,
        separator: String,
    },
    // 导航 capability 关闭时同步收缩分页快照变体。
    #[cfg(feature = "navigation")]
    Pagination {
        total: usize,
        page_size: usize,
        current: usize,
        show_size_changer: bool,
        show_total: bool,
        size: f32,
        page_size_options: Vec<usize>,
    },
    // 导航 capability 关闭时同步收缩锚点快照变体。
    #[cfg(feature = "navigation")]
    Anchor {
        items: Vec<AnchorItem>,
        active_index: usize,
        offset_top: f32,
        bg_color: Option<Color>,
    },
    // 导航 capability 关闭时同步收缩菜单快照变体。
    #[cfg(feature = "navigation")]
    Menu {
        items: Vec<MenuItem>,
        active_key: String,
        mode: MenuMode,
        item_h: f32,
    },
    // 导航 capability 关闭时同步收缩下拉菜单快照变体。
    #[cfg(feature = "navigation")]
    Dropdown {
        label: String,
        // 保留完整 keyed 选项以观察稳定身份与展示文字。
        items: Vec<DropdownItem>,
        open: bool,
        selected_index: Option<usize>,
        highlighted_index: Option<usize>,
    },
    // 导航 capability 关闭时同步收缩标签页快照变体。
    #[cfg(feature = "navigation")]
    Tabs {
        tabs: Vec<Tab>,
        active_index: usize,
        active_key: Option<String>,
        position: TabPosition,
        tab_height: f32,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
    },
    // 导航 capability 关闭时同步收缩步骤条快照变体。
    #[cfg(feature = "navigation")]
    Steps {
        steps: Vec<Step>,
        current: usize,
        direction: StepsDirection,
    },
    // 导航 capability 关闭时同步收缩导航项快照变体。
    #[cfg(feature = "navigation")]
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
    // Navigation 外壳只快照调用方元数据与整栏折叠事实。
    #[cfg(feature = "navigation")]
    Navigation {
        title: String,
        version: Option<String>,
        collapsed: bool,
    },
    // 树组件 capability 关闭时不保留展示树快照变体。
    #[cfg(feature = "tree-widgets")]
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
        custom_options: bool,
    },
    AutoComplete {
        placeholder: String,
        options: Vec<String>,
        value: String,
        open: bool,
    },
    // 树组件 capability 关闭时不保留树选择器快照变体。
    #[cfg(feature = "tree-widgets")]
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
    // 表格 capability 关闭时同步收缩公开快照枚举。
    #[cfg(feature = "table")]
    // 启用后记录表格列、数据、布局与交互状态。
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
        loading: bool,
        selected_row: Option<usize>,
        checked_rows: Vec<usize>,
        empty_text: String,
        current_page: Option<usize>,
        total: Option<usize>,
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
    // 图表 capability 启用时才保留柱状图快照变体。
    #[cfg(feature = "charts")]
    // 启用后记录柱状图数据与布局配置。
    BarChart {
        data: Vec<BarData>,
        fixed_width: f32,
        fixed_height: f32,
        max_value: f32,
        show_value: bool,
        bar_radius: f32,
    },
    // 图表 capability 启用时才保留折线图快照变体。
    #[cfg(feature = "charts")]
    // 启用后记录折线图数据、布局与样式配置。
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
    // 图表 capability 启用时才保留饼图快照变体。
    #[cfg(feature = "charts")]
    // 启用后记录饼图数据与环形布局配置。
    PieChart {
        data: Vec<PieData>,
        fixed_size: f32,
        hole_radius: f32,
    },
    // 图表 capability 启用时才保留高级图表快照变体。
    #[cfg(feature = "charts")]
    // 启用后记录高级图表的标题与类型摘要。
    ChartPlaceholder {
        title: String,
        subtitle: String,
        kind_name: &'static str,
    },
    // 关闭二维码 capability 时同步收缩公开快照枚举。
    #[cfg(feature = "qrcode")]
    // 启用后保留二维码编码与矩阵状态字段。
    QRCode {
        value: String,
        size: f32,
        error_level: u8,
        module_count: usize,
        encoding_error: Option<String>,
    },
    // 关闭富文本 capability 时同步收缩公开快照枚举。
    #[cfg(feature = "rich-text")]
    // 启用后保留富文本内容、样式与焦点状态字段。
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
        max_size: Option<u64>,
        show_upload_list: bool,
        preview_image: bool,
        manual: bool,
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

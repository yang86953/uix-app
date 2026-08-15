use std::sync::Arc;

use crate::draw::Color;
use crate::draw::geometry::spatial::PhysicalUnit;
// 反馈 capability 启用时才需要警告提示状态级别。
#[cfg(feature = "feedback")]
use crate::platform::capabilities::StatusLevel;
use crate::platform::windowing::{ControlSize, ScrollDirection};
use crate::ui::form::FormLayout;
use crate::ui::layout::{AlignItems, FlexDirection, JustifyContent};
// 引入主题感知颜色值与通用样式快照类型。
use crate::ui::theme::style::{ColorValue, Style, StyleSet};
use crate::ui::widgets::window_chrome::WindowControl;
use crate::ui::widgets::*;
// FloatButton 与可选反馈组件共同使用公开浮层方位。
use crate::ui::Placement;
// Modal 与 Drawer 快照保存统一 overlay backdrop 请求。
#[cfg(feature = "feedback")]
use crate::ui::OverlayBackdropBlur;

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
/// 内置与自定义组件用于比较、语义派生和声明协调的类型化字段快照。
pub enum SnapshotFields {
    /// 组件没有提供可识别的快照字段。
    Unknown,
    /// 由公开组件宏捕获的自定义组件字段。
    Custom {
        /// 自定义组件声明的静态类型名称。
        widget: &'static str,
        /// 按组件声明顺序捕获的字段集合。
        fields: Vec<SnapshotField>,
    },
    /// 按钮的内容、状态与样式快照。
    Button {
        /// 按钮显示的文字。
        text: String,
        /// 按钮是否禁止交互。
        disabled: bool,
        /// 按钮是否填满可用横向空间。
        block: bool,
        /// 按钮是否处于加载状态。
        loading: bool,
        /// 按钮声明的图标名称。
        icon: String,
        /// 按钮在组合按钮中的边缘位置。
        group_position: Option<ButtonGroupPosition>,
        /// 按钮使用的交互状态样式集合。
        style_set: Arc<StyleSet>,
        /// 按钮当前的基础样式。
        style: Arc<Style>,
    },
    /// 原生窗口控制按钮的动作与无障碍名称快照。
    WindowControl {
        /// 窗口控制按钮触发的原生窗口动作。
        control: WindowControl,
        /// 窗口控制按钮向无障碍消费者公开的名称。
        accessible_name: String,
    },
    /// 标签的文字、字体、颜色与固定尺寸快照。
    Label {
        /// 标签显示的文字。
        text: String,
        /// 标签声明的基础字号。
        font_size: f32,
        /// 字号使用的可选物理单位。
        font_size_unit: Option<PhysicalUnit>,
        /// 标签显式声明的文字颜色。
        color: Option<Color>,
        /// 标签显式声明的固定宽度。
        fixed_width: Option<f32>,
        /// 标签显式声明的固定高度。
        fixed_height: Option<f32>,
        /// 标签可选的完整样式覆写。
        style: Option<Style>,
    },
    /// 文本输入组件的装饰、能力与校验状态快照。
    Input {
        /// 输入为空时显示的占位文字。
        placeholder: String,
        /// 输入控件采用的标准尺寸档位。
        input_size: ControlSize,
        /// 输入控件是否禁止交互。
        disabled: bool,
        /// 输入框前缀装饰文字。
        prefix: String,
        /// 输入框后缀装饰文字。
        suffix: String,
        /// 输入控件前置附加内容。
        addon_before: String,
        /// 输入控件后置附加内容。
        addon_after: String,
        /// 输入是否按密码模式隐藏内容。
        password: bool,
        /// 密码模式下内容当前是否可见。
        password_visible: bool,
        /// 输入是否提供一键清空动作。
        clearable: bool,
        /// 输入是否提供搜索提交动作。
        search: bool,
        /// 输入当前的可选校验状态。
        status: Option<InputStatus>,
        /// 与校验状态关联的提示文字。
        status_message: String,
        /// 输入是否采用多行文本区域模式。
        textarea: bool,
        /// 多行模式声明的可见行数。
        textarea_rows: usize,
        /// 输入允许的可选最大字符数。
        max_length: Option<usize>,
    },
    /// 间距容器的排列方向、间隔与尺寸策略快照。
    Space {
        /// 子项排列采用的主轴方向。
        direction: FlexDirection,
        /// 子项之间采用的标准间距档位。
        space_size: SpaceSize,
        /// 空间不足时子项是否允许换行。
        wrap: bool,
        /// 子项沿主轴的分布方式。
        justify: JustifyContent,
        /// 子项沿交叉轴的对齐方式。
        align: AlignItems,
        /// 容器显式声明的固定宽度。
        fixed_width: Option<f32>,
        /// 容器显式声明的固定高度。
        fixed_height: Option<f32>,
        /// 容器参与父级弹性分配的增长权重。
        flex_grow: f32,
    },
    /// 分隔线的文字、方向与视觉配置快照。
    Divider {
        /// 分隔线上可选的说明文字。
        text: Option<String>,
        /// 说明文字在分隔线上的位置。
        orientation: DividerOrientation,
        /// 分隔线采用水平或垂直方向。
        direction: DividerDirection,
        /// 分隔线显式声明的颜色。
        color: Option<Color>,
        /// 分隔线说明文字的字号。
        text_size: f32,
        /// 分隔线是否采用虚线样式。
        dashed: bool,
    },
    /// 图标的资源名称与显示尺寸快照。
    Icon {
        /// 图标资源的稳定名称。
        name: String,
        /// 图标声明的方形边长。
        size: f32,
    },
    /// 富文本排版组件的内容、修饰与颜色快照。
    Typography {
        /// 组件显示的文字内容。
        content: String,
        /// 文字采用的语义排版类型。
        type_: TypographyType,
        /// 文字交互是否禁用。
        disabled: bool,
        /// 文字是否使用标记高亮样式。
        mark: bool,
        /// 文字是否使用行内代码样式。
        code: bool,
        /// 文字是否显示下划线。
        underline: bool,
        /// 文字是否显示删除线。
        delete: bool,
        /// 文字是否使用加粗字重。
        strong: bool,
        /// 文字是否使用斜体样式。
        italic: bool,
        /// 文字是否提供复制动作。
        copyable: bool,
        /// 保存主题感知的语义文字颜色身份。
        semantic_color: Option<ColorValue>,
        /// 显式指定且优先于语义颜色的固定颜色。
        color_override: Option<Color>,
    },
    /// 复选框的选中、禁用与标签快照。
    Checkbox {
        /// 复选框当前是否选中。
        checked: bool,
        /// 复选框是否禁止交互。
        disabled: bool,
        /// 复选框关联的显示标签。
        label: String,
    },
    /// 单选组的候选项、选择状态与布局快照。
    Radio {
        /// 互斥单选组的稳定名称。
        group_name: String,
        /// 按声明顺序保存的候选项标签。
        options: Vec<String>,
        /// 当前选中候选项的索引。
        selected: usize,
        /// 整个单选组是否禁止交互。
        disabled: bool,
        /// 候选项采用的排列方向。
        direction: RadioDirection,
        /// 每个候选项声明的行高。
        item_h: f32,
    },
    /// 开关的名称、状态与尺寸快照。
    Switch {
        /// 展示名称供无障碍 name 播报；空值表示无名称开关。
        label: String,
        /// 开关当前是否开启。
        checked: bool,
        /// 开关是否禁止交互。
        disabled: bool,
        /// 开关声明的视觉尺寸。
        size: f32,
    },
    /// 单值滑块的范围、步长、标记与提示配置快照。
    Slider {
        /// 滑块允许的最小值。
        min: f64,
        /// 滑块允许的最大值。
        max: f64,
        /// 键盘或步进操作采用的增量。
        step: f64,
        /// 滑块当前值。
        value: f64,
        /// 按数值定位的标记及其显示文本。
        marks: Vec<(f64, String)>,
        /// 当前值提示气泡的可选方位。
        tooltip: Option<crate::ui::widgets::TooltipPlacement>,
    },
    /// 双端范围滑块的边界、值与活动端点快照。
    RangeSlider {
        /// 范围滑块允许的最小值。
        min: f64,
        /// 范围滑块允许的最大值。
        max: f64,
        /// 键盘或步进操作采用的增量。
        step: f64,
        /// 当前选中范围的起始值。
        start: f64,
        /// 当前选中范围的结束值。
        end: f64,
        /// 当前接收键盘操作的滑块端点。
        active_thumb: RangeSliderThumb,
        /// 范围滑块采用的标准尺寸档位。
        size: ControlSize,
    },
    /// 评分组件的数量、取值与交互配置快照。
    Rate {
        /// 可供选择的评分符号总数。
        count: usize,
        /// 当前选中的完整评分数量。
        value: usize,
        /// 是否允许选择半个评分单位。
        half: bool,
        /// 评分组件是否禁止交互。
        disabled: bool,
        /// 再次选择当前值时是否允许清空评分。
        clearable: bool,
        /// 每个评分单位显示的字符或图标文本。
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
        // 保存声明式 Modal 的可选效果请求。
        backdrop_blur: Option<OverlayBackdropBlur>,
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
        // 保存声明式 Drawer 的可选效果请求。
        backdrop_blur: Option<OverlayBackdropBlur>,
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

use crate::ui::form::{Form, FormItem};
use crate::ui::virtualization::virtual_scroll::VirtualScroll;
use crate::ui::widgets::window_chrome::WindowInteractionRegion;
use crate::ui::widgets::{
    Affix, AutoComplete, Avatar, BackTop, Badge, Button, Calendar, Card, Carousel, Cascader,
    Checkbox, Collapse, ColorPicker, Container, Content, DatePicker, DateRangePicker, Descriptions,
    Divider, Empty, FloatButton, FloatButtonGroup, Footer, Grid, Header, Icon, Image, ImageGroup,
    Input, InputNumber, Label, Layout, List, Mentions, Radio, RangeSlider, Rate, ResultView,
    ScrollView, Segmented, Select, SelectableList, Sider, Skeleton, Slider, Space, Splitter,
    Switch, Tag, ThemeToggle, TimePicker, Timeline, Transfer, Typography, Upload, Watermark,
};
// 反馈 capability 启用时才引入对应 patch 目标类型。
#[cfg(feature = "feedback")]
// 该导入覆盖反馈组件族全部拥有运行时状态的组件。
use crate::ui::widgets::{
    Alert, Drawer, Message, MessageDeclaration, Modal, Notification, NotificationDeclaration,
    Popconfirm, Popover, ProgressBar, Spin, Tooltip,
};
// 图表 capability 启用时才引入对应 patch 目标类型。
#[cfg(feature = "charts")]
// 该导入覆盖基础图表与高级图表共用的占位组件。
use crate::ui::widgets::{BarChart, ChartPlaceholder, LineChart, PieChart};
// 表格 capability 启用时才引入对应 patch 目标类型。
#[cfg(feature = "table")]
// 该导入仅供内建组件 patch 分派使用。
use crate::ui::widgets::Table;
// 导航 capability 启用时才引入对应 patch 目标类型。
#[cfg(feature = "navigation")]
// 该导入覆盖全部拥有运行时组件状态的导航目标。
use crate::ui::widgets::{
    Anchor, Breadcrumb, Dropdown, Menu, MenuBar, NavItem, NavigationShell, Pagination, Steps, Tabs,
};
// 二维码 capability 启用时才引入对应组件类型。
#[cfg(feature = "qrcode")]
// 该导入仅供内建组件 patch 分派使用。
use crate::ui::widgets::QRCode;
// 富文本 capability 启用时才引入对应组件类型。
#[cfg(feature = "rich-text")]
// 该导入仅供内建组件 patch 分派使用。
use crate::ui::widgets::RichText;
// 树组件 capability 启用时才引入展示树与树选择器 patch 目标。
use crate::ui::SnapshotFields;
use crate::ui::Widget;
#[cfg(feature = "tree-widgets")]
// 两个类型共享同一源码与公开面门禁。
use crate::ui::widgets::{Tree, TreeSelect};
// 终端 capability 启用时才引入终端 patch 目标。
#[cfg(feature = "terminal")]
// 该类型与终端快照变体共享同一门禁。
use crate::ui::widgets::Terminal;
// 导入区分布局字段与纯绘制字段所需的统一样式快照。
use crate::ui::theme::style::Style;

// 在已知具体类型时比较无需拥有型快照的内建组件作者配置。
pub(crate) fn builtin_widget_config_changed_without_snapshot(
    current: &dyn Widget,
    next: &dyn Widget,
) -> Option<bool> {
    #[cfg(feature = "terminal")]
    if let (Some(current), Some(next)) = (
        current.as_any().downcast_ref::<Terminal>(),
        next.as_any().downcast_ref::<Terminal>(),
    ) {
        // 排除草稿与历史，保留语义快照刻意省略的视觉字段。
        return Some(current.reconcile_config_changed(next));
    }
    let current = current.as_any().downcast_ref::<Button>()?;
    let next = next.as_any().downcast_ref::<Button>()?;
    Some(!current.has_same_reconcile_config(next))
}

// 终端输出变化只重绘；尺寸与 Flex 变化才请求父级重新布局。
pub(crate) fn builtin_widget_layout_changed_without_snapshot(
    current: &dyn Widget,
    next: &dyn Widget,
) -> Option<bool> {
    if let (Some(current), Some(next)) = (
        current.as_any().downcast_ref::<Label>(),
        next.as_any().downcast_ref::<Label>(),
    ) {
        // 私有排版策略没有公开快照字段；变化必须触发布局，未变时继续比较其余样式。
        if current.typography_changed(next) {
            return Some(true);
        }
    }

    #[cfg(feature = "terminal")]
    if let (Some(current), Some(next)) = (
        current.as_any().downcast_ref::<Terminal>(),
        next.as_any().downcast_ref::<Terminal>(),
    ) {
        return Some(current.reconcile_layout_changed(next));
    }
    let _ = (current, next);
    None
}

/// Compare authored configuration for widgets whose public snapshot also
/// contains runtime state. `None` falls back to ordinary snapshot equality.
pub(crate) fn builtin_widget_config_changed(
    current: &SnapshotFields,
    next: &SnapshotFields,
) -> Option<bool> {
    match (current, next) {
        // Badge 快照包含布局后子存在事实与最终装饰边界，配置比较必须排除运行态。
        (
            SnapshotFields::Badge {
                count: current_count,
                max: current_max,
                dot: current_dot,
                color: current_color,
                adaptive_foreground: current_adaptive_foreground,
                ribbon: current_ribbon,
                status: current_status,
                show_zero: current_show_zero,
                text: current_text,
                offset_x: current_offset_x,
                offset_y: current_offset_y,
                offset_unit: current_offset_unit,
                composite: current_composite,
                ..
            },
            SnapshotFields::Badge {
                count: next_count,
                max: next_max,
                dot: next_dot,
                color: next_color,
                adaptive_foreground: next_adaptive_foreground,
                ribbon: next_ribbon,
                status: next_status,
                show_zero: next_show_zero,
                text: next_text,
                offset_x: next_offset_x,
                offset_y: next_offset_y,
                offset_unit: next_offset_unit,
                composite: next_composite,
                ..
            },
        ) => Some(
            // 只比较作者声明配置，忽略 child_present 与 decoration_bounds。
            current_count != next_count
                // 最大计数改变可见标签。
                || current_max != next_max
                // 圆点模式改变绘制形态。
                || current_dot != next_dot
                // 自定义颜色改变装饰绘制。
                || current_color != next_color
                // 自适应前景策略改变文字颜色。
                || current_adaptive_foreground != next_adaptive_foreground
                // 丝带模式改变装饰几何。
                || current_ribbon != next_ribbon
                // 状态 marker 改变语义与绘制。
                || current_status != next_status
                // 零值显隐改变装饰存在性。
                || current_show_zero != next_show_zero
                // 文字改变测量、语义与绘制。
                || current_text != next_text
                // 像素横向偏移改变装饰位置。
                || current_offset_x != next_offset_x
                // 像素纵向偏移改变装饰位置。
                || current_offset_y != next_offset_y
                // 物理单位偏移改变 DPI 解析位置。
                || current_offset_unit != next_offset_unit
                // 叶与组合模式切换改变布局所有权。
                || current_composite != next_composite,
        ),
        (
            SnapshotFields::Collapse {
                panels: current_panels,
                accordion: current_accordion,
                ..
            },
            SnapshotFields::Collapse {
                panels: next_panels,
                accordion: next_accordion,
                ..
            },
        ) => Some(
            current_accordion != next_accordion
                || current_panels.len() != next_panels.len()
                || current_panels
                    .iter()
                    .zip(next_panels)
                    .any(|(current, next)| {
                        current.header != next.header || current.content != next.content
                    }),
        ),
        (
            SnapshotFields::Carousel {
                show_dots: current_dots,
                show_arrows: current_arrows,
                fixed_width: current_width,
                fixed_height: current_height,
                ..
            },
            SnapshotFields::Carousel {
                show_dots: next_dots,
                show_arrows: next_arrows,
                fixed_width: next_width,
                fixed_height: next_height,
                ..
            },
        ) => Some(
            current_dots != next_dots
                || current_arrows != next_arrows
                || current_width != next_width
                || current_height != next_height,
        ),
        (
            SnapshotFields::ImageGroup {
                images: current_images,
                start_index: current_start,
                ..
            },
            SnapshotFields::ImageGroup {
                images: next_images,
                start_index: next_start,
                ..
            },
        ) => Some(current_images != next_images || current_start != next_start),
        // ScrollView 快照同时含作者布局配置和 live 滚动位置，配置比较必须排除偏移。
        (
            // 解构当前声明配置并忽略运行态滚动坐标。
            SnapshotFields::ScrollView {
                // 当前允许滚动的轴向。
                direction: current_direction,
                // 当前显式视口宽度。
                fixed_width: current_width,
                // 当前显式视口高度。
                fixed_height: current_height,
                // 当前父级弹性增长权重。
                flex_grow: current_flex_grow,
                // 当前父级弹性收缩权重。
                flex_shrink: current_flex_shrink,
                // 当前滚动条可见配置。
                show_scrollbar: current_show_scrollbar,
                // live 横向偏移不属于作者静态配置。
                scroll_x: _,
                // live 纵向偏移不属于作者静态配置。
                scroll_y: _,
            },
            // 解构下一声明配置并忽略最新受控滚动坐标。
            SnapshotFields::ScrollView {
                // 下一声明允许滚动的轴向。
                direction: next_direction,
                // 下一声明显式视口宽度。
                fixed_width: next_width,
                // 下一声明显式视口高度。
                fixed_height: next_height,
                // 下一声明父级弹性增长权重。
                flex_grow: next_flex_grow,
                // 下一声明父级弹性收缩权重。
                flex_shrink: next_flex_shrink,
                // 下一声明滚动条可见配置。
                show_scrollbar: next_show_scrollbar,
                // 受控横向偏移由运行态比较负责。
                scroll_x: _,
                // 受控纵向偏移由运行态比较负责。
                scroll_y: _,
            },
        ) => Some(
            // 轴向变化会改变滚动、裁剪和滚动条布局。
            current_direction != next_direction
                // 固定宽度变化会改变视口几何。
                || current_width != next_width
                // 固定高度变化会改变视口几何。
                || current_height != next_height
                // 父级增长权重变化会改变分配尺寸。
                || current_flex_grow != next_flex_grow
                // 父级收缩权重变化会改变分配尺寸。
                || current_flex_shrink != next_flex_shrink
                // 滚动条可见配置会改变内容沟槽。
                || current_show_scrollbar != next_show_scrollbar,
        ),
        _ => None,
    }
}

// 比较统一样式中会改变测量、放置或可见性的字段。
fn style_layout_changed(current: &Style, next: &Style) -> bool {
    // 盒模型字段直接改变内容区域或父级占位。
    current.margin != next.margin
        || current.padding != next.padding
        || current.border_width != next.border_width
        // 固定尺寸字段改变组件约束结果。
        || current.width != next.width
        || current.height != next.height
        // 容器布局字段改变 Flex 或 Grid 求解结果。
        || current.display != next.display
        || current.flex_direction != next.flex_direction
        || current.flex_wrap != next.flex_wrap
        || current.overflow_content != next.overflow_content
        || current.justify_content != next.justify_content
        || current.align_items != next.align_items
        || current.gap != next.gap
        || current.grid_template_columns != next.grid_template_columns
        || current.grid_template_rows != next.grid_template_rows
        || current.grid_column_gap != next.grid_column_gap
        || current.grid_row_gap != next.grid_row_gap
        // 子项布局字段改变父容器对当前节点的分配。
        || current.flex_grow != next.flex_grow
        || current.flex_shrink != next.flex_shrink
        || current.align_self != next.align_self
        || current.grid_cell != next.grid_cell
        || current.grid_column_span != next.grid_column_span
        || current.grid_row_span != next.grid_row_span
        // 字号参与文本组件的固有尺寸测量。
        || current.font_size != next.font_size
        // 显式行高参与文本行盒与多行测量。
        || current.line_height != next.line_height
        // 可见性决定节点是否参与布局。
        || current.visible != next.visible
}

// 比较可选样式；样式是否存在会改变 Label 的字号回退语义。
fn optional_style_layout_changed(current: &Option<Style>, next: &Option<Style>) -> bool {
    // 同时存在时只比较布局相关字段，同时缺失时保持布局稳定。
    match (current, next) {
        // 两份样式使用统一的几何字段比较。
        (Some(current), Some(next)) => style_layout_changed(current, next),
        // 两边都没有样式时没有布局变化。
        (None, None) => false,
        // 样式出现或消失会改变 Label 的字号与尺寸回退。
        _ => true,
    }
}

/// 返回已审计内建组件的布局变化；其余组件显式归入保守 Layout。
pub(crate) fn builtin_widget_layout_changed(
    current: &SnapshotFields,
    next: &SnapshotFields,
) -> Option<bool> {
    // 只对已核对 measure/layout 字段的快照类型作精细分类。
    match (current, next) {
        // Label 的颜色是纯绘制字段，其余公开快照字段参与度量或盒模型。
        (
            SnapshotFields::Label {
                text: current_text,
                font_size: current_font_size,
                font_size_unit: current_font_size_unit,
                color: _,
                fixed_width: current_width,
                fixed_height: current_height,
                style: current_style,
            },
            SnapshotFields::Label {
                text: next_text,
                font_size: next_font_size,
                font_size_unit: next_font_size_unit,
                color: _,
                fixed_width: next_width,
                fixed_height: next_height,
                style: next_style,
            },
        ) => Some(
            // 文本、字号和固定尺寸都会改变固有测量。
            current_text != next_text
                || current_font_size != next_font_size
                || current_font_size_unit != next_font_size_unit
                || current_width != next_width
                || current_height != next_height
                // 样式只比较布局相关字段，颜色等视觉字段留在 Paint。
                || optional_style_layout_changed(current_style, next_style),
        ),
        // Container 的声明快照完全由统一样式组成。
        (
            SnapshotFields::Container {
                style: current_style,
            },
            SnapshotFields::Container { style: next_style },
        ) => Some(style_layout_changed(current_style, next_style)),
        // Grid 除统一样式外，响应式断点与列声明也参与放置。
        (
            SnapshotFields::Grid {
                style: current_style,
                breakpoints: current_breakpoints,
                cols: current_cols,
            },
            SnapshotFields::Grid {
                style: next_style,
                breakpoints: next_breakpoints,
                cols: next_cols,
            },
        ) => Some(
            // 统一样式、断点或响应列变化都必须重新布局。
            style_layout_changed(current_style, next_style)
                || current_breakpoints != next_breakpoints
                || current_cols != next_cols,
        ),
        // ScrollView 的偏移是局部合成运行态，只有视口配置变化才请求布局。
        (
            // 解构当前视口配置并忽略 live 偏移。
            SnapshotFields::ScrollView {
                // 当前滚动轴向。
                direction: current_direction,
                // 当前固定宽度。
                fixed_width: current_width,
                // 当前固定高度。
                fixed_height: current_height,
                // 当前增长权重。
                flex_grow: current_flex_grow,
                // 当前收缩权重。
                flex_shrink: current_flex_shrink,
                // 当前滚动条可见配置。
                show_scrollbar: current_show_scrollbar,
                // 横向偏移不参与布局分类。
                scroll_x: _,
                // 纵向偏移不参与布局分类。
                scroll_y: _,
            },
            // 解构下一视口配置并忽略受控偏移。
            SnapshotFields::ScrollView {
                // 下一滚动轴向。
                direction: next_direction,
                // 下一固定宽度。
                fixed_width: next_width,
                // 下一固定高度。
                fixed_height: next_height,
                // 下一增长权重。
                flex_grow: next_flex_grow,
                // 下一收缩权重。
                flex_shrink: next_flex_shrink,
                // 下一滚动条可见配置。
                show_scrollbar: next_show_scrollbar,
                // 受控横向偏移只触发 Paint 或 Composite。
                scroll_x: _,
                // 受控纵向偏移只触发 Paint 或 Composite。
                scroll_y: _,
            },
        ) => Some(
            // 轴向会改变内容测量与滚动条放置。
            current_direction != next_direction
                // 固定宽度属于视口盒模型。
                || current_width != next_width
                // 固定高度属于视口盒模型。
                || current_height != next_height
                // 增长权重参与父级 Flex 求解。
                || current_flex_grow != next_flex_grow
                // 收缩权重参与父级 Flex 求解。
                || current_flex_shrink != next_flex_shrink
                // 滚动条沟槽会改变可用内容范围。
                || current_show_scrollbar != next_show_scrollbar,
        ),
        // Icon 的名称与尺寸参与固有测量；颜色是纯绘制字段，留在 Paint。
        (
            SnapshotFields::Icon {
                name: current_name,
                size: current_size,
                color: _,
            },
            SnapshotFields::Icon {
                name: next_name,
                size: next_size,
                color: _,
            },
        ) => Some(current_name != next_name || current_size != next_size),
        // 其余所有快照类型显式归入保守布局，避免新增组件静默漏掉几何失效。
        _ => Some(true),
    }
}

pub(crate) fn builtin_widget_runtime_changed(current: &dyn Widget, next: &dyn Widget) -> bool {
    if let (Some(current), Some(next)) = (
        current.as_any().downcast_ref::<Label>(),
        next.as_any().downcast_ref::<Label>(),
    ) {
        return current.typography_changed(next);
    }

    #[cfg(feature = "rich-text")]
    if let (Some(current), Some(next)) = (
        current.as_any().downcast_ref::<RichText>(),
        next.as_any().downcast_ref::<RichText>(),
    ) {
        return current.view_layout_changed(next);
    }

    // Input 的受控文本值属于运行态而不是作者静态配置。
    if let (Some(current), Some(next)) = (
        current.as_any().downcast_ref::<Input>(),
        next.as_any().downcast_ref::<Input>(),
    ) {
        // 把外部值或快照跳过的 View 布局变化交给统一 patch 失效传播。
        return current.controlled_value_changed(next) || current.view_layout_changed(next);
    }
    // Select 的直接尺寸保存在私有布局字段中，不进入含运行态的公开快照。
    if let (Some(current), Some(next)) = (
        current.as_any().downcast_ref::<Select>(),
        next.as_any().downcast_ref::<Select>(),
    ) {
        // 显式尺寸或 Flex 变化必须触发布局并由 sync_from 采用新值。
        return current.view_layout_changed(next);
    }
    // Collapse 的受控展开集合也会改变父子布局位置与可见性。
    if let (Some(current), Some(next)) = (
        // 读取当前 live Collapse。
        current.as_any().downcast_ref::<Collapse>(),
        // 读取已采用最新外部 key 的下一声明。
        next.as_any().downcast_ref::<Collapse>(),
    ) {
        // 由稳定 key 差异触发统一 Layout 失效。
        return current.controlled_expansion_changed(next);
    }
    // ScrollView 的受控偏移变化属于运行态局部合成，不是作者布局配置变化。
    if let (Some(current), Some(next)) = (
        // 读取当前 live ScrollView。
        current.as_any().downcast_ref::<ScrollView>(),
        // 读取已经采样最新外部偏移的下一声明。
        next.as_any().downcast_ref::<ScrollView>(),
    ) {
        // 由专属契约判断下一声明是否确实控制并改变偏移。
        return current.controlled_offset_changed(next);
    }
    // 其余内建组件当前没有独立于声明配置的受控运行态差异。
    false
}

std::thread_local! {
    // 缓存最近一次完整分派后确认的非内建类型；同类自定义同级节点无需重复扫描类型表。
    static LAST_UNPATCHED_WIDGET_TYPE: std::cell::Cell<Option<std::any::TypeId>> =
        const { std::cell::Cell::new(None) };
}

pub(crate) fn patch_builtin_widget(
    current: &mut dyn Widget,
    next: Box<dyn Widget>,
) -> std::result::Result<bool, Box<dyn Widget>> {
    let next_type = next.as_any().type_id();
    if LAST_UNPATCHED_WIDGET_TYPE.with(|cached| cached.get() == Some(next_type)) {
        // 类型表在进程生命周期内不可变，已确认的负命中可直接保持既有替换语义。
        return Err(next);
    }
    macro_rules! patch_as {
        ($ty:ty) => {
            if current.as_any().is::<$ty>() && next.as_any().is::<$ty>() {
                let Some(current) = current.as_any_mut().downcast_mut::<$ty>() else {
                    return Err(next);
                };
                let Ok(next) = next.into_any().downcast::<$ty>() else {
                    return Ok(false);
                };
                current.sync_from(*next);
                return Ok(true);
            }
        };
    }

    patch_as!(Button);
    patch_as!(VirtualScroll);
    patch_as!(Container);
    patch_as!(WindowInteractionRegion);
    patch_as!(Grid);
    patch_as!(Layout);
    patch_as!(Header);
    patch_as!(Sider);
    patch_as!(Content);
    patch_as!(Footer);
    patch_as!(Affix);
    patch_as!(Splitter);
    patch_as!(Divider);
    patch_as!(Icon);
    patch_as!(Typography);
    patch_as!(Avatar);
    patch_as!(Badge);
    patch_as!(Input);
    patch_as!(Checkbox);
    patch_as!(Radio);
    patch_as!(Switch);
    patch_as!(Slider);
    patch_as!(RangeSlider);
    patch_as!(InputNumber);
    patch_as!(Rate);
    patch_as!(Segmented);
    patch_as!(FormItem);
    patch_as!(Form);
    // 表格 capability 启用时才生成类型化 patch 分支。
    #[cfg(feature = "table")]
    // 启用后保持表格组件的原位同步语义。
    patch_as!(Table);
    // 反馈 capability 启用时才生成进度条类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(ProgressBar);
    // 反馈 capability 启用时才生成加载指示器类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Spin);
    patch_as!(FloatButton);
    patch_as!(FloatButtonGroup);
    patch_as!(BackTop);
    // 反馈 capability 启用时才生成警告提示类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Alert);
    patch_as!(Tag);
    patch_as!(Empty);
    patch_as!(Skeleton);
    patch_as!(Card);
    patch_as!(Image);
    patch_as!(ImageGroup);
    patch_as!(Calendar);
    patch_as!(Collapse);
    patch_as!(Carousel);
    patch_as!(Descriptions);
    patch_as!(ResultView);
    patch_as!(List);
    patch_as!(Timeline);
    // 树组件 capability 启用时才生成展示树类型化 patch 分支。
    #[cfg(feature = "tree-widgets")]
    // 启用后保持展示树的原位同步语义。
    patch_as!(Tree);
    // 终端 capability 启用时才生成终端类型化 patch 分支。
    #[cfg(feature = "terminal")]
    // 启用后保持终端的原位同步语义。
    patch_as!(Terminal);
    patch_as!(SelectableList);
    // 图表 capability 启用时才生成柱状图类型化 patch 分支。
    #[cfg(feature = "charts")]
    // 启用后保持柱状图的原位同步语义。
    patch_as!(BarChart);
    // 图表 capability 启用时才生成折线图类型化 patch 分支。
    #[cfg(feature = "charts")]
    // 启用后保持折线图的原位同步语义。
    patch_as!(LineChart);
    // 图表 capability 启用时才生成饼图类型化 patch 分支。
    #[cfg(feature = "charts")]
    // 启用后保持饼图的原位同步语义。
    patch_as!(PieChart);
    // 图表 capability 启用时才生成高级图表占位组件 patch 分支。
    #[cfg(feature = "charts")]
    // 启用后保持高级图表的原位同步语义。
    patch_as!(ChartPlaceholder);
    // 关闭二维码 capability 时不生成类型化 patch 分支。
    #[cfg(feature = "qrcode")]
    // 启用后保持二维码组件的原位同步语义。
    patch_as!(QRCode);
    patch_as!(Watermark);
    patch_as!(Label);
    patch_as!(ScrollView);
    patch_as!(Select);
    patch_as!(Space);
    // 反馈 capability 启用时才生成文字提示类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Tooltip);
    // 反馈 capability 启用时才生成气泡卡片类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Popover);
    // 反馈 capability 启用时才生成气泡确认框类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Popconfirm);
    // 反馈 capability 启用时才生成对话框类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Modal);
    // 反馈 capability 启用时才生成抽屉类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Drawer);
    // 导航 capability 启用时才生成下拉菜单类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(Dropdown);
    // 导航 capability 启用时才生成菜单栏类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(MenuBar);
    // 导航 capability 启用时才生成菜单类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(Menu);
    // 导航 capability 启用时才生成标签页类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(Tabs);
    // 导航 capability 启用时才生成分页类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(Pagination);
    // 导航 capability 启用时才生成面包屑类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(Breadcrumb);
    // 导航 capability 启用时才生成导航项类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(NavItem);
    // 导航 capability 启用时才生成 Navigation 外壳 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(NavigationShell);
    // 导航 capability 启用时才生成锚点类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(Anchor);
    // 导航 capability 启用时才生成步骤条类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(Steps);
    // 树组件 capability 启用时才生成树选择器类型化 patch 分支。
    #[cfg(feature = "tree-widgets")]
    // 启用后保持树选择器的原位同步语义。
    patch_as!(TreeSelect);
    patch_as!(Cascader);
    patch_as!(ColorPicker);
    patch_as!(AutoComplete);
    patch_as!(DatePicker);
    patch_as!(DateRangePicker);
    patch_as!(TimePicker);
    patch_as!(Mentions);
    // 反馈 capability 启用时才生成全局消息容器类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Message);
    // 反馈 capability 启用时才生成通知容器类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Notification);
    // 反馈 capability 启用时保持 Message 声明租约原位同步。
    #[cfg(feature = "feedback")]
    patch_as!(MessageDeclaration);
    // 反馈 capability 启用时保持 Notification 声明租约原位同步。
    #[cfg(feature = "feedback")]
    patch_as!(NotificationDeclaration);
    // 富文本 capability 启用时才参与内建组件 patch 分派。
    #[cfg(feature = "rich-text")]
    patch_as!(RichText);
    patch_as!(ThemeToggle);
    patch_as!(Transfer);
    patch_as!(Upload);

    LAST_UNPATCHED_WIDGET_TYPE.with(|cached| cached.set(Some(next_type)));
    Err(next)
}

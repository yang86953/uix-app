// 引入过程宏令牌流。
use proc_macro2::TokenStream;

// 引入核心文本、按钮与图标生成 helper。
use super::codegen::{generate_button, generate_icon, generate_text};
// 引入所有独立组件生成器、语法树与诊断入口。
use super::{
    generate_affix, generate_alert, generate_anchor, generate_app_layout, generate_autocomplete,
    generate_avatar, generate_back_top, generate_badge, generate_basic_chart, generate_breadcrumb,
    generate_button_group, generate_calendar, generate_card, generate_carousel, generate_cascader,
    generate_checkbox, generate_color_picker, generate_column, generate_container,
    generate_date_picker, generate_date_range_picker, generate_descriptions, generate_divider,
    generate_drawer, generate_dropdown, generate_empty, generate_float_button,
    generate_float_button_group, generate_focus_trap, generate_form, generate_grid,
    generate_hierarchy_gauge_chart, generate_input, generate_input_group, generate_input_number,
    generate_kernel_host, generate_kernel_view, generate_list, generate_matrix_delta_chart,
    generate_mentions, generate_menu, generate_message_declaration, generate_modal,
    generate_navigation, generate_notification_declaration, generate_orphan_col,
    generate_orphan_form_checkbox_item, generate_orphan_form_input_item,
    generate_orphan_form_radio_item, generate_orphan_form_select_item,
    generate_orphan_form_slider_item, generate_orphan_form_switch_item, generate_pagination,
    generate_popconfirm, generate_popover, generate_progress_bar, generate_qrcode, generate_radio,
    generate_range_slider, generate_rate, generate_result_view, generate_rich_text, generate_row,
    generate_scroll_view, generate_segmented, generate_select, generate_series_chart,
    generate_skeleton, generate_slider, generate_space, generate_spin, generate_splitter,
    generate_static_chart, generate_steps, generate_switch, generate_table, generate_tabs,
    generate_tag, generate_theme_toggle, generate_time_picker, generate_timeline, generate_tooltip,
    generate_tree, generate_tree_select, generate_typography, generate_upload,
    generate_virtual_scroll, generate_watermark, generate_window_control,
    generate_window_drag_region, planned_builtin_diagnostic, supported_builtin_hint, Diagnostic,
    Element,
};
// 引入 SelectableList 独立生成入口。
use super::generate_selectable_list;
// 引入 Collapse 独立生成入口。
use super::generate_collapse;
// 引入 Image 叶组件专用生成入口。
use super::generate_image;
// 引入 ImageGroup 叶组件专用生成入口。
use super::generate_image_group;

// 按当前核心映射矩阵生成一个普通元素。
pub(super) fn generate_element(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 普通元素不应携带控制绑定。
    if element.control.is_some() {
        // 返回内部形状保护诊断。
        return Err(Diagnostic::new(
            // 指向完整元素。
            element.span,
            // 说明控制绑定归属错误。
            format!("<{}> 不能携带 If/For 控制绑定", element.name),
            // 给出合法控制标签。
            "只在 <If> 或 <For> 元素上声明控制绑定",
        ));
    }
    // 只登记可由现有公开 API 确定表达的组件。
    match element.name.as_str() {
        // 框架模板可把已经拥有行为的 Rust 基础 View 注入声明组合树。
        "KernelView" => generate_kernel_view(element),
        // 框架模板可把唯一 UIX 展示子树交给 Rust 基础内核包装。
        "KernelHost" => generate_kernel_host(element),
        // 文本映射到公开 label 构造器。
        "Text" => generate_text(element),
        // 文档通用组件 Label 与 Text 使用同一公开构造器。
        "Label" => generate_text(element),
        // 按钮映射到公开 button 构建器。
        "Button" => generate_button(element),
        // 通用容器按 direction 映射到 Flex row 或 column。
        "Container" => generate_container(element),
        // 文档 Row 映射到 24 单元响应式栅格。
        "Row" => generate_row(element),
        // 显式列容器保留 Flex column 兼容入口。
        "Column" => generate_column(element),
        // 显式 Grid 映射到公开轨道构建器。
        "Grid" => generate_grid(element),
        // 滚动容器映射到公开 ScrollBuilder，并保留状态所有权。
        "ScrollView" => generate_scroll_view(element),
        // 虚拟滚动映射到公开 VirtualScroll 惰性 renderer。
        "VirtualScroll" => generate_virtual_scroll(element),
        // 固钉容器映射到公开 Affix，并从 State<f32> 读取当前滚动位置。
        "Affix" => generate_affix(element),
        // 两个回到顶部标签统一映射到持有滚动状态的公开 BackTop。
        "BackTop" | "FloatButtonBackTop" => generate_back_top(element),
        // 双面板分隔器映射到公开 Splitter，并保留运行时交互所有权。
        "Splitter" => generate_splitter(element),
        // 应用布局壳五类标签共享公开运行时组合生成入口。
        "Layout" | "Sider" | "Header" | "Content" | "Footer" => generate_app_layout(element),
        // 文本输入映射到公开 Input 与 View Change 事件契约。
        "Input" => generate_input(element),
        // 数值输入映射到公开 InputNumber 绑定、范围、步长与精度契约。
        "InputNumber" => generate_input_number(element),
        // 复合文本输入映射到公开 InputGroup 附加文本与状态绑定契约。
        "InputGroup" => generate_input_group(element),
        // 单值滑块映射到公开 Slider 范围、步长与状态绑定契约。
        "Slider" => generate_slider(element),
        // 区间滑块映射到公开 RangeSlider 范围、步长与双状态绑定契约。
        "RangeSlider" => generate_range_slider(element),
        // 评分组件映射到公开 Rate 星数、半星与状态绑定契约。
        "Rate" => generate_rate(element),
        // 复选框映射到公开 Checkbox 标签、禁用与双向勾选契约。
        "Checkbox" => generate_checkbox(element),
        // 开关映射到公开 Switch 禁用与双向勾选契约。
        "Switch" => generate_switch(element),
        // 单选组映射到公开 Radio 选项与 State<String> 双向值绑定契约。
        "Radio" => generate_radio(element),
        // 分段控制器映射到公开 Segmented 选项与 State<String> 双向值绑定契约。
        "Segmented" => generate_segmented(element),
        // 下拉选择器映射到结构化选项和编译期单选或多选状态契约。
        "Select" => generate_select(element),
        // 级联选择器映射到选项树与 State<CascaderValue> 双向路径契约。
        "Cascader" => generate_cascader(element),
        // 树形选择器映射到 TreeNode 树与 State<String> 稳定 key 双向契约。
        "TreeSelect" => generate_tree_select(element),
        // 自动完成输入映射到字符串候选与 State<String> 双向文本契约。
        "AutoComplete" => generate_autocomplete(element),
        // 提及输入映射到字符串候选与 State<String> 完整文本双向契约。
        "Mentions" => generate_mentions(element),
        // 日期选择映射到 State<Date> 双向值与确定的选择粒度契约。
        "DatePicker" => generate_date_picker(element),
        // 日期范围映射到 start/end 两个 State<Date> 的结构化双向契约。
        "DateRangePicker" => generate_date_range_picker(element),
        // 时间选择映射到 State<Time> 双向值契约。
        "TimePicker" => generate_time_picker(element),
        // 颜色选择映射到 State<Color> 双向值契约。
        "ColorPicker" => generate_color_picker(element),
        // 头像映射到公开来源、回退文字、形状与固有尺寸契约。
        "Avatar" => generate_avatar(element),
        // 徽章映射到零或一个真实子 View 的透明装饰器契约。
        "Badge" => generate_badge(element),
        // 图片映射到公开来源、固有尺寸与运行时加载预览配置契约。
        "Image" => generate_image(element),
        // 图片组映射到公开路径集合、初始索引与运行时变化事实契约。
        "ImageGroup" => generate_image_group(element),
        // 文本列表映射到公开数据集合、兼容字符串、命名节点槽与空态替代契约。
        "List" => generate_list(element),
        // 可选中列表映射到类型化条目、可空稳定 id 状态与 Change 契约。
        "SelectableList" => generate_selectable_list(element),
        // 折叠组映射到类型化面板、稳定展开 key 状态与 Change 契约。
        "Collapse" => generate_collapse(element),
        // 上传组件映射到受控文件队列、确定过滤和类型化 Change 契约。
        "Upload" => generate_upload(element),
        // Message 映射为零布局 keyed 声明租约节点。
        "Message" => generate_message_declaration(element),
        // Notification 映射为零布局 keyed 声明租约节点。
        "Notification" => generate_notification_declaration(element),
        // 骨架屏映射到公开形状与固有尺寸契约。
        "Skeleton" => generate_skeleton(element),
        // 空状态映射到公开描述与图标契约。
        "Empty" => generate_empty(element),
        // 结果页映射到公开结果类型与文本契约。
        "ResultView" => generate_result_view(element),
        // 标签映射到公开文本、颜色与初始交互能力契约。
        "Tag" => generate_tag(element),
        // 卡片映射到公开标题、操作项与完整 View 子树契约。
        "Card" => generate_card(element),
        // 描述列表映射到类型化数据集合与确定列数契约。
        "Descriptions" => generate_descriptions(element),
        // 时间轴映射到类型化事件集合、pending 与 reverse 契约。
        "Timeline" => generate_timeline(element),
        // 日历映射到公开默认选择与月视图导航契约。
        "Calendar" => generate_calendar(element),
        // 轮播器映射到有序幻灯片子树与三秒自动播放契约。
        "Carousel" => generate_carousel(element),
        // 树形控件映射到类型化节点与初始勾选、展开契约。
        "Tree" => generate_tree(element),
        // 数据表格映射到类型化 TableRow / TableColumn 与首版运行配置。
        "Table" => generate_table(element),
        // 标签页映射到静态元数据、State<String> 活动 key 与有序面板子树。
        "Tabs" => generate_tabs(element),
        // 菜单映射到 typed MenuItem 树、双受控 key 状态与选择事件。
        "Menu" => generate_menu(element),
        // 下拉菜单映射到 keyed 数据、唯一 trigger 子树与 Change 稳定 key。
        "Dropdown" => generate_dropdown(element),
        // 侧栏导航映射到只拥有外壳状态并复用唯一受控 Menu 的组合 View。
        "Navigation" => generate_navigation(element),
        // 步骤条映射到类型化 Step 集合与 State<usize> 双向 current 契约。
        "Steps" => generate_steps(element),
        // 分页器映射到总条数、双 State<usize> 与既有 Change 载荷契约。
        "Pagination" => generate_pagination(element),
        // 面包屑映射到类型化 BreadcrumbItem 路径与末项当前页契约。
        "Breadcrumb" => generate_breadcrumb(element),
        // 锚点导航映射到类型化 AnchorItem、偏移与 href Change 契约。
        "Anchor" => generate_anchor(element),
        // 二维码映射到内容、文档尺寸与纠错等级契约。
        "QRCode" => generate_qrcode(element),
        // 水印映射到文字、透明度与运行时平铺绘制契约。
        "Watermark" => generate_watermark(element),
        // 富文本映射到公开 Markdown 解析器与运行时选择生命周期契约。
        "RichText" => generate_rich_text(element),
        // 警告提示映射到状态、关闭能力与关闭事件契约。
        "Alert" => generate_alert(element),
        // 进度条映射到有限 fraction、显式模式与 line/circle 形态契约。
        "ProgressBar" => generate_progress_bar(element),
        // 三类基础图表映射到现有静态 Chart Widget。
        "BarChart" | "LineChart" | "PieChart" => generate_basic_chart(element),
        // 三类静态高级图表映射到现有 Chart Module Widget。
        "AreaChart" | "ScatterChart" | "FunnelChart" => generate_static_chart(element),
        // 矩形树图与仪表盘映射到现有 Chart Module Widget。
        "Treemap" | "Gauge" => generate_hierarchy_gauge_chart(element),
        // 热力图与瀑布图映射到现有 Chart Module Widget。
        "Heatmap" | "WaterfallChart" => generate_matrix_delta_chart(element),
        // 雷达图与组合图映射到现有 Chart Module Widget。
        "RadarChart" | "ComboChart" => generate_series_chart(element),
        // 确认气泡映射到唯一真实 trigger、六向位置与同步操作回调契约。
        "Popconfirm" => generate_popconfirm(element),
        // 模态框映射到 State<bool>、同步操作回调与完整有序内容子树契约。
        "Modal" => generate_modal(element),
        // 抽屉映射到 State<bool>、方向、面板宽度与完整有序内容子树契约。
        "Drawer" => generate_drawer(element),
        // 文字提示映射到默认顶部悬停行为与唯一静态触发子树契约。
        "Tooltip" => generate_tooltip(element),
        // 气泡卡片映射到内容、点击或悬停触发与唯一静态触发子树契约。
        "Popover" => generate_popover(element),
        // 焦点陷阱映射到静态作用域身份与完整有序子树契约。
        "FocusTrap" => generate_focus_trap(element),
        // 加载指示器映射到声明配置、可选提示与完整有序遮罩子树契约。
        "Spin" => generate_spin(element),
        // 表单映射到类型化模型、字段投影与提交闭环。
        "Form" => generate_form(element),
        // FormInputItem 只能由 Form 解释类型化字段语义。
        "FormInputItem" => generate_orphan_form_input_item(element),
        // FormSelectItem 只能由 Form 解释类型化选择字段语义。
        "FormSelectItem" => generate_orphan_form_select_item(element),
        // FormCheckboxItem 只能由 Form 解释类型化布尔字段语义。
        "FormCheckboxItem" => generate_orphan_form_checkbox_item(element),
        // FormRadioItem 只能由 Form 解释类型化单选组语义。
        "FormRadioItem" => generate_orphan_form_radio_item(element),
        // FormSwitchItem 只能由 Form 解释类型化开关语义。
        "FormSwitchItem" => generate_orphan_form_switch_item(element),
        // FormSliderItem 只能由 Form 解释类型化 f64 滑块语义。
        "FormSliderItem" => generate_orphan_form_slider_item(element),
        // Col 只能由 Row 或 Grid 解释其父级布局语义。
        "Col" => generate_orphan_col(element),
        // 图标映射到公开 Icon 组件。
        "Icon" => generate_icon(element),
        // 分割线映射到现有 Divider Widget。
        "Divider" => generate_divider(element),
        // 间距容器映射到现有 Space Widget。
        "Space" => generate_space(element),
        // 排版文本映射到现有 Typography Widget。
        "Typography" => generate_typography(element),
        // 主题切换映射到现有 ThemeToggle Widget。
        "ThemeToggle" => generate_theme_toggle(element),
        // 按钮组映射到静态直接 Button 子项的连体组合。
        "ButtonGroup" => generate_button_group(element),
        // 窗口控制映射到 window_chrome 公开组合函数。
        "WindowControl" => generate_window_control(element),
        // 标题栏拖拽区映射到 window_chrome 公开包装函数。
        "WindowDragRegion" => generate_window_drag_region(element),
        // 浮动按钮映射到现有 FloatButton Widget。
        "FloatButton" => generate_float_button(element),
        // 浮动按钮组映射到保留直接子按钮事件的 View 组合入口。
        "FloatButtonGroup" => generate_float_button_group(element),
        // 文档内置组件按登记类别返回规划中诊断。
        _ if planned_builtin_diagnostic(element).is_some() => {
            // 前置条件保证诊断存在。
            Err(planned_builtin_diagnostic(element).expect("已确认规划中内置组件已登记"))
        }
        // 真正未知元素仍返回普通映射诊断。
        _ => Err(Diagnostic::new(
            // 指向未登记元素。
            element.span,
            // 说明没有静默猜测映射。
            format!("元素 <{}> 尚无已登记的 Rust API 映射", element.name),
            // 指向明确支持路径。
            format!(
                "使用 {}；Col 仅作为 Row/Grid 的直接子项，或先登记组件状态",
                supported_builtin_hint()
            ),
        )),
    }
    // 结束元素分派函数。
}

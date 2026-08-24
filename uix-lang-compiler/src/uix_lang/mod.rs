// 定义转换器内部共享的带跨度语法树。
mod ast;
// 定义独立于普通表达式语法的同步 action 语句 AST。
mod action_ast;
// 定义同步 action 声明与 do 语句块解析。
mod action_parser;
// 定义同步 action 词法作用域、调用图与递归语义验证。
mod action_semantic;
// 定义已降低 action 标签块到纯 Rust 令牌的生成。
mod action_codegen;
// 定义静态 animation 简写到持久化 Animated 状态的降低。
mod animation_lower;
// 定义静态 animation 装饰的关键帧代码生成。
mod animation_codegen;
// 集中验证 animation 完整简写、字段矩阵与拒绝路径。
#[cfg(test)]
mod animation_codegen_tests;
// 定义 transition 最终目标比较装饰的代码生成。
mod transition_codegen;
// 集中验证 transition 状态目标、动态分支与拒绝路径。
#[cfg(test)]
mod transition_codegen_tests;
// 定义 transition 简写、字段矩阵与状态分支的编译期降低。
mod transition_lower;
// 定义六类文档内置组件的已登记与规划中矩阵。
mod builtin_matrix;
// 定义语言面结构化数据类型到公开构造 API 的编译期映射。
mod data_binding_codegen;
// 定义语言面数据构造器到公开 Rust API 的参数级生成规则。
mod data_constructor_codegen;
// 定义组件内 setStyle 的闭合分支代码生成。
mod dynamic_style_codegen;
// 定义状态伪类的自动事实选择与差异叠加代码生成。
mod pseudo_style_codegen;
// 定义 For 实际实例路径的内部标识符读取。
mod for_identity_codegen;
// 定义最终 ViewNode 的组件状态装饰应用。
mod view_decoration_codegen;
// 定义 record 声明到模块级结构体的生成与语言类型映射。
mod record_codegen;
// 定义 UIX 单源静态视觉记录到模块级 Rust 常量的生成。
mod visual_codegen;
// 定义 Record 与 Visual 模块级项目标的统一生成入口。
mod item_codegen;
// 集中验证 record 生成、对象初始值与拒绝路径。
#[cfg(test)]
mod record_codegen_tests;
// 集中验证 Visual 具名静态项生成、拒绝路径与来源定位。
#[cfg(test)]
mod visual_codegen_tests;
// 集中验证数据构造生成、枚举映射与拒绝路径。
#[cfg(test)]
mod data_binding_codegen_tests;
// 定义 ButtonGroup 内置组件的公开 API 代码生成边界。
mod button_group_codegen;
// 定义核心元素、属性、事件与控制流的 Rust View 代码生成。
mod codegen;
// 定义动态 Text 插值的拥有型捕获与延迟求值生成。
mod dynamic_text_codegen;
// 集中验证动态文本不会建立无条件根结构依赖。
#[cfg(test)]
mod dynamic_text_codegen_tests;
// 定义相邻 If、ElseIf 与 Else 的配对和短路条件链生成。
mod conditional_chain_codegen;
// 集中验证条件链相邻配对、短路生成与拒绝路径。
#[cfg(test)]
mod conditional_chain_codegen_tests;
// 定义元素名称到既有专用生成器的确定性分派。
mod element_codegen;
// 定义框架 Rust 基础 View 注入 UIX 组合树的窄边界。
mod kernel_view_codegen;
// 集中验证基础 View 桥接的所有权与拒绝路径。
#[cfg(test)]
mod kernel_view_codegen_tests;
// 定义 Divider 内置组件的公开 API 代码生成边界。
mod divider_codegen;
// 定义 Space 内置组件的公开 API 代码生成边界。
mod space_codegen;
// 定义 Typography 内置组件的公开 API 代码生成边界。
mod typography_codegen;
// 定义 ThemeToggle 内置组件的公开 API 代码生成边界。
mod theme_toggle_codegen;
// 定义完整设计 token 白名单、类型解析与样式引用生成边界。
mod theme_token_codegen;
// 定义 WindowControl 内置组件的公开 API 代码生成边界。
mod window_control_codegen;
// 定义 WindowDragRegion 内置组件的公开 API 代码生成边界。
mod window_drag_region_codegen;
// 集中验证 WindowDragRegion 生成、形状与交互所有权诊断。
#[cfg(test)]
mod window_drag_region_codegen_tests;
// 集中验证代码生成快照、消费者编译与拒绝路径。
#[cfg(test)]
mod codegen_tests;
// 集中验证组件参考文档中的 UIX 示例持续通过完整代码生成。
#[cfg(test)]
mod reference_examples_tests;
// 集中验证通用 Button 与 Label 的文档属性生成和拒绝路径。
#[cfg(test)]
mod general_widget_codegen_tests;
// 定义 Widget、props 与 state 的结构化 AST。
mod widget_ast;
// 定义 Widget 声明头与 external 白名单解析。
mod widget_declaration_parser;
// 定义 Widget props 与私有状态的类型化 Rust 绑定。
mod widget_binding_codegen;
// 定义 Option、集合与嵌套 record 的递归类型化初始值生成。
mod widget_typed_value_codegen;
// 集中验证扩展集合、Some 与嵌套 record 初始值。
#[cfg(test)]
mod widget_typed_value_tests;
// 定义 Widget 有序 computed 派生值的静态绑定。
mod widget_computed_codegen;
// 定义完整文档中的组件调用展开。
mod widget_codegen;
// 定义 Widget Slot 调用方投影与模板内联。
mod widget_slot_codegen;
// 定义 Widget 模板 Slot 声明验证。
mod widget_slot_parser;
// 集中验证组件展开、状态与拒绝路径。
#[cfg(test)]
mod widget_codegen_tests;
// 集中验证 For 内 Widget 的逐实例身份与插槽作用域。
#[cfg(test)]
mod widget_for_codegen_tests;
// 集中验证默认与具名 Slot 的展开和诊断。
#[cfg(test)]
mod widget_slot_tests;
// 定义组件字段表达式改写与 setState 降低。
mod widget_expression_lower;
// 定义组件内 setStyle 的作用域校验与表达式降低。
mod dynamic_style_lower;
// 定义状态伪类到既有 hover、disabled 与 checked 事实的降低。
mod pseudo_style_lower;
// 定义 Widget 声明级语法与类型白名单解析。
mod widget_parser;
// 定义顶层 Visual 具名静态字段解析与 Rust 字段映射。
mod visual_parser;
// 定义 Widget prop 类型后缀与受限默认表达式解析。
mod widget_prop_default_parser;
// 定义组件调用属性完整性、唯一性与必填校验。
mod widget_call_validator;
// 集中验证 Widget props、state 与名称诊断。
#[cfg(test)]
mod widget_tests;
// 集中验证同步 action 声明、静态展开与拒绝路径。
#[cfg(test)]
mod widget_action_tests;
// 集中验证 Widget prop 默认值、覆盖与必填诊断。
#[cfg(test)]
mod widget_prop_default_tests;
// 定义包含位置、原因与修复建议的解析诊断。
mod diagnostic;
// 定义顶层指令、样式类与主题解析。
mod declaration_parser;
// 定义受限表达式的确定性 AST。
mod expression_ast;
// 定义受限表达式到 Rust 令牌的确定性转换。
mod expression_codegen;
// 定义按事件类型登记的 $event 字段校验与处理器生成边界。
mod event_payload_codegen;
// 定义通用点击、指针与键盘事件到公开 View API 的生成边界。
mod event_codegen;
// 集中验证事件载荷登记、字段投影与未知字段诊断。
#[cfg(test)]
mod event_codegen_tests;
// 定义受限表达式的词法事实层。
mod expression_lexer;
// 定义受限表达式优先级解析与语义验证。
mod expression_parser;
// 定义 Affix 单子树与滚动状态映射的公开 API 代码生成边界。
mod affix_codegen;
// 集中验证 Affix 生成、状态绑定与拒绝路径。
#[cfg(test)]
mod affix_codegen_tests;
// 定义 BackTop 状态回写与叶组件映射的公开 API 代码生成边界。
mod back_top_codegen;
// 集中验证 BackTop 生成与拒绝路径。
#[cfg(test)]
mod back_top_codegen_tests;
// 定义 Splitter 双面板与初始比例映射的公开 API 代码生成边界。
mod splitter_codegen;
// 集中验证 Splitter 生成、双面板形状与拒绝路径。
#[cfg(test)]
mod splitter_codegen_tests;
// 定义应用布局壳五类标签的公开 API 代码生成边界。
mod app_layout_codegen;
// 定义 <App> 根文档到现有 App builder 的编译期组装边界。
mod app_codegen;
// 集中验证应用入口属性、主题与拒绝路径。
#[cfg(test)]
mod app_codegen_tests;
// 集中验证布局壳嵌套、布尔简写与拒绝路径。
#[cfg(test)]
mod app_layout_codegen_tests;
// 定义 Input 文本绑定、类型与变更事件的公开 API 代码生成边界。
mod input_codegen;
// 集中验证 Input 生成与拒绝路径。
#[cfg(test)]
mod input_codegen_tests;
// 定义 InputNumber 数值绑定、范围、步长与精度的公开 API 代码生成边界。
mod input_number_codegen;
// 集中验证 InputNumber 生成与拒绝路径。
#[cfg(test)]
mod input_number_codegen_tests;
// 定义 InputGroup 附加文本与状态绑定的公开 API 代码生成边界。
mod input_group_codegen;
// 集中验证 InputGroup 生成与拒绝路径。
#[cfg(test)]
mod input_group_codegen_tests;
// 定义 Slider 范围、步长与状态绑定的公开 API 代码生成边界。
mod slider_codegen;
// 集中验证 Slider 生成与拒绝路径。
#[cfg(test)]
mod slider_codegen_tests;
// 定义 RangeSlider 范围、步长与双状态绑定的公开 API 代码生成边界。
mod range_slider_codegen;
// 集中验证 RangeSlider 生成与拒绝路径。
#[cfg(test)]
mod range_slider_codegen_tests;
// 定义 Rate 星数、半星与状态绑定的公开 API 代码生成边界。
mod rate_codegen;
// 集中验证 Rate 生成与拒绝路径。
#[cfg(test)]
mod rate_codegen_tests;
// 定义 Checkbox 标签、禁用与双向勾选的公开 API 代码生成边界。
mod checkbox_codegen;
// 集中验证 Checkbox 生成与拒绝路径。
#[cfg(test)]
mod checkbox_codegen_tests;
// 定义 Switch 禁用与双向勾选的公开 API 代码生成边界。
mod switch_codegen;
// 集中验证 Switch 生成与拒绝路径。
#[cfg(test)]
mod switch_codegen_tests;
// 定义 Radio 选项与双向值绑定的公开 API 代码生成边界。
mod radio_codegen;
// 集中验证 Radio 生成与拒绝路径。
#[cfg(test)]
mod radio_codegen_tests;
// 定义 Segmented 选项与双向值绑定的公开 API 代码生成边界。
mod segmented_codegen;
// 集中验证 Segmented 生成与拒绝路径。
#[cfg(test)]
mod segmented_codegen_tests;
// 定义 Select 结构化选项与模式绑定的公开 API 代码生成边界。
mod select_codegen;
// 集中验证 Select 生成与拒绝路径。
#[cfg(test)]
mod select_codegen_tests;
// 定义 Cascader 选项树与路径绑定的公开 API 代码生成边界。
mod cascader_codegen;
// 集中验证 Cascader 生成与拒绝路径。
#[cfg(test)]
mod cascader_codegen_tests;
// 定义 TreeSelect 节点树与稳定 key 绑定的公开 API 代码生成边界。
mod tree_select_codegen;
// 集中验证 TreeSelect 生成与拒绝路径。
#[cfg(test)]
mod tree_select_codegen_tests;
// 定义 AutoComplete 候选与文本绑定的公开 API 代码生成边界。
mod autocomplete_codegen;
// 集中验证 AutoComplete 生成与拒绝路径。
#[cfg(test)]
mod autocomplete_codegen_tests;
// 定义 Mentions 候选与完整文本绑定的公开 API 代码生成边界。
mod mentions_codegen;
// 集中验证 Mentions 生成与拒绝路径。
#[cfg(test)]
mod mentions_codegen_tests;
// 定义 DatePicker 日期状态与选择粒度的公开 API 代码生成边界。
mod date_picker_codegen;
// 集中验证 DatePicker 生成与拒绝路径。
#[cfg(test)]
mod date_picker_codegen_tests;
// 定义 DateRangePicker 双日期状态对象的公开 API 代码生成边界。
mod date_range_picker_codegen;
// 集中验证 DateRangePicker 生成与拒绝路径。
#[cfg(test)]
mod date_range_picker_codegen_tests;
// 定义 TimePicker 时间状态的公开 API 代码生成边界。
mod time_picker_codegen;
// 集中验证 TimePicker 生成与拒绝路径。
#[cfg(test)]
mod time_picker_codegen_tests;
// 定义 ColorPicker 颜色状态的公开 API 代码生成边界。
mod color_picker_codegen;
// 集中验证 ColorPicker 生成与拒绝路径。
#[cfg(test)]
mod color_picker_codegen_tests;
// 定义 Avatar 来源、回退文字、形状与尺寸的公开 API 代码生成边界。
mod avatar_codegen;
// 集中验证 Avatar 生成、默认值与拒绝路径。
#[cfg(test)]
mod avatar_codegen_tests;
// 定义 Badge 零或单真实子 View 与公开属性的代码生成边界。
mod badge_codegen;
// 集中验证 Badge 生成、组合形状与拒绝路径。
#[cfg(test)]
mod badge_codegen_tests;
// 定义 Image 来源、固有尺寸与运行时配置的公开 API 代码生成边界。
mod image_codegen;
// 集中验证 Image 生成、默认值与拒绝路径。
#[cfg(test)]
mod image_codegen_tests;
// 定义 ImageGroup 图片集合、初始索引与变化观察器的公开 API 代码生成边界。
mod image_group_codegen;
// 集中验证 ImageGroup 生成、默认值与拒绝路径。
#[cfg(test)]
mod image_group_codegen_tests;
// 定义 List 文本集合与字符串槽位的公开 API 代码生成边界。
mod list_codegen;
// 集中验证 List 生成、默认值与拒绝路径。
#[cfg(test)]
mod list_codegen_tests;
// 定义 SelectableList 类型化条目、受控稳定 id 与 Change 事件生成边界。
mod selectable_list_codegen;
// 集中验证 SelectableList 生成、默认值与拒绝路径。
#[cfg(test)]
mod selectable_list_codegen_tests;
// 定义 Collapse 稳定面板 key、受控展开集合与 Change 事件生成边界。
mod collapse_codegen;
// 集中验证 Collapse 生成、默认值与拒绝路径。
#[cfg(test)]
mod collapse_codegen_tests;
// 定义 Upload 受控队列、扩展名门禁与类型化 Change 事件生成边界。
mod upload_codegen;
// 集中验证 Upload 生成、默认值与拒绝路径。
#[cfg(test)]
mod upload_codegen_tests;
// 定义 Message / Notification keyed 声明租约生成边界。
mod feedback_declaration_codegen;
// 集中验证反馈声明生成、默认值与拒绝路径。
#[cfg(test)]
mod feedback_declaration_codegen_tests;
// 定义 Skeleton 静态外观与尺寸的公开 API 代码生成边界。
mod skeleton_codegen;
// 集中验证 Skeleton 生成与拒绝路径。
#[cfg(test)]
mod skeleton_codegen_tests;
// 定义 Empty 描述与图标的公开 API 代码生成边界。
mod empty_codegen;
// 集中验证 Empty 生成与拒绝路径。
#[cfg(test)]
mod empty_codegen_tests;
// 定义 ResultView 类型与文本的公开 API 代码生成边界。
mod result_view_codegen;
// 集中验证 ResultView 生成与拒绝路径。
#[cfg(test)]
mod result_view_codegen_tests;
// 定义 Tag 文本、颜色与初始交互能力的公开 API 代码生成边界。
mod tag_codegen;
// 集中验证 Tag 生成与拒绝路径。
#[cfg(test)]
mod tag_codegen_tests;
// 定义 Card 标题、操作项与完整子树的公开 API 代码生成边界。
mod card_codegen;
// 集中验证 Card 生成、控制子树与拒绝路径。
#[cfg(test)]
mod card_codegen_tests;
// 定义 Descriptions 类型化数据与列数的公开 API 代码生成边界。
mod descriptions_codegen;
// 集中验证 Descriptions 生成、默认值与拒绝路径。
#[cfg(test)]
mod descriptions_codegen_tests;
// 定义 Timeline 类型化数据、pending 与 reverse 的公开 API 代码生成边界。
mod timeline_codegen;
// 集中验证 Timeline 生成、默认值与拒绝路径。
#[cfg(test)]
mod timeline_codegen_tests;
// 定义 Calendar 默认交互组件的公开 API 代码生成边界。
mod calendar_codegen;
// 集中验证 Calendar 生成与拒绝路径。
#[cfg(test)]
mod calendar_codegen_tests;
// 定义 Carousel 有序幻灯片与自动播放的公开 API 代码生成边界。
mod carousel_codegen;
// 集中验证 Carousel 生成、默认值、控制流与拒绝路径。
#[cfg(test)]
mod carousel_codegen_tests;
// 定义 Tree 类型化节点与初始交互配置的公开 API 代码生成边界。
mod tree_codegen;
// 集中验证 Tree 生成、默认值与拒绝路径。
#[cfg(test)]
mod tree_codegen_tests;
// 定义 Table 类型化行列与运行配置的公开 API 代码生成边界。
mod table_codegen;
// 集中验证 Table 生成与拒绝路径。
#[cfg(test)]
mod table_codegen_tests;
// 定义 Tabs 静态面板与受控 key 的公开 API 代码生成边界。
mod tabs_codegen;
// 集中验证 Tabs 生成与拒绝路径。
#[cfg(test)]
mod tabs_codegen_tests;
// 定义 Menu typed 数据与双受控状态的公开 API 代码生成边界。
mod menu_codegen;
// 集中验证 Menu 生成与拒绝路径。
#[cfg(test)]
mod menu_codegen_tests;
// 定义 Dropdown keyed 数据、组合 trigger 与 Change 的公开 API 代码生成边界。
mod dropdown_codegen;
// 集中验证 Dropdown 生成与拒绝路径。
#[cfg(test)]
mod dropdown_codegen_tests;
// 定义 Navigation 受控侧栏组合与选择事件的公开 API 代码生成边界。
mod navigation_codegen;
// 集中验证 Navigation 生成与拒绝路径。
#[cfg(test)]
mod navigation_codegen_tests;
// 定义 Steps 类型化步骤、受控 current 与方向的公开 API 代码生成边界。
mod steps_codegen;
// 集中验证 Steps 生成、状态绑定与拒绝路径。
#[cfg(test)]
mod steps_codegen_tests;
// 定义 Pagination 总条数、双状态与 Change 事件的公开 API 代码生成边界。
mod pagination_codegen;
// 集中验证 Pagination 生成、默认值、状态绑定与拒绝路径。
#[cfg(test)]
mod pagination_codegen_tests;
// 定义 Breadcrumb 类型化路径数据与末项当前页的公开 API 代码生成边界。
mod breadcrumb_codegen;
// 集中验证 Breadcrumb 生成与拒绝路径。
#[cfg(test)]
mod breadcrumb_codegen_tests;
// 定义 Anchor 类型化滚动目标、偏移与 href 事件的公开 API 代码生成边界。
mod anchor_codegen;
// 集中验证 Anchor 生成与拒绝路径。
#[cfg(test)]
mod anchor_codegen_tests;
// 定义 QRCode 内容、尺寸与纠错等级的公开 API 代码生成边界。
mod qrcode_codegen;
// 集中验证 QRCode 生成、默认值与拒绝路径。
#[cfg(test)]
mod qrcode_codegen_tests;
// 定义 Watermark 文字与透明度的公开 API 代码生成边界。
mod watermark_codegen;
// 集中验证 Watermark 生成、默认值与拒绝路径。
#[cfg(test)]
mod watermark_codegen_tests;
// 定义 RichText 内容解析与选择配置的公开 API 代码生成边界。
mod rich_text_codegen;
// 集中验证 RichText 生成、默认值与拒绝路径。
#[cfg(test)]
mod rich_text_codegen_tests;
// 定义 Alert 状态、关闭能力与关闭事件的公开 API 代码生成边界。
mod alert_codegen;
// 集中验证 Alert 生成、默认值、事件与拒绝路径。
#[cfg(test)]
mod alert_codegen_tests;
// 定义 ProgressBar fraction、模式与形态的公开 API 代码生成边界。
mod progress_codegen;
// 集中验证 ProgressBar 生成、默认值与拒绝路径。
#[cfg(test)]
mod progress_codegen_tests;
// 定义 BarChart、LineChart 与 PieChart 的基础静态生成边界。
mod basic_chart_codegen;
// 集中映射十二类图表共享的标题、图例、动画与交互配置。
mod chart_common_codegen;
// 集中验证三类基础图表生成、数据类型与拒绝路径。
#[cfg(test)]
mod basic_chart_codegen_tests;
// 定义 AreaChart、ScatterChart 与 FunnelChart 的静态生成边界。
mod static_chart_codegen;
// 集中验证三类静态高级图表生成、枚举与拒绝路径。
#[cfg(test)]
mod static_chart_codegen_tests;
// 定义 Treemap 与 Gauge 的层级数据和单值静态生成边界。
mod hierarchy_gauge_chart_codegen;
// 集中验证矩形树图与仪表盘生成、数据构造和拒绝路径。
#[cfg(test)]
mod hierarchy_gauge_chart_codegen_tests;
// 定义 Heatmap 与 WaterfallChart 的混合数据静态生成边界。
mod matrix_delta_chart_codegen;
// 集中验证热力图与瀑布图生成、混合数据和拒绝路径。
#[cfg(test)]
mod matrix_delta_chart_codegen_tests;
// 定义 RadarChart 与 ComboChart 的精确泛型系列生成边界。
mod series_chart_codegen;
// 集中验证雷达图与组合图生成、泛型集合和拒绝路径。
#[cfg(test)]
mod series_chart_codegen_tests;
// 定义 Popconfirm 唯一触发子树、六向位置与同步回调的公开 API 代码生成边界。
mod popconfirm_codegen;
// 集中验证 Popconfirm 生成、组合 trigger、回调与拒绝路径。
#[cfg(test)]
mod popconfirm_codegen_tests;
// 定义 Modal 受控状态、操作回调与有序内容子树的公开 API 代码生成边界。
mod modal_codegen;
// 集中验证 Modal 生成、默认值、事件与拒绝路径。
#[cfg(test)]
mod modal_codegen_tests;
// 定义 Drawer 受控状态、方向、宽度与有序内容子树的公开 API 代码生成边界。
mod drawer_codegen;
// 集中验证 Drawer 生成、默认值与拒绝路径。
#[cfg(test)]
mod drawer_codegen_tests;
// 定义 Tooltip 文字与唯一静态触发 View 的公开 API 代码生成边界。
mod tooltip_codegen;
// 集中验证 Tooltip 生成、触发子树与拒绝路径。
#[cfg(test)]
mod tooltip_codegen_tests;
// 定义 Popover 内容、触发方式与唯一静态触发 View 的公开 API 代码生成边界。
mod popover_codegen;
// 集中验证 Popover 生成、触发子树与拒绝路径。
#[cfg(test)]
mod popover_codegen_tests;
// 定义 FocusTrap 有序焦点作用域子树的公开 API 代码生成边界。
mod focus_trap_codegen;
// 集中验证 FocusTrap 生成、控制流与拒绝路径。
#[cfg(test)]
mod focus_trap_codegen_tests;
// 定义 Spin 加载状态、提示文字与有序遮罩子树的公开 API 代码生成边界。
mod spin_codegen;
// 集中验证 Spin 生成、默认值、控制流与拒绝路径。
#[cfg(test)]
mod spin_codegen_tests;
// 定义 Form 与类型化字段的公开 API 代码生成边界。
mod form_codegen;
// 定义 FormSliderItem 类型化 f64 字段的独立生成边界。
mod form_slider_codegen;
// 集中验证 Form 生成与拒绝路径。
#[cfg(test)]
mod form_codegen_tests;
// 定义 FloatButton 内置组件的公开 API 代码生成边界。
mod float_button_codegen;
// 定义 FloatButtonGroup 静态子按钮与触发方式的代码生成边界。
mod float_button_group_codegen;
// 集中验证 FloatButtonGroup 生成、事件保留与直接子项诊断。
#[cfg(test)]
mod float_button_group_codegen_tests;
// 定义布局容器、Row/Col 栅格与 Grid/Col 的公开 API 代码生成边界。
mod layout_codegen;
// 定义 Container 专属视觉与布局简写的独立生成边界。
mod container_style_codegen;
// 集中验证布局标签生成、父子形状与拒绝路径。
#[cfg(test)]
mod layout_codegen_tests;
// 定义 VirtualScroll 数据快照与惰性行模板的公开 API 代码生成边界。
mod virtual_scroll_codegen;
// 集中验证 VirtualScroll 生成、身份与拒绝路径。
#[cfg(test)]
mod virtual_scroll_codegen_tests;
// 集中验证表达式与控制绑定 Gate。
#[cfg(test)]
mod expression_tests;
// 定义面向 UTF-8 源码的词法游标。
mod lexer;
// 定义唯一根元素与核心节点的递归下降解析器。
mod parser;
// 定义顶层声明和样式值 AST。
mod style_ast;
// 定义已映射内联样式到公开 Style 字段的编译期转换。
mod style_codegen;
// 定义三个背景图层属性到 UI 运行时契约的独立映射。
mod style_background_codegen;
// 集中验证背景来源、定位、重复与拒绝路径。
#[cfg(test)]
mod style_background_codegen_tests;
// 定义 borderStyle 关键字到 UI 边框线型契约的独立映射。
mod style_border_codegen;
// 集中验证 borderStyle 的完整枚举生成与拒绝路径。
#[cfg(test)]
mod style_border_codegen_tests;
// 定义 fontFamily 有序列表到 UI 字体族契约的独立映射。
mod style_font_family_codegen;
// 定义 float/clear 非目标布局决策的专用编译诊断。
mod style_float_codegen;
// 集中验证规范值、非法值与可执行替代建议。
#[cfg(test)]
mod style_float_codegen_tests;
// 集中验证 fontFamily 的列表生成与拒绝路径。
#[cfg(test)]
mod style_font_family_codegen_tests;
// 定义 fontWeight 关键字与整数到 UI 字体粗细契约的独立映射。
mod style_font_weight_codegen;
// 集中验证 fontWeight 的完整生成与拒绝路径。
#[cfg(test)]
mod style_font_weight_codegen_tests;
// 定义 lineHeight 倍率与像素值到 UI 行高契约的独立映射。
mod style_line_height_codegen;
// 集中验证 lineHeight 的单位生成与拒绝路径。
#[cfg(test)]
mod style_line_height_codegen_tests;
// 定义 position 与四边值到 UI 定位契约的独立映射。
mod style_position_codegen;
// 集中验证五模式、四边像素与 auto 的生成和拒绝路径。
#[cfg(test)]
mod style_position_codegen_tests;
// 定义 textAlign 关键字到 UI 文本水平对齐契约的独立映射。
mod style_text_align_codegen;
// 集中验证 textAlign 的完整枚举生成与拒绝路径。
#[cfg(test)]
mod style_text_align_codegen_tests;
// 定义 textDecoration 关键字到 UI 文本装饰契约的独立映射。
mod style_text_decoration_codegen;
// 集中验证 textDecoration 的完整枚举生成与拒绝路径。
#[cfg(test)]
mod style_text_decoration_codegen_tests;
// 定义样式类继承、引用与内联优先级改写。
mod style_class_resolver;
// 定义样式值、颜色、边距与枚举的编译期映射。
mod style_value_codegen;
// 定义完整盒阴影值与字段更新的编译期映射。
mod style_shadow_codegen;
// 集中验证盒阴影旧语法与可选 spread 的生成契约。
#[cfg(test)]
mod style_shadow_codegen_tests;
// 定义 transform 函数列表到公开二维仿射矩阵的编译期映射。
mod style_transform_codegen;
// 定义 cursor 文档值到公开平台无关枚举的独立映射。
mod style_cursor_codegen;
// 集中验证 transform 与 transformOrigin 的生成和拒绝路径。
#[cfg(test)]
mod style_transform_codegen_tests;
// 集中验证 cursor 的公开枚举生成与拒绝路径。
#[cfg(test)]
mod style_cursor_codegen_tests;
// 定义 userSelect 关键字到 UI 文字选择策略的独立映射。
mod style_user_select_codegen;
// 集中验证 userSelect 的公开枚举生成与拒绝路径。
#[cfg(test)]
mod style_user_select_codegen_tests;
// 定义样式块与内联样式共享解析器。
mod style_parser;
// 集中验证顶层声明与样式 Gate。
#[cfg(test)]
mod style_tests;
// 定义公共属性值与布局枚举的 Rust 映射。
mod value_codegen;

// 向后续转换 Gate 暴露核心语法树类型。
pub(crate) use ast::*;
// 向核心 View 生成器暴露规划中内置组件诊断与完整支持标签提示。
pub(crate) use builtin_matrix::{planned_builtin_diagnostic, supported_builtin_hint};
// 向核心元素生成器暴露 ButtonGroup 专用映射。
pub(crate) use button_group_codegen::generate_button_group;
// 向过程宏入口暴露核心 View 生成函数。
pub(crate) use codegen::generate_view;
// 向核心元素生成器暴露基础 View 桥接映射。
pub(crate) use kernel_view_codegen::{
    generate_kernel_children, generate_kernel_host, generate_kernel_view,
};
// 向核心元素生成器暴露 Divider 专用映射。
pub(crate) use divider_codegen::generate_divider;
// 向核心元素生成器暴露 Space 专用映射。
pub(crate) use space_codegen::generate_space;
// 向核心元素生成器暴露 Typography 专用映射。
pub(crate) use typography_codegen::generate_typography;
// 向核心元素生成器暴露 ThemeToggle 专用映射。
pub(crate) use theme_toggle_codegen::generate_theme_toggle;
// 向核心元素生成器暴露 WindowControl 专用映射。
pub(crate) use window_control_codegen::generate_window_control;
// 向核心元素生成器暴露 WindowDragRegion 专用映射。
pub(crate) use window_drag_region_codegen::generate_window_drag_region;
// 向组件解析与代码生成暴露结构化组件声明。
pub(crate) use widget_ast::*;
// 向 action 解析、语义与降低阶段暴露独立语句 AST。
pub(crate) use action_ast::*;
// 导出语言面数据类型到公开构造 API 的映射查询。
pub(crate) use data_binding_codegen::{
    DataConstructorSpec, data_chain_root, data_constructor_spec, is_data_constructor_chain,
    is_registered_data_type, normalize_number_literals, step_status_path,
};
// 向过程宏入口暴露组件感知文档生成函数。
pub(crate) use widget_codegen::generate_document_view;
// 向过程宏入口暴露 App builder 生成函数。
pub(crate) use app_codegen::generate_document_app;
// 向后续转换 Gate 暴露稳定诊断类型。
pub(crate) use diagnostic::*;
// 向文档解析器暴露 Widget 声明验证入口。
pub(crate) use widget_declaration_parser::parse_widget_declaration;
// 向文档解析器暴露 Record 声明验证入口。
pub(crate) use widget_parser::parse_record_declaration;
// 向文档解析器暴露 Visual 声明验证入口。
pub(crate) use visual_parser::parse_visual_declaration;
// 向 uix_items! 暴露 Record 与 Visual 的统一模块级生成入口。
pub(crate) use item_codegen::generate_document_items;
// 向组件绑定暴露 record 类型映射入口。
pub(crate) use record_codegen::value_type_tokens;
// 仅向同 crate 测试暴露细分生成器，生产入口统一使用 generate_document_items。
#[cfg(test)]
pub(crate) use record_codegen::generate_record_items;
// 向文档解析器暴露顶层声明入口。
pub(crate) use declaration_parser::{
    parse_at_declaration, parse_style_class, register_declaration_name, starts_record_declaration,
    starts_visual_declaration, starts_widget_declaration,
};
// 向核心解析器暴露表达式 AST。
pub(crate) use expression_ast::*;
// 向 View 生成器暴露表达式与事件处理器生成入口。
pub(crate) use expression_codegen::{
    expression_uses_event, generate_expression, generate_expression_without_source_marker,
    generate_handler_expression, mark_source_tokens, with_record_source_marker,
    with_source_marker_id, with_source_markers, with_visual_source_marker,
    with_widget_source_marker,
};

// 编译器内部用该属性把导入后的元素绑定到稳定 SourceId，写出前必须消费。
pub(crate) const SOURCE_ID_ATTRIBUTE: &str = "__uix_source_id";
// 向各事件适配器暴露带字段登记校验的处理器生成入口。
pub(crate) use event_payload_codegen::generate_event_handler_expression;
// 向表达式解析器暴露词法标记。
pub(crate) use expression_lexer::*;
// 向核心解析器暴露受限表达式入口。
pub(crate) use expression_parser::parse_expression;
// 向核心元素生成器暴露 Affix 专用映射。
pub(crate) use affix_codegen::generate_affix;
// 向核心元素生成器暴露 BackTop 专用映射。
pub(crate) use back_top_codegen::generate_back_top;
// 向核心元素生成器暴露 Splitter 专用映射。
pub(crate) use splitter_codegen::generate_splitter;
// 向核心元素生成器暴露应用布局壳专用映射。
pub(crate) use app_layout_codegen::generate_app_layout;
// 向核心元素生成器暴露 Input 专用映射。
pub(crate) use input_codegen::generate_input;
// 向核心元素生成器暴露 InputNumber 专用映射。
pub(crate) use input_number_codegen::generate_input_number;
// 向核心元素生成器暴露 InputGroup 专用映射。
pub(crate) use input_group_codegen::generate_input_group;
// 向核心元素生成器暴露 Slider 专用映射。
pub(crate) use slider_codegen::generate_slider;
// 向核心元素生成器暴露 RangeSlider 专用映射。
pub(crate) use range_slider_codegen::generate_range_slider;
// 向核心元素生成器暴露 Rate 专用映射。
pub(crate) use rate_codegen::generate_rate;
// 向核心元素生成器暴露 Checkbox 专用映射。
pub(crate) use checkbox_codegen::generate_checkbox;
// 向核心元素生成器暴露 Switch 专用映射。
pub(crate) use switch_codegen::generate_switch;
// 向核心元素生成器暴露 Radio 专用映射。
pub(crate) use radio_codegen::generate_radio;
// 向核心元素生成器暴露 Segmented 专用映射。
pub(crate) use segmented_codegen::generate_segmented;
// 向核心元素生成器暴露 Select 专用映射。
pub(crate) use select_codegen::generate_select;
// 向核心元素生成器暴露 Cascader 专用映射。
pub(crate) use cascader_codegen::generate_cascader;
// 向核心元素生成器暴露 TreeSelect 专用映射。
pub(crate) use tree_select_codegen::generate_tree_select;
// 向核心元素生成器暴露 AutoComplete 专用映射。
pub(crate) use autocomplete_codegen::generate_autocomplete;
// 向核心元素生成器暴露 Mentions 专用映射。
pub(crate) use mentions_codegen::generate_mentions;
// 向核心元素生成器暴露 DatePicker 专用映射。
pub(crate) use date_picker_codegen::generate_date_picker;
// 向核心元素生成器暴露 DateRangePicker 专用映射。
pub(crate) use date_range_picker_codegen::generate_date_range_picker;
// 向核心元素生成器暴露 TimePicker 专用映射。
pub(crate) use time_picker_codegen::generate_time_picker;
// 向核心元素生成器暴露 ColorPicker 专用映射。
pub(crate) use color_picker_codegen::generate_color_picker;
// 向核心元素生成器暴露 Avatar 叶组件专用映射。
pub(crate) use avatar_codegen::generate_avatar;
// 向核心元素生成器暴露 Badge 组合装饰器专用映射。
pub(crate) use badge_codegen::generate_badge;
// 向核心元素生成器暴露 Image 叶组件专用映射。
pub(crate) use image_codegen::generate_image;
// 向核心元素生成器暴露 ImageGroup 叶组件专用映射。
pub(crate) use image_group_codegen::generate_image_group;
// 向核心元素生成器暴露 List 数据与命名节点插槽专用映射。
pub(crate) use list_codegen::generate_list;
// 向核心元素生成器暴露 SelectableList 叶组件专用映射。
pub(crate) use selectable_list_codegen::generate_selectable_list;
// 向核心元素生成器暴露 Collapse 叶组件专用映射。
pub(crate) use collapse_codegen::generate_collapse;
// 向核心元素生成器暴露 Upload 叶组件专用映射。
pub(crate) use upload_codegen::generate_upload;
// 向核心元素生成器暴露两类反馈声明专用映射。
pub(crate) use feedback_declaration_codegen::{
    generate_message_declaration, generate_notification_declaration,
};
// 向核心元素生成器暴露 Skeleton 专用映射。
pub(crate) use skeleton_codegen::generate_skeleton;
// 向核心元素生成器暴露 Empty 专用映射。
pub(crate) use empty_codegen::generate_empty;
// 向核心元素生成器暴露 ResultView 专用映射。
pub(crate) use result_view_codegen::generate_result_view;
// 向核心元素生成器暴露 Tag 专用映射。
pub(crate) use tag_codegen::generate_tag;
// 向核心元素生成器暴露 Card 容器专用映射。
pub(crate) use card_codegen::generate_card;
// 向核心元素生成器暴露 Descriptions 叶组件专用映射。
pub(crate) use descriptions_codegen::generate_descriptions;
// 向核心元素生成器暴露 Timeline 叶组件专用映射。
pub(crate) use timeline_codegen::generate_timeline;
// 向核心元素生成器暴露 Calendar 叶组件专用映射。
pub(crate) use calendar_codegen::generate_calendar;
// 向核心元素生成器暴露 Carousel 容器专用映射。
pub(crate) use carousel_codegen::generate_carousel;
// 向核心元素生成器暴露 Tree 叶组件专用映射。
pub(crate) use tree_codegen::generate_tree;
// 向核心元素生成器暴露 Table 叶组件专用映射。
pub(crate) use table_codegen::generate_table;
// 向核心元素生成器暴露 Tabs 容器专用映射。
pub(crate) use tabs_codegen::generate_tabs;
// 向核心元素生成器暴露 Menu 叶组件专用映射。
pub(crate) use menu_codegen::generate_menu;
// 向核心元素生成器暴露 Dropdown 组合组件专用映射。
pub(crate) use dropdown_codegen::generate_dropdown;
// 向核心元素生成器暴露 Navigation 组合组件专用映射。
pub(crate) use navigation_codegen::generate_navigation;
// 向核心元素生成器暴露 Steps 叶组件专用映射。
pub(crate) use steps_codegen::generate_steps;
// 向核心元素生成器暴露 Pagination 叶组件专用映射。
pub(crate) use pagination_codegen::generate_pagination;
// 向核心元素生成器暴露 Breadcrumb 叶组件专用映射。
pub(crate) use breadcrumb_codegen::generate_breadcrumb;
// 向核心元素生成器暴露 Anchor 叶组件专用映射。
pub(crate) use anchor_codegen::generate_anchor;
// 向核心元素生成器暴露 QRCode 叶组件专用映射。
pub(crate) use qrcode_codegen::generate_qrcode;
// 向核心元素生成器暴露 Watermark 叶组件专用映射。
pub(crate) use watermark_codegen::generate_watermark;
// 向核心元素生成器暴露 RichText 叶组件专用映射。
pub(crate) use rich_text_codegen::generate_rich_text;
// 向核心元素生成器暴露 Alert 叶组件专用映射。
pub(crate) use alert_codegen::generate_alert;
// 向核心元素生成器暴露 ProgressBar 叶组件专用映射。
pub(crate) use progress_codegen::generate_progress_bar;
// 向核心元素生成器暴露三类基础图表叶组件映射。
pub(crate) use basic_chart_codegen::generate_basic_chart;
// 向核心元素生成器暴露三类静态高级图表叶组件映射。
pub(crate) use static_chart_codegen::generate_static_chart;
// 向核心元素生成器暴露矩形树图与仪表盘叶组件映射。
pub(crate) use hierarchy_gauge_chart_codegen::generate_hierarchy_gauge_chart;
// 向核心元素生成器暴露热力图与瀑布图叶组件映射。
pub(crate) use matrix_delta_chart_codegen::generate_matrix_delta_chart;
// 向核心元素生成器暴露雷达图与组合图叶组件映射。
pub(crate) use series_chart_codegen::generate_series_chart;
// 向核心元素生成器暴露 Popconfirm 组合组件专用映射。
pub(crate) use popconfirm_codegen::generate_popconfirm;
// 向核心元素生成器暴露 Modal 容器组件专用映射。
pub(crate) use modal_codegen::generate_modal;
// 向核心元素生成器暴露 Drawer 容器组件专用映射。
pub(crate) use drawer_codegen::generate_drawer;
// 向核心元素生成器暴露 Tooltip 容器组件专用映射。
pub(crate) use tooltip_codegen::generate_tooltip;
// 向核心元素生成器暴露 Popover 容器组件专用映射。
pub(crate) use popover_codegen::generate_popover;
// 向核心元素生成器暴露 FocusTrap 容器组件专用映射。
pub(crate) use focus_trap_codegen::generate_focus_trap;
// 向核心元素生成器暴露 Spin 容器组件专用映射。
pub(crate) use spin_codegen::generate_spin;
// 向核心元素生成器暴露 Form 与越界字段项映射。
pub(crate) use form_codegen::{
    generate_form, generate_orphan_form_checkbox_item, generate_orphan_form_input_item,
    generate_orphan_form_radio_item, generate_orphan_form_select_item,
    generate_orphan_form_switch_item,
};
// 向核心元素生成器暴露孤立滑块字段诊断。
pub(crate) use form_slider_codegen::generate_orphan_form_slider_item;
// 向核心元素生成器暴露 FloatButton 专用映射。
pub(crate) use float_button_codegen::generate_float_button;
// 向核心元素生成器暴露 FloatButtonGroup 专用映射。
pub(crate) use float_button_group_codegen::generate_float_button_group;
// 向核心元素生成器暴露布局组件专用映射。
pub(crate) use layout_codegen::{
    generate_column, generate_container, generate_grid, generate_orphan_col, generate_row,
    generate_scroll_view,
};
// 向核心元素生成器暴露 VirtualScroll 专用映射。
pub(crate) use virtual_scroll_codegen::generate_virtual_scroll;
// 向核心解析器暴露 UTF-8 安全词法游标。
pub(crate) use lexer::Cursor;
// 向过程宏入口暴露文档解析函数。
pub(crate) use parser::{parse_document, parse_items_document};
// 向文档解析器暴露声明与样式 AST。
pub(crate) use style_ast::*;
// 向 View 生成器暴露内联样式映射入口。
pub(crate) use style_codegen::apply_inline_style;
// 向组件展开器暴露样式类解析器。
pub(crate) use style_class_resolver::StyleClassResolver;
// 向文档解析器暴露共享样式入口。
pub(crate) use style_parser::parse_style_properties;
// 向 App 主题生成器暴露共享颜色字面量解析。
pub(crate) use style_value_codegen::parse_color;
// 向 View 生成器暴露属性值共享映射入口。
pub(crate) use value_codegen::{
    align_value, boolean_value, deferred_style_diagnostic, justify_value, literal_string,
    numeric_value, rust_identifier, string_value, typography_value,
};

// 为需要控制流身份的测试提供与公开 uix! 一致的完整文档入口。
#[cfg(test)]
pub(crate) fn generate_test_document_view(source: &str) -> Result<String, Diagnostic> {
    // 先执行完整文档解析与声明校验。
    let document = parse_document(source)?;
    // 再经过组件展开生成稳定令牌文本。
    generate_document_view(&document).map(|tokens| tokens.to_string())
}

// 定义转换器内部共享的带跨度语法树。
mod ast;
// 定义独立于普通表达式语法的同步 action 语句 AST。
mod action_ast;
// 定义同步 action 声明与 do 语句块解析。
mod action_parser;
pub(crate) use action_parser::{ModuleActionBody, parse_module_body};
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
#[path = "../../tests-src/uix_lang/animation_codegen_tests.rs"]
mod animation_codegen_tests;
// 定义 transition 最终目标比较装饰的代码生成。
mod transition_codegen;
// 集中验证 transition 状态目标、动态分支与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/transition_codegen_tests.rs"]
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
// 定义元素属性查找的共享生成辅助。
mod attribute_lookup_codegen;
pub(super) use attribute_lookup_codegen::find_attribute;
pub(crate) use attribute_lookup_codegen::required_attribute;
// 定义 record 声明到模块级结构体的生成与语言类型映射。
mod record_codegen;
// 定义 UIX 单源静态视觉记录到模块级 Rust 常量的生成。
mod visual_codegen;
// 定义 Record 与 Visual 模块级项目标的统一生成入口。
mod item_codegen;
// 集中验证 record 生成、对象初始值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/record_codegen_tests.rs"]
mod record_codegen_tests;
// 集中验证 Visual 具名静态项生成、拒绝路径与来源定位。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/visual_codegen_tests.rs"]
mod visual_codegen_tests;
// 集中验证数据构造生成、枚举映射与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/data_binding_codegen_tests.rs"]
mod data_binding_codegen_tests;
// 定义核心元素、属性、事件与控制流的 Rust View 代码生成。
mod codegen;
// 定义动态 Text 插值的拥有型捕获与延迟求值生成。
mod dynamic_text_codegen;
// 集中验证动态文本不会建立无条件根结构依赖。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/dynamic_text_codegen_tests.rs"]
mod dynamic_text_codegen_tests;
// 定义相邻 If、ElseIf 与 Else 的配对和短路条件链生成。
mod conditional_chain_codegen;
// 集中验证条件链相邻配对、短路生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/conditional_chain_codegen_tests.rs"]
mod conditional_chain_codegen_tests;
// 定义元素名称到既有专用生成器的确定性分派。
mod element_codegen;
// 定义框架 Rust 基础 View 注入 UIX 组合树的窄边界。
mod kernel_view_codegen;
// 集中验证基础 View 桥接的所有权与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/kernel_view_codegen_tests.rs"]
mod kernel_view_codegen_tests;
// 定义 Canvas 绘制组件的公开 API 代码生成边界。
mod canvas_codegen;
// 集中验证 Canvas 组件的专有属性与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/canvas_codegen_tests.rs"]
mod canvas_codegen_tests;
// 集中验证 reactive 组件的作用域包装与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/reactive_codegen_tests.rs"]
mod reactive_codegen_tests;
// 定义完整设计 token 白名单、类型解析与样式引用生成边界。
mod theme_token_codegen;
// 集中验证 WindowDragRegion 生成、形状与交互所有权诊断。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/window_drag_region_codegen_tests.rs"]
mod window_drag_region_codegen_tests;
// 集中验证 WindowControl 生成与图标前景色定制诊断。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/window_control_codegen_tests.rs"]
mod window_control_codegen_tests;
// 集中验证代码生成快照、消费者编译与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/codegen_tests.rs"]
mod codegen_tests;
// 集中验证组件参考文档中的 UIX 示例持续通过完整代码生成。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/reference_examples_tests.rs"]
mod reference_examples_tests;
// 集中验证通用 Button 与 Label 的文档属性生成和拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/general_widget_codegen_tests.rs"]
mod general_widget_codegen_tests;
// 定义 Widget、props 与 state 的结构化 AST。
mod widget_ast;
// 定义 Widget 声明头与 external 白名单解析。
mod widget_declaration_parser;
mod component_codegen;
mod lazy_children_codegen;
// 定义 Widget props 与私有状态的类型化 Rust 绑定。
mod widget_binding_codegen;
// 定义 Option、集合与嵌套 record 的递归类型化初始值生成。
mod widget_typed_value_codegen;
// 集中验证扩展集合、Some 与嵌套 record 初始值。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/widget_typed_value_tests.rs"]
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
#[path = "../../tests-src/uix_lang/widget_codegen_tests.rs"]
mod widget_codegen_tests;
// 集中验证 For 内 Widget 的逐实例身份与插槽作用域。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/widget_for_codegen_tests.rs"]
mod widget_for_codegen_tests;
// 集中验证默认与具名 Slot 的展开和诊断。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/widget_slot_tests.rs"]
mod widget_slot_tests;
// 定义组件字段表达式改写与 setState 降低。
mod widget_expression_lower;
// 定义组件内 setStyle 的作用域校验与表达式降低。
mod dynamic_style_lower;
// 定义状态伪类到既有 hover、disabled 与 checked 事实的降低。
mod pseudo_style_lower;
// 定义 Widget 声明级语法与类型白名单解析。
mod widget_parser;
// 定义组件专属值类型的 capability 门禁与 schema 反向映射。
pub(crate) mod value_type_gate;
// 定义顶层 Visual 具名静态字段解析与 Rust 字段映射。
mod visual_parser;
// 定义 Widget prop 类型后缀与受限默认表达式解析。
mod widget_prop_default_parser;
// 定义组件调用属性完整性、唯一性与必填校验。
mod widget_call_validator;
// 集中验证 Widget props、state 与名称诊断。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/widget_tests.rs"]
mod widget_tests;
// 集中验证 Widget 成员声明块解析、双写拒绝与等价展开。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/widget_member_block_tests.rs"]
mod widget_member_block_tests;
// 集中验证同步 action 声明、静态展开与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/widget_action_tests.rs"]
mod widget_action_tests;
// 集中验证 Widget prop 默认值、覆盖与必填诊断。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/widget_prop_default_tests.rs"]
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
#[path = "../../tests-src/uix_lang/event_codegen_tests.rs"]
mod event_codegen_tests;
// 定义受限表达式的词法事实层。
mod expression_lexer;
// 定义受限表达式优先级解析与语义验证。
mod expression_parser;
// 集中验证 Affix 生成、状态绑定与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/affix_codegen_tests.rs"]
mod affix_codegen_tests;
// 集中验证 BackTop 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/back_top_codegen_tests.rs"]
mod back_top_codegen_tests;
// 集中验证 Splitter 生成、双面板形状与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/splitter_codegen_tests.rs"]
mod splitter_codegen_tests;
// 定义 <App> 根文档到现有 App builder 的编译期组装边界。
mod app_codegen;
// 集中验证应用入口属性、主题与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/app_codegen_tests.rs"]
mod app_codegen_tests;
// 集中验证布局壳嵌套、布尔简写与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/app_layout_codegen_tests.rs"]
mod app_layout_codegen_tests;
// 集中验证 Input 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/input_codegen_tests.rs"]
mod input_codegen_tests;
// 集中验证 InputNumber 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/input_number_codegen_tests.rs"]
mod input_number_codegen_tests;
// 集中验证 InputGroup 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/input_group_codegen_tests.rs"]
mod input_group_codegen_tests;
// 集中验证 Slider 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/slider_codegen_tests.rs"]
mod slider_codegen_tests;
// 集中验证 RangeSlider 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/range_slider_codegen_tests.rs"]
mod range_slider_codegen_tests;
// 集中验证 Rate 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/rate_codegen_tests.rs"]
mod rate_codegen_tests;
// 集中验证 Checkbox 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/checkbox_codegen_tests.rs"]
mod checkbox_codegen_tests;
// 集中验证 Switch 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/switch_codegen_tests.rs"]
mod switch_codegen_tests;
// 集中验证 Radio 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/radio_codegen_tests.rs"]
mod radio_codegen_tests;
// 集中验证 Segmented 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/segmented_codegen_tests.rs"]
mod segmented_codegen_tests;
// 集中验证 Select 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/select_codegen_tests.rs"]
mod select_codegen_tests;
// 集中验证 Cascader 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/cascader_codegen_tests.rs"]
mod cascader_codegen_tests;
// 集中验证 TreeSelect 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/tree_select_codegen_tests.rs"]
mod tree_select_codegen_tests;
// 集中验证 AutoComplete 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/autocomplete_codegen_tests.rs"]
mod autocomplete_codegen_tests;
// 集中验证 Mentions 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/mentions_codegen_tests.rs"]
mod mentions_codegen_tests;
// 集中验证 DatePicker 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/date_picker_codegen_tests.rs"]
mod date_picker_codegen_tests;
// 集中验证 DateRangePicker 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/date_range_picker_codegen_tests.rs"]
mod date_range_picker_codegen_tests;
// 集中验证 TimePicker 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/time_picker_codegen_tests.rs"]
mod time_picker_codegen_tests;
// 集中验证 ColorPicker 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/color_picker_codegen_tests.rs"]
mod color_picker_codegen_tests;
// 集中验证 Avatar 生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/avatar_codegen_tests.rs"]
mod avatar_codegen_tests;
// 集中验证 Badge 生成、组合形状与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/badge_codegen_tests.rs"]
mod badge_codegen_tests;
// 集中验证 Image 生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/image_codegen_tests.rs"]
mod image_codegen_tests;
// 集中验证 ImageGroup 生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/image_group_codegen_tests.rs"]
mod image_group_codegen_tests;
// 集中验证 List 生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/list_codegen_tests.rs"]
mod list_codegen_tests;
// 集中验证 SelectableList 生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/selectable_list_codegen_tests.rs"]
mod selectable_list_codegen_tests;
// 集中验证 Collapse 生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/collapse_codegen_tests.rs"]
mod collapse_codegen_tests;
// 集中验证 Upload 生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/upload_codegen_tests.rs"]
mod upload_codegen_tests;
// 集中验证反馈声明生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/feedback_declaration_codegen_tests.rs"]
mod feedback_declaration_codegen_tests;
// 集中验证 Skeleton 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/skeleton_codegen_tests.rs"]
mod skeleton_codegen_tests;
// 集中验证 Empty 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/empty_codegen_tests.rs"]
mod empty_codegen_tests;
// 集中验证 ResultView 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/result_view_codegen_tests.rs"]
mod result_view_codegen_tests;
// 集中验证 Tag 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/tag_codegen_tests.rs"]
mod tag_codegen_tests;
// 集中验证 Card 生成、控制子树与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/card_codegen_tests.rs"]
mod card_codegen_tests;
// 集中验证 Descriptions 生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/descriptions_codegen_tests.rs"]
mod descriptions_codegen_tests;
// 集中验证 Timeline 生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/timeline_codegen_tests.rs"]
mod timeline_codegen_tests;
// 集中验证 Calendar 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/calendar_codegen_tests.rs"]
mod calendar_codegen_tests;
// 集中验证 Carousel 生成、默认值、控制流与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/carousel_codegen_tests.rs"]
mod carousel_codegen_tests;
// 集中验证 Tree 生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/tree_codegen_tests.rs"]
mod tree_codegen_tests;
// 集中验证 Terminal 生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/terminal_codegen_tests.rs"]
mod terminal_codegen_tests;
// 集中验证 Table 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/table_codegen_tests.rs"]
mod table_codegen_tests;
// 集中验证 Tabs 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/tabs_codegen_tests.rs"]
mod tabs_codegen_tests;
// 集中验证 Menu 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/menu_codegen_tests.rs"]
mod menu_codegen_tests;
// 集中验证 MenuBar 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/menu_bar_codegen_tests.rs"]
mod menu_bar_codegen_tests;
// 集中验证 Dropdown 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/dropdown_codegen_tests.rs"]
mod dropdown_codegen_tests;
// 集中验证 Navigation 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/navigation_codegen_tests.rs"]
mod navigation_codegen_tests;
// 集中验证 Steps 生成、状态绑定与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/steps_codegen_tests.rs"]
mod steps_codegen_tests;
// 集中验证 Pagination 生成、默认值、状态绑定与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/pagination_codegen_tests.rs"]
mod pagination_codegen_tests;
// 集中验证 Breadcrumb 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/breadcrumb_codegen_tests.rs"]
mod breadcrumb_codegen_tests;
// 集中验证 Anchor 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/anchor_codegen_tests.rs"]
mod anchor_codegen_tests;
// 集中验证 QRCode 生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/qrcode_codegen_tests.rs"]
mod qrcode_codegen_tests;
// 集中验证 Watermark 生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/watermark_codegen_tests.rs"]
mod watermark_codegen_tests;
// 集中验证 RichText 生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/rich_text_codegen_tests.rs"]
mod rich_text_codegen_tests;
// 集中验证 Alert 生成、默认值、事件与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/alert_codegen_tests.rs"]
mod alert_codegen_tests;
// 集中验证 ProgressBar 生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/progress_codegen_tests.rs"]
mod progress_codegen_tests;
// 集中验证三类基础图表生成、数据类型与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/basic_chart_codegen_tests.rs"]
mod basic_chart_codegen_tests;
// 集中验证三类静态高级图表生成、枚举与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/static_chart_codegen_tests.rs"]
mod static_chart_codegen_tests;
// 集中验证矩形树图与仪表盘生成、数据构造和拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/hierarchy_gauge_chart_codegen_tests.rs"]
mod hierarchy_gauge_chart_codegen_tests;
// 集中验证热力图与瀑布图生成、混合数据和拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/matrix_delta_chart_codegen_tests.rs"]
mod matrix_delta_chart_codegen_tests;
// 集中验证雷达图与组合图生成、泛型集合和拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/series_chart_codegen_tests.rs"]
mod series_chart_codegen_tests;
// 集中验证 Popconfirm 生成、组合 trigger、回调与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/popconfirm_codegen_tests.rs"]
mod popconfirm_codegen_tests;
// 集中验证 Modal 生成、默认值、事件与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/modal_codegen_tests.rs"]
mod modal_codegen_tests;
// 集中验证 Drawer 生成、默认值与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/drawer_codegen_tests.rs"]
mod drawer_codegen_tests;
// 集中验证 Tooltip 生成、触发子树与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/tooltip_codegen_tests.rs"]
mod tooltip_codegen_tests;
// 集中验证 Popover 生成、触发子树与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/popover_codegen_tests.rs"]
mod popover_codegen_tests;
// 定义 FocusTrap 有序焦点作用域子树的公开 API 代码生成边界。
mod focus_trap_codegen;
// 集中验证 FocusTrap 生成、控制流与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/focus_trap_codegen_tests.rs"]
mod focus_trap_codegen_tests;
// 集中验证 Spin 生成、默认值、控制流与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/spin_codegen_tests.rs"]
mod spin_codegen_tests;
// 集中验证 Form 生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/form_codegen_tests.rs"]
mod form_codegen_tests;
// 集中验证 FloatButtonGroup 生成、事件保留与直接子项诊断。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/float_button_group_codegen_tests.rs"]
mod float_button_group_codegen_tests;
// 定义 Container 专属视觉与布局简写的独立生成边界。
mod view_shorthand_codegen;
// 集中验证布局标签生成、父子形状与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/layout_codegen_tests.rs"]
mod layout_codegen_tests;
// 集中验证 VirtualScroll 生成、身份与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/virtual_scroll_codegen_tests.rs"]
mod virtual_scroll_codegen_tests;
// 集中验证表达式与控制绑定 Gate。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/expression_tests.rs"]
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
// 集中验证背景来源、定位、重复、尺寸策略与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/style_background_codegen_tests.rs"]
mod style_background_codegen_tests;
// 集中验证 backgroundSize 关键字、轴尺寸与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/style_size_codegen_tests.rs"]
mod style_size_codegen_tests;
// 定义 borderStyle 关键字到 UI 边框线型契约的独立映射。
mod style_border_codegen;
// 集中验证 borderStyle 的完整枚举生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/style_border_codegen_tests.rs"]
mod style_border_codegen_tests;
// 定义 fontFamily 有序列表到 UI 字体族契约的独立映射。
mod style_font_family_codegen;
// 定义 float/clear 非目标布局决策的专用编译诊断。
mod style_float_codegen;
// 集中验证规范值、非法值与可执行替代建议。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/style_float_codegen_tests.rs"]
mod style_float_codegen_tests;
// 集中验证 fontFamily 的列表生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/style_font_family_codegen_tests.rs"]
mod style_font_family_codegen_tests;
// 定义 fontWeight 关键字与整数到 UI 字体粗细契约的独立映射。
mod style_font_weight_codegen;
// 集中验证 fontWeight 的完整生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/style_font_weight_codegen_tests.rs"]
mod style_font_weight_codegen_tests;
// 定义 lineHeight 倍率与像素值到 UI 行高契约的独立映射。
mod style_line_height_codegen;
// 集中验证 lineHeight 的单位生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/style_line_height_codegen_tests.rs"]
mod style_line_height_codegen_tests;
// 定义保留单位长度（px/百分比/有限 calc）到 UI StyleLength 契约的独立映射。
mod style_length_codegen;
// 定义 @media 窗口宽度条件到构建期判定表达式的独立映射。
mod media_query_codegen;
// 集中验证 @media 解析、分层顺序与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/media_query_tests.rs"]
mod media_query_tests;
// 集中验证 Grid minmax 有界轨道的生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/grid_minmax_codegen_tests.rs"]
mod grid_minmax_codegen_tests;
// 集中验证 min/max 尺寸、百分比与 calc 的生成和拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/style_length_codegen_tests.rs"]
mod style_length_codegen_tests;
// 定义 position 与四边值到 UI 定位契约的独立映射。
mod style_position_codegen;
// 集中验证五模式、四边像素与 auto 的生成和拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/style_position_codegen_tests.rs"]
mod style_position_codegen_tests;
// 定义 textAlign 关键字到 UI 文本水平对齐契约的独立映射。
mod style_text_align_codegen;
// 集中验证 textAlign 的完整枚举生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/style_text_align_codegen_tests.rs"]
mod style_text_align_codegen_tests;
// 定义 textDecoration 关键字到 UI 文本装饰契约的独立映射。
mod style_text_decoration_codegen;
// 集中验证 textDecoration 的完整枚举生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/style_text_decoration_codegen_tests.rs"]
mod style_text_decoration_codegen_tests;
// 定义样式类继承、引用与内联优先级改写。
mod style_class_resolver;
// 定义样式值、颜色、边距与枚举的编译期映射。
mod style_value_codegen;
// 定义完整盒阴影值与字段更新的编译期映射。
mod style_shadow_codegen;
// 集中验证盒阴影旧语法与可选 spread 的生成契约。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/style_shadow_codegen_tests.rs"]
mod style_shadow_codegen_tests;
// 定义 transform 函数列表到公开二维仿射矩阵的编译期映射。
mod style_transform_codegen;
// 定义 cursor 文档值到公开平台无关枚举的独立映射。
mod style_cursor_codegen;
// 集中验证 transform 与 transformOrigin 的生成和拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/style_transform_codegen_tests.rs"]
mod style_transform_codegen_tests;
// 集中验证 cursor 的公开枚举生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/style_cursor_codegen_tests.rs"]
mod style_cursor_codegen_tests;
// 定义 userSelect 关键字到 UI 文字选择策略的独立映射。
mod style_user_select_codegen;
// 集中验证 userSelect 的公开枚举生成与拒绝路径。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/style_user_select_codegen_tests.rs"]
mod style_user_select_codegen_tests;
// 定义样式块与内联样式共享解析器。
mod style_parser;
// 集中验证顶层声明与样式 Gate。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/style_tests.rs"]
mod style_tests;

#[cfg(test)]
#[path = "../../tests-src/uix_lang/test_document_view.rs"]
mod test_document_view;
// 保持移入前的命名空间：兄弟测试模块仍以 use super::generate_test_document_view 引用。
#[cfg(test)]
use test_document_view::generate_test_document_view;
// 定义公共属性值与布局枚举的 Rust 映射。
mod value_codegen;

// 向后续转换 Gate 暴露核心语法树类型。
pub(crate) use ast::*;
// 向核心 View 生成器暴露规划中内置组件诊断与完整支持标签提示。
pub(crate) use builtin_matrix::{planned_builtin_diagnostic, supported_builtin_hint};
// 向过程宏入口暴露核心 View 生成函数。
pub(crate) use codegen::generate_view;
// 向核心元素生成器暴露基础 View 桥接映射。
pub(crate) use kernel_view_codegen::{
    generate_kernel_children, generate_kernel_host, generate_kernel_view,
};
// 向核心元素生成器暴露 Canvas 专用映射。
pub(crate) use canvas_codegen::generate_canvas;
// 向组件解析与代码生成暴露结构化组件声明。
pub(crate) use widget_ast::*;
// 向 action 解析、语义与降低阶段暴露独立语句 AST。
pub(crate) use action_ast::*;
// 导出语言面数据类型到公开构造 API 的映射查询。
pub(crate) use data_binding_codegen::{
    DataConstructorSpec, data_chain_root, data_constructor_spec, is_data_constructor_chain,
    is_registered_data_type, normalize_number_literals,
};
// 向过程宏入口暴露组件感知文档生成函数。
pub(crate) use widget_codegen::{generate_document_view, generate_document_view_owned};
// 向过程宏入口暴露 App builder 生成函数。
pub(crate) use app_codegen::generate_document_app;
// 向后续转换 Gate 暴露稳定诊断类型。
pub(crate) use diagnostic::*;
// 向文档解析器暴露 Widget 声明验证入口。
pub(crate) use widget_declaration_parser::parse_widget_declaration;
// 向文档解析器暴露 Record 声明验证入口。
pub(crate) use widget_parser::parse_record_declaration;
// 暴露组件专属值类型的能力门禁入口。
pub(crate) use value_type_gate::validate_value_type_capabilities;
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
pub(crate) use event_payload_codegen::{
    generate_event_handler_expression, generate_key_event_handler_expression,
};
// 向表达式解析器暴露词法标记。
pub(crate) use expression_lexer::*;
// 向核心解析器暴露受限表达式入口。
pub(crate) use expression_parser::parse_expression;
// 向核心元素生成器暴露 FocusTrap 容器组件专用映射。
pub(crate) use focus_trap_codegen::generate_focus_trap;
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
    numeric_value, optional_boolean, rust_identifier, string_value, typography_value,
};

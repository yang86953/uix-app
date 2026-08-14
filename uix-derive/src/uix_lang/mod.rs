// 定义转换器内部共享的带跨度语法树。
mod ast;
// 定义六类文档内置组件的已登记与规划中矩阵。
mod builtin_matrix;
// 定义语言面结构化数据类型到公开构造 API 的编译期映射。
mod data_binding_codegen;
// 定义组件内 setStyle 的闭合分支代码生成。
mod dynamic_style_codegen;
// 定义 For 实际实例路径的内部标识符读取。
mod for_identity_codegen;
// 定义最终 ViewNode 的组件状态装饰应用。
mod view_decoration_codegen;
// 定义 record 声明到模块级结构体的生成与语言类型映射。
mod record_codegen;
// 集中验证 record 生成、对象初始值与拒绝路径。
#[cfg(test)]
mod record_codegen_tests;
// 集中验证数据构造生成、枚举映射与拒绝路径。
#[cfg(test)]
mod data_binding_codegen_tests;
// 定义 ButtonGroup 内置组件的公开 API 代码生成边界。
mod button_group_codegen;
// 定义核心元素、属性、事件与控制流的 Rust View 代码生成。
mod codegen;
// 定义元素名称到既有专用生成器的确定性分派。
mod element_codegen;
// 定义 Divider 内置组件的公开 API 代码生成边界。
mod divider_codegen;
// 定义 Space 内置组件的公开 API 代码生成边界。
mod space_codegen;
// 定义 Typography 内置组件的公开 API 代码生成边界。
mod typography_codegen;
// 定义 ThemeToggle 内置组件的公开 API 代码生成边界。
mod theme_toggle_codegen;
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
// 定义 Component、props 与 state 的结构化 AST。
mod component_ast;
// 定义 Component props 与私有状态的类型化 Rust 绑定。
mod component_binding_codegen;
// 定义完整文档中的组件调用展开。
mod component_codegen;
// 集中验证组件展开、状态与拒绝路径。
#[cfg(test)]
mod component_codegen_tests;
// 定义组件字段表达式改写与 setState 降低。
mod component_expression_lower;
// 定义组件内 setStyle 的作用域校验与表达式降低。
mod dynamic_style_lower;
// 定义 Component 声明级语法与类型白名单解析。
mod component_parser;
// 集中验证 Component props、state 与名称诊断。
#[cfg(test)]
mod component_tests;
// 定义包含位置、原因与修复建议的解析诊断。
mod diagnostic;
// 定义顶层指令、样式类与主题解析。
mod declaration_parser;
// 定义受限表达式的确定性 AST。
mod expression_ast;
// 定义受限表达式到 Rust 令牌的确定性转换。
mod expression_codegen;
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
// 定义样式类继承、引用与内联优先级改写。
mod style_class_resolver;
// 定义样式值、颜色、边距与枚举的编译期映射。
mod style_value_codegen;
// 定义样式块与内联样式共享解析器。
mod style_parser;
// 集中验证顶层声明与样式 Gate。
#[cfg(test)]
mod style_tests;
// 定义公共属性值与布局枚举的 Rust 映射。
mod value_codegen;

// 向后续转换 Gate 暴露核心语法树类型。
pub(crate) use ast::*;
// 向核心 View 生成器暴露规划中内置组件诊断。
pub(crate) use builtin_matrix::planned_builtin_diagnostic;
// 向核心元素生成器暴露 ButtonGroup 专用映射。
pub(crate) use button_group_codegen::generate_button_group;
// 向过程宏入口暴露核心 View 生成函数。
pub(crate) use codegen::generate_view;
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
pub(crate) use component_ast::*;
// 导出语言面数据类型到公开构造 API 的映射查询。
pub(crate) use data_binding_codegen::{
    DataConstructorSpec, data_chain_root, data_constructor_spec, is_data_constructor_chain,
    normalize_number_literals, step_status_path,
};
// 向过程宏入口暴露组件感知文档生成函数。
pub(crate) use component_codegen::generate_document_view;
// 向过程宏入口暴露 App builder 生成函数。
pub(crate) use app_codegen::generate_document_app;
// 向后续转换 Gate 暴露稳定诊断类型。
pub(crate) use diagnostic::*;
// 向文档解析器暴露 Component 与 Record 声明验证入口。
pub(crate) use component_parser::{parse_component_declaration, parse_record_declaration};
// 向 uix_items! 与组件绑定暴露 record 生成与类型映射入口。
pub(crate) use record_codegen::{generate_record_items, value_type_tokens};
// 向文档解析器暴露顶层声明入口。
pub(crate) use declaration_parser::{
    parse_at_declaration, parse_style_class, register_declaration_name,
    starts_component_declaration, starts_record_declaration,
};
// 向核心解析器暴露表达式 AST。
pub(crate) use expression_ast::*;
// 向 View 生成器暴露表达式与事件处理器生成入口。
pub(crate) use expression_codegen::{
    expression_uses_event, generate_expression, generate_handler_expression,
};
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
// 向核心元素生成器暴露 List 叶组件专用映射。
pub(crate) use list_codegen::generate_list;
// 向核心元素生成器暴露 SelectableList 叶组件专用映射。
pub(crate) use selectable_list_codegen::generate_selectable_list;
// 向核心元素生成器暴露 Collapse 叶组件专用映射。
pub(crate) use collapse_codegen::generate_collapse;
// 向核心元素生成器暴露 Upload 叶组件专用映射。
pub(crate) use upload_codegen::generate_upload;
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
pub(crate) use parser::parse_document;
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

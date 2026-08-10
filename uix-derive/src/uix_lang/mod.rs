// 定义转换器内部共享的带跨度语法树。
mod ast;
// 定义六类文档内置组件的已登记与规划中矩阵。
mod builtin_matrix;
// 定义 ButtonGroup 内置组件的公开 API 代码生成边界。
mod button_group_codegen;
// 定义核心元素、属性、事件与控制流的 Rust View 代码生成。
mod codegen;
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
// 定义 Skeleton 静态外观与尺寸的公开 API 代码生成边界。
mod skeleton_codegen;
// 集中验证 Skeleton 生成与拒绝路径。
#[cfg(test)]
mod skeleton_codegen_tests;
// 定义 Form 与类型化字段的公开 API 代码生成边界。
mod form_codegen;
// 定义 FormSliderItem 类型化 f64 字段的独立生成边界。
mod form_slider_codegen;
// 集中验证 Form 生成与拒绝路径。
#[cfg(test)]
mod form_codegen_tests;
// 定义 FloatButton 内置组件的公开 API 代码生成边界。
mod float_button_codegen;
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
// 向组件解析与代码生成暴露结构化组件声明。
pub(crate) use component_ast::*;
// 向过程宏入口暴露组件感知文档生成函数。
pub(crate) use component_codegen::generate_document_view;
// 向后续转换 Gate 暴露稳定诊断类型。
pub(crate) use diagnostic::*;
// 向文档解析器暴露 Component 声明验证入口。
pub(crate) use component_parser::parse_component_declaration;
// 向文档解析器暴露顶层声明入口。
pub(crate) use declaration_parser::{
    parse_at_declaration, parse_style_class, register_declaration_name,
    starts_component_declaration,
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
// 向核心元素生成器暴露 Skeleton 专用映射。
pub(crate) use skeleton_codegen::generate_skeleton;
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
// 向 View 生成器暴露属性值共享映射入口。
pub(crate) use value_codegen::{
    align_value, boolean_value, deferred_style_diagnostic, justify_value, literal_string,
    numeric_value, rust_identifier, string_value, typography_value,
};

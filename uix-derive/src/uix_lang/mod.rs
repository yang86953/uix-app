// 定义转换器内部共享的带跨度语法树。
mod ast;
// 定义核心元素、属性、事件与控制流的 Rust View 代码生成。
mod codegen;
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
// 向过程宏入口暴露核心 View 生成函数。
pub(crate) use codegen::generate_view;
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
// 向核心解析器暴露 UTF-8 安全词法游标。
pub(crate) use lexer::Cursor;
// 向过程宏入口暴露文档解析函数。
pub(crate) use parser::parse_document;
// 向文档解析器暴露声明与样式 AST。
pub(crate) use style_ast::*;
// 向 View 生成器暴露内联样式映射入口。
pub(crate) use style_codegen::apply_inline_style;
// 向文档解析器暴露共享样式入口。
pub(crate) use style_parser::parse_style_properties;
// 向 View 生成器暴露属性值共享映射入口。
pub(crate) use value_codegen::{
    align_value, boolean_value, deferred_style_diagnostic, justify_value, literal_string,
    numeric_value, rust_identifier, string_value, typography_value,
};

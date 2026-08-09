// 定义转换器内部共享的带跨度语法树。
mod ast;
// 定义包含位置、原因与修复建议的解析诊断。
mod diagnostic;
// 定义受限表达式的确定性 AST。
mod expression_ast;
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

// 向后续转换 Gate 暴露核心语法树类型。
pub(crate) use ast::*;
// 向后续转换 Gate 暴露稳定诊断类型。
pub(crate) use diagnostic::*;
// 向核心解析器暴露表达式 AST。
pub(crate) use expression_ast::*;
// 向表达式解析器暴露词法标记。
pub(crate) use expression_lexer::*;
// 向核心解析器暴露受限表达式入口。
pub(crate) use expression_parser::parse_expression;
// 向核心解析器暴露 UTF-8 安全词法游标。
pub(crate) use lexer::Cursor;

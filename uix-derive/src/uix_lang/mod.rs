// 定义转换器内部共享的带跨度语法树。
mod ast;
// 定义包含位置、原因与修复建议的解析诊断。
mod diagnostic;
// 定义面向 UTF-8 源码的词法游标。
mod lexer;
// 定义唯一根元素与核心节点的递归下降解析器。
mod parser;

// 向后续转换 Gate 暴露核心语法树类型。
pub(crate) use ast::*;
// 向后续转换 Gate 暴露稳定诊断类型。
pub(crate) use diagnostic::*;
// 向核心解析器暴露 UTF-8 安全词法游标。
pub(crate) use lexer::Cursor;

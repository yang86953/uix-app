//! 类型化组件语言的结构化前端。
//!
//! 这是显式实验入口，不改变旧 `CompileTarget` 的含义。解析只建立源码事实，
//! 不承诺类型正确、导入可用或可执行；后续共享语义阶段消费此树，不能把它重新
//! 编码成旧 XML 字符串属性。所有区间为原文件 UTF-8 字节半开区间。

mod ast;
mod check;
mod emit;
mod format;
/// 来自同一 Rust 属性声明的有界原生接口文件；不含调用实现。
pub mod interface;
mod link;
mod lower;
mod parser;
mod prune;
mod tools;

pub use ast::*;
pub use check::{
    BindingFact, BindingKind, CheckedSource, ComponentSignature, ExpressionFact, FunctionSignature,
    NativeExport, NativeLibraries, NativeLibrary, NodeId, ResolvedName, Type, check,
};
pub use emit::emit_native;
pub use format::format;
pub use link::{
    Binding, DeclarationId, LinkedSource, SourceUnit, link_file, link_file_with_overlays,
    link_inline,
};
pub use lower::lower;
pub use parser::recognizes_source;
pub use tools::ComponentOutput;
pub(crate) use tools::document_prefix;

use super::{CompilerDiagnostic, source_graph::SourceGraph};

/// 保存原始源码与直接解析所得的组件声明，不含生成的中间源码。
#[derive(Debug, Clone)]
pub struct ParsedSource {
    pub source_graph: SourceGraph,
    pub imports: Vec<Import>,
    pub declarations: Vec<Declaration>,
}

/// 解析一份组件源码。此入口不读取文件系统或推断任何官方组件库。
pub fn parse(
    source: &str,
    source_name: impl Into<String>,
) -> Result<ParsedSource, CompilerDiagnostic> {
    parser::parse(source, source_name.into())
}

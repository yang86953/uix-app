//! UIX Lang 共享构建期编译器。
//!
//! 本 crate 拥有语言解析、导入图、语义检查与 Rust UI 生成；过程宏、命令行和
//! 编辑器能力只能通过这里的公开编译命令进入，不得维护独立语言规则。

use std::path::{Path, PathBuf};

use proc_macro2::TokenStream;
use source_graph::SourceId;

/// 导出编译器、工具链与文档生成共同消费的 UI 投影登记事实。
pub mod projection_schema;
/// 导出稳定源码身份、内容摘要与导入边契约。
pub mod source_graph;
mod uix_import;
#[cfg(test)]
mod uix_import_tests;
#[allow(dead_code)]
mod uix_lang;

use uix_import::{ImportDiagnostic, reject_inline_imports, resolve_file};
use uix_lang::{
    Diagnostic, Document, generate_document_app, generate_document_view, generate_record_items,
    parse_document, with_source_markers,
};

/// 声明一次编译需要生成的公开入口形状。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileTarget {
    /// 生成 `ViewNode` 表达式。
    View,
    /// 生成尚未运行的 `App` builder 表达式。
    App,
    /// 生成文档内全部模块级 Record 项。
    Items,
}

/// 标识诊断首次成立的 Compiler System 阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticPhase {
    /// 源文件读取、编码或入口失败。
    Source,
    /// 词法、语法与单文件 AST 建立失败。
    Syntax,
    /// 导入路径、可见性、循环或合并失败。
    Import,
    /// 名称、类型、组件投影或能力检查失败。
    Semantic,
    /// 已验证 IR 到 Rust 输出的内部失败。
    Emit,
}

impl DiagnosticPhase {
    /// 返回供 CLI、LSP 与快照消费的稳定小写名称。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Syntax => "syntax",
            Self::Import => "import",
            Self::Semantic => "semantic",
            Self::Emit => "emit",
        }
    }
}

/// 保存 Compiler System 成功建立的确定性 Rust 结果与递归文件依赖。
#[derive(Debug)]
pub struct CompileOutput {
    /// 保存供入口 Adapter 消费的 Rust 令牌。
    pub tokens: TokenStream,
    /// 保存根文件及递归导入闭包中的规范路径；内嵌输入为空。
    pub tracked_files: Vec<PathBuf>,
    /// 保存本次编译读取的稳定源码图。
    pub source_graph: source_graph::SourceGraph,
}

/// 保存与入口机制无关的结构化语言诊断。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerDiagnostic {
    /// 保存跨入口稳定的诊断代码。
    pub code: &'static str,
    /// 保存诊断首次成立的编译阶段。
    pub phase: DiagnosticPhase,
    /// 保存与 SourceGraph 一致的稳定源码身份。
    pub source_id: SourceId,
    /// 保存真实文件路径或内嵌来源标签。
    pub source_name: String,
    /// 保存 UTF-8 字节半开区间起点。
    pub start: usize,
    /// 保存 UTF-8 字节半开区间终点。
    pub end: usize,
    /// 保存一基行号。
    pub line: usize,
    /// 保存一基列号。
    pub column: usize,
    /// 保存失败原因。
    pub message: String,
    /// 保存可执行修复建议。
    pub suggestion: String,
}

impl CompilerDiagnostic {
    // 把语言内部诊断投影为 Compiler System 公开诊断。
    fn from_language(
        source_name: impl Into<String>,
        source_id: SourceId,
        phase: DiagnosticPhase,
        code: &'static str,
        diagnostic: Diagnostic,
    ) -> Self {
        Self {
            code,
            phase,
            source_id,
            source_name: source_name.into(),
            start: diagnostic.span.start,
            end: diagnostic.span.end,
            line: diagnostic.span.line,
            column: diagnostic.span.column,
            message: diagnostic.message,
            suggestion: diagnostic.suggestion,
        }
    }

    // 保留 Source Module 已确定的真实失败文件。
    fn from_import(diagnostic: ImportDiagnostic) -> Self {
        let source_id = SourceId::from_source_name(&diagnostic.source_name);
        Self::from_language(
            diagnostic.source_name,
            source_id,
            diagnostic.phase,
            diagnostic.code,
            diagnostic.diagnostic,
        )
    }
}

/// 编译没有文件系统基准目录的内嵌 UIX 源码。
pub fn compile_inline(
    source: &str,
    source_name: impl Into<String>,
    target: CompileTarget,
) -> Result<CompileOutput, CompilerDiagnostic> {
    let source_name = source_name.into();
    let source_graph = source_graph::SourceGraph::inline(&source_name, source);
    let source_id = source_graph.root();
    let document = parse_document(source).map_err(|diagnostic| {
        CompilerDiagnostic::from_language(
            &source_name,
            source_id,
            DiagnosticPhase::Syntax,
            "UIX1000",
            diagnostic,
        )
    })?;
    reject_inline_imports(&document).map_err(|diagnostic| {
        CompilerDiagnostic::from_language(
            &source_name,
            source_id,
            DiagnosticPhase::Import,
            "UIX1100",
            diagnostic,
        )
    })?;
    let tokens = compile_document(&document, target).map_err(|diagnostic| {
        CompilerDiagnostic::from_language(
            &source_name,
            source_id,
            DiagnosticPhase::Semantic,
            "UIX2000",
            diagnostic,
        )
    })?;
    Ok(CompileOutput {
        tokens,
        tracked_files: Vec::new(),
        source_graph,
    })
}

/// 编译一个真实 `.uix` 根文件及其递归导入闭包。
pub fn compile_file(
    path: &Path,
    target: CompileTarget,
) -> Result<CompileOutput, CompilerDiagnostic> {
    let resolved = resolve_file(path).map_err(CompilerDiagnostic::from_import)?;
    let source_id = resolved.source_graph.root();
    let source_name = resolved
        .source_graph
        .file(source_id)
        .map(|file| file.path.clone())
        .unwrap_or_else(|| path.display().to_string());
    let tokens = compile_document(&resolved.document, target).map_err(|diagnostic| {
        CompilerDiagnostic::from_language(
            source_name,
            source_id,
            DiagnosticPhase::Semantic,
            "UIX2000",
            diagnostic,
        )
    })?;
    Ok(CompileOutput {
        tokens,
        tracked_files: resolved.tracked_files,
        source_graph: resolved.source_graph,
    })
}

// 让全部入口共享唯一的解析后生成路径。
fn compile_document(document: &Document, target: CompileTarget) -> Result<TokenStream, Diagnostic> {
    match target {
        CompileTarget::View => with_source_markers(|| generate_document_view(document)),
        CompileTarget::App => with_source_markers(|| generate_document_app(document)),
        CompileTarget::Items => generate_record_items(document),
    }
}

#[cfg(test)]
mod tests {
    use super::{CompileTarget, DiagnosticPhase, compile_inline};
    use crate::source_graph::SourceId;

    #[test]
    fn shared_commands_compile_all_public_target_shapes() {
        let view = compile_inline("<Text>Hello</Text>", "<view>", CompileTarget::View)
            .expect("View 编译应成功");
        assert!(view.tokens.to_string().contains("label"));
        assert!(view.tracked_files.is_empty());

        let app = compile_inline("<App><Text>Hello</Text></App>", "<app>", CompileTarget::App)
            .expect("App 编译应成功");
        assert!(app.tokens.to_string().contains("App :: new"));

        let items = compile_inline(
            "<Record name=\"User\" fields=\"name: String\" /><Text>Hello</Text>",
            "<items>",
            CompileTarget::Items,
        )
        .expect("Record 编译应成功");
        assert!(items.tokens.to_string().contains("struct User"));
    }

    #[test]
    fn shared_commands_preserve_structured_diagnostics() {
        let error = compile_inline("<Mystery />", "demo.uix", CompileTarget::View)
            .expect_err("未知标签必须失败");
        assert_eq!(error.source_name, "demo.uix");
        assert_eq!(error.source_id, SourceId::from_source_name("demo.uix"));
        assert_eq!(error.code, "UIX2000");
        assert_eq!(error.phase, DiagnosticPhase::Semantic);
        assert_eq!(error.line, 1);
        assert!(error.message.contains("Mystery"));
        assert!(!error.suggestion.is_empty());
    }

    #[test]
    fn shared_commands_distinguish_syntax_and_import_diagnostics() {
        let syntax = compile_inline("<Text>", "syntax.uix", CompileTarget::View)
            .expect_err("未闭合标签必须失败");
        assert_eq!(syntax.code, "UIX1000");
        assert_eq!(syntax.phase, DiagnosticPhase::Syntax);

        let import = compile_inline(
            "@import('./card.uix', 'Card')\n<Text />",
            "import.uix",
            CompileTarget::View,
        )
        .expect_err("内嵌导入必须失败");
        assert_eq!(import.code, "UIX1100");
        assert_eq!(import.phase, DiagnosticPhase::Import);
    }
}

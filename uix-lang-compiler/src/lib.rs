//! UIX Lang 共享构建期编译器。
//!
//! 本 crate 拥有语言解析、导入图、语义检查与 Rust UI 生成；过程宏、命令行和
//! 编辑器能力只能通过这里的公开编译命令进入，不得维护独立语言规则。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::{fs, io};

use proc_macro2::TokenStream;
use projection_schema::{UI_PROJECTION_SCHEMA, UiProjectionSchema};
use semantic_ir::{TypedUiIr, lower_document};
use source_graph::SourceId;
use source_map::SourceMap;

mod compiler_session;
mod formatter;
pub use compiler_session::CompilerSession;
/// 导出可选的开发期纯 UI AOT 热重载入口。
#[cfg(feature = "hot-reload")]
pub mod hot_reload;
/// 导出覆盖全部源码字节的无损具体语法流。
pub mod lossless_cst;
/// 导出编译器、工具链与文档生成共同消费的 UI 投影登记事实。
pub mod projection_schema;
/// 导出完成名称、角色和值形状分类的语义 IR。
pub mod semantic_ir;
/// 导出稳定源码身份、内容摘要与导入边契约。
pub mod source_graph;
/// 导出 Rust 输出到 UIX 源码与语义节点的确定映射。
pub mod source_map;
mod uix_import;
#[cfg(test)]
mod uix_import_tests;
#[allow(dead_code)]
mod uix_lang;

use uix_import::{
    ImportDiagnostic, reject_inline_imports, resolve_file, resolve_file_with_overlays,
    resolve_items_file, resolve_items_file_with_overlays,
};
use uix_lang::{
    Diagnostic, SourceSpan, generate_document_app, generate_document_items, generate_document_view,
    parse_document, parse_items_document, with_source_markers,
};

/// 声明一次编译需要生成的公开入口形状。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompileTarget {
    /// 生成 `ViewNode` 表达式。
    View,
    /// 生成尚未运行的 `App` builder 表达式。
    App,
    /// 生成文档内全部模块级 Record 与 Visual 项。
    Items,
}

/// 区分 Compiler System 可查询的 UI 投影登记类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryKind {
    Components,
    Attributes,
    Events,
    Styles,
    Themes,
    Handles,
    Slots,
    Capabilities,
    CapabilityUses,
    Data,
}

impl QueryKind {
    /// 解析 CLI、LSP 与其他 Adapter 共用的稳定类别名。
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "components" => Some(Self::Components),
            "attributes" => Some(Self::Attributes),
            "events" => Some(Self::Events),
            "styles" => Some(Self::Styles),
            "themes" => Some(Self::Themes),
            "handles" => Some(Self::Handles),
            "slots" => Some(Self::Slots),
            "capabilities" => Some(Self::Capabilities),
            "capability-uses" => Some(Self::CapabilityUses),
            "data" => Some(Self::Data),
            _ => None,
        }
    }

    /// 返回稳定协议名称。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Components => "components",
            Self::Attributes => "attributes",
            Self::Events => "events",
            Self::Styles => "styles",
            Self::Themes => "themes",
            Self::Handles => "handles",
            Self::Slots => "slots",
            Self::Capabilities => "capabilities",
            Self::CapabilityUses => "capability-uses",
            Self::Data => "data",
        }
    }
}

/// 保存 query 命令返回的一条只读 schema 登记。
#[derive(Debug, Clone, Copy)]
pub enum QueryEntry {
    Component(&'static projection_schema::ComponentSpec),
    Attribute(&'static projection_schema::AttributeSpec),
    Event(&'static projection_schema::EventSpec),
    Style(&'static projection_schema::StyleSpec),
    Theme(&'static projection_schema::ThemeTokenSpec),
    Handle(&'static projection_schema::HandleSpec),
    Slot(&'static projection_schema::SlotSpec),
    Capability(&'static projection_schema::CapabilitySpec),
    AttributeCapability(&'static projection_schema::AttributeCapabilitySpec),
    Data(&'static projection_schema::DataConstructorSpec),
}

/// 保存 Compiler System 的确定性 schema 查询结果。
#[derive(Debug, Clone)]
pub struct QueryOutput {
    pub kind: QueryKind,
    pub entries: Vec<QueryEntry>,
}

/// 保存可用于阶段缓存与增量失效判定的完整编译身份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilationKey {
    pub root_content_hash: u64,
    pub dependency_hash: u64,
    pub language_version: &'static str,
    pub schema_version: u32,
    pub capabilities: Vec<&'static str>,
    pub compiler_version: &'static str,
    pub target: CompileTarget,
}

impl CompilationKey {
    pub(crate) fn new(source_graph: &source_graph::SourceGraph, target: CompileTarget) -> Self {
        let root_content_hash = source_graph
            .file(source_graph.root())
            .map(|file| file.content_hash)
            .unwrap_or_default();
        Self {
            root_content_hash,
            dependency_hash: source_graph.dependency_hash(),
            language_version: UI_PROJECTION_SCHEMA.language_version(),
            schema_version: UI_PROJECTION_SCHEMA.version(),
            capabilities: enabled_capabilities(),
            compiler_version: env!("CARGO_PKG_VERSION"),
            target,
        }
    }
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
#[derive(Debug, Clone)]
pub struct CompileOutput {
    /// 保存供入口 Adapter 消费的 Rust 令牌。
    pub tokens: TokenStream,
    /// 保存根文件及递归导入闭包中的规范路径；内嵌输入为空。
    pub tracked_files: Vec<PathBuf>,
    /// 保存本次编译读取的稳定源码图。
    pub source_graph: source_graph::SourceGraph,
    /// 保存与 Rust 输出来自同一次分析的类型化 UI IR。
    pub ir: TypedUiIr,
    /// 保存生成 Rust token 文本到 UIX 源码及语义节点的映射。
    pub source_map: SourceMap,
    /// 保存本次 AOT 结果的完整增量身份。
    pub compilation_key: CompilationKey,
}

/// 保存不产生公开 Rust 输出的共享检查结果。
#[derive(Debug, Clone)]
pub struct CheckOutput {
    /// 保存根文件及递归导入闭包中的规范路径。
    pub tracked_files: Vec<PathBuf>,
    /// 保存本次检查读取的稳定源码图。
    pub source_graph: source_graph::SourceGraph,
    /// 保存检查通过后的类型化 UI IR。
    pub ir: TypedUiIr,
    /// 保存本次检查结果的完整增量身份。
    pub compilation_key: CompilationKey,
}

/// 保存 formatter 的无损结果与来源身份。
#[derive(Debug)]
pub struct FormatOutput {
    /// 保存真实文件路径或内嵌来源标签。
    pub source_name: String,
    /// 保存与 SourceGraph 相同算法形成的来源身份。
    pub source_id: SourceId,
    /// 保存规范化后的 UTF-8 源码。
    pub formatted: String,
    /// 表示结果是否与输入字节不同。
    pub changed: bool,
    /// 保存格式化结果的无损 CST。
    pub cst: lossless_cst::LosslessCst,
}

/// 组合语言 Modules、schema 与公开命令的唯一 System 根。
#[derive(Debug, Clone, Copy)]
pub struct CompilerSystem {
    schema: UiProjectionSchema,
}

impl Default for CompilerSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerSystem {
    /// 使用仓库唯一 UI 投影登记创建 Compiler System。
    pub const fn new() -> Self {
        Self {
            schema: UI_PROJECTION_SCHEMA,
        }
    }

    /// 返回本 System 持有的只读 UI 投影 schema。
    pub const fn schema(self) -> UiProjectionSchema {
        self.schema
    }

    /// 编译没有文件系统基准目录的内嵌 UIX 源码。
    pub fn compile_inline(
        self,
        source: &str,
        source_name: impl Into<String>,
        target: CompileTarget,
    ) -> Result<CompileOutput, CompilerDiagnostic> {
        let analyzed = analyze_inline(source, source_name.into(), Some(target))?;
        compile_analyzed(analyzed)
    }

    /// 编译一个真实 `.uix` 根文件及其递归导入闭包。
    pub fn compile_file(
        self,
        path: &Path,
        target: CompileTarget,
    ) -> Result<CompileOutput, CompilerDiagnostic> {
        let analyzed = analyze_file(path, Some(target))?;
        compile_analyzed(analyzed)
    }

    /// 检查内嵌 UIX 并返回与 AOT 相同的语义结论。
    pub fn check_inline(
        self,
        source: &str,
        source_name: impl Into<String>,
        target: CompileTarget,
    ) -> Result<CheckOutput, CompilerDiagnostic> {
        let analyzed = analyze_inline(source, source_name.into(), Some(target))?;
        check_analyzed(analyzed)
    }

    /// 按根元素自动选择 View 或 App 并检查内嵌 UIX。
    pub fn check_inline_auto(
        self,
        source: &str,
        source_name: impl Into<String>,
    ) -> Result<CheckOutput, CompilerDiagnostic> {
        let analyzed = analyze_inline(source, source_name.into(), None)?;
        check_analyzed(analyzed)
    }

    /// 检查真实 `.uix` 根及其递归导入闭包。
    pub fn check_file(
        self,
        path: &Path,
        target: CompileTarget,
    ) -> Result<CheckOutput, CompilerDiagnostic> {
        let analyzed = analyze_file(path, Some(target))?;
        check_analyzed(analyzed)
    }

    /// 按根元素自动选择 View 或 App 并检查真实文件。
    pub fn check_file_auto(self, path: &Path) -> Result<CheckOutput, CompilerDiagnostic> {
        let analyzed = analyze_file(path, None)?;
        check_analyzed(analyzed)
    }

    /// 检查真实根文件，并让 LSP 会话快照覆盖对应磁盘文件。
    pub fn check_file_with_overlays(
        self,
        path: &Path,
        overlays: &BTreeMap<PathBuf, String>,
        target: CompileTarget,
    ) -> Result<CheckOutput, CompilerDiagnostic> {
        let analyzed = analyze_file_with_overlays(path, overlays, Some(target))?;
        check_analyzed(analyzed)
    }

    /// 按覆盖后的根元素自动选择 View 或 App 并检查 LSP 会话快照。
    pub fn check_file_with_overlays_auto(
        self,
        path: &Path,
        overlays: &BTreeMap<PathBuf, String>,
    ) -> Result<CheckOutput, CompilerDiagnostic> {
        let analyzed = analyze_file_with_overlays(path, overlays, None)?;
        check_analyzed(analyzed)
    }

    /// 格式化内嵌 UIX，同时验证 AST 等价。
    pub fn format_inline(
        self,
        source: &str,
        source_name: impl Into<String>,
    ) -> Result<FormatOutput, CompilerDiagnostic> {
        format_named_source(source, source_name.into())
    }

    /// 读取并格式化一个真实 `.uix` 文件，不直接覆盖磁盘。
    pub fn format_file(self, path: &Path) -> Result<FormatOutput, CompilerDiagnostic> {
        let canonical =
            fs::canonicalize(path).map_err(|error| source_io_diagnostic(path, error))?;
        let source = fs::read_to_string(&canonical)
            .map_err(|error| source_io_diagnostic(&canonical, error))?;
        format_named_source(&source, canonical.display().to_string())
    }

    /// 查询共享 UI 投影 schema，不允许 Adapter 维护副本。
    pub fn query(self, kind: QueryKind) -> QueryOutput {
        let entries = match kind {
            QueryKind::Components => self
                .schema
                .components()
                .iter()
                .map(QueryEntry::Component)
                .collect(),
            QueryKind::Attributes => self
                .schema
                .common_attributes()
                .iter()
                .map(QueryEntry::Attribute)
                .collect(),
            QueryKind::Events => self.schema.events().iter().map(QueryEntry::Event).collect(),
            QueryKind::Styles => self
                .schema
                .style_properties()
                .iter()
                .map(QueryEntry::Style)
                .collect(),
            QueryKind::Themes => self
                .schema
                .theme_tokens()
                .iter()
                .map(QueryEntry::Theme)
                .collect(),
            QueryKind::Handles => self
                .schema
                .handle_slots()
                .iter()
                .map(QueryEntry::Handle)
                .collect(),
            QueryKind::Slots => self.schema.slots().iter().map(QueryEntry::Slot).collect(),
            QueryKind::Capabilities => self
                .schema
                .capabilities()
                .iter()
                .map(QueryEntry::Capability)
                .collect(),
            QueryKind::CapabilityUses => self
                .schema
                .attribute_capabilities()
                .iter()
                .map(QueryEntry::AttributeCapability)
                .collect(),
            QueryKind::Data => self
                .schema
                .data_constructors()
                .iter()
                .map(QueryEntry::Data)
                .collect(),
        };
        QueryOutput { kind, entries }
    }
}

// 保存语义阶段成功后供 check 与 AOT 共享的不可变产物。
#[derive(Debug, Clone)]
pub(crate) struct AnalyzedUnit {
    tracked_files: Vec<PathBuf>,
    source_graph: source_graph::SourceGraph,
    ir: TypedUiIr,
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
    CompilerSystem::new().compile_inline(source, source_name, target)
}

/// 检查没有文件系统基准目录的内嵌 UIX 源码。
pub fn check_inline(
    source: &str,
    source_name: impl Into<String>,
    target: CompileTarget,
) -> Result<CheckOutput, CompilerDiagnostic> {
    CompilerSystem::new().check_inline(source, source_name, target)
}

// 建立内嵌 SourceGraph、AST 与 Typed UI IR。
fn analyze_inline(
    source: &str,
    source_name: String,
    requested_target: Option<CompileTarget>,
) -> Result<AnalyzedUnit, CompilerDiagnostic> {
    let source_graph = source_graph::SourceGraph::inline(&source_name, source);
    let source_id = source_graph.root();
    let document = if requested_target == Some(CompileTarget::Items) {
        parse_items_document(source)
    } else {
        parse_document(source)
    }
    .map_err(|diagnostic| {
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
    let target = requested_target.unwrap_or_else(|| inferred_target(&document));
    let declaration_sources = vec![source_id; document.declarations.len()];
    let ir =
        lower_document(document, target, source_id, &declaration_sources).map_err(|failure| {
            CompilerDiagnostic::from_language(
                &source_name,
                failure.source_id,
                DiagnosticPhase::Semantic,
                "UIX2000",
                failure.diagnostic,
            )
        })?;
    Ok(AnalyzedUnit {
        tracked_files: Vec::new(),
        source_graph,
        ir,
    })
}

/// 编译一个真实 `.uix` 根文件及其递归导入闭包。
pub fn compile_file(
    path: &Path,
    target: CompileTarget,
) -> Result<CompileOutput, CompilerDiagnostic> {
    CompilerSystem::new().compile_file(path, target)
}

/// 检查一个真实 `.uix` 根文件及其递归导入闭包。
pub fn check_file(path: &Path, target: CompileTarget) -> Result<CheckOutput, CompilerDiagnostic> {
    CompilerSystem::new().check_file(path, target)
}

/// 格式化内嵌 UIX，并返回无损 CST 与规范文本。
pub fn format_inline(
    source: &str,
    source_name: impl Into<String>,
) -> Result<FormatOutput, CompilerDiagnostic> {
    CompilerSystem::new().format_inline(source, source_name)
}

/// 读取并格式化一个真实 `.uix` 文件，不直接覆盖磁盘。
pub fn format_file(path: &Path) -> Result<FormatOutput, CompilerDiagnostic> {
    CompilerSystem::new().format_file(path)
}

/// 查询共享 UI 投影 schema。
pub fn query(kind: QueryKind) -> QueryOutput {
    CompilerSystem::new().query(kind)
}

fn format_named_source(
    source: &str,
    source_name: String,
) -> Result<FormatOutput, CompilerDiagnostic> {
    let source_id = SourceId::from_source_name(&source_name);
    let formatted = formatter::format_source(source, source_id).map_err(|failure| {
        CompilerDiagnostic::from_language(
            &source_name,
            source_id,
            if failure.invariant {
                DiagnosticPhase::Emit
            } else {
                DiagnosticPhase::Syntax
            },
            if failure.invariant {
                "UIX3001"
            } else {
                "UIX1000"
            },
            failure.diagnostic,
        )
    })?;
    Ok(FormatOutput {
        source_name,
        source_id,
        changed: formatted.formatted != source,
        formatted: formatted.formatted,
        cst: formatted.cst,
    })
}

fn source_io_diagnostic(path: &Path, error: io::Error) -> CompilerDiagnostic {
    let source_name = path.display().to_string();
    CompilerDiagnostic {
        code: "UIX0001",
        phase: DiagnosticPhase::Source,
        source_id: SourceId::from_source_name(&source_name),
        source_name,
        start: 0,
        end: 0,
        line: 1,
        column: 1,
        message: format!("无法读取 UIX 文件 {}：{error}", path.display()),
        suggestion: "确认文件存在、可读且为 UTF-8".to_string(),
    }
}

// 建立文件 SourceGraph、AST 与 Typed UI IR。
fn analyze_file(
    path: &Path,
    requested_target: Option<CompileTarget>,
) -> Result<AnalyzedUnit, CompilerDiagnostic> {
    let resolved = if requested_target == Some(CompileTarget::Items) {
        resolve_items_file(path)
    } else {
        resolve_file(path)
    }
    .map_err(CompilerDiagnostic::from_import)?;
    analyze_resolved_file(resolved, path, requested_target)
}

// 建立覆盖编辑器内存快照的文件 SourceGraph、AST 与 Typed UI IR。
fn analyze_file_with_overlays(
    path: &Path,
    overlays: &BTreeMap<PathBuf, String>,
    requested_target: Option<CompileTarget>,
) -> Result<AnalyzedUnit, CompilerDiagnostic> {
    let resolved = if requested_target == Some(CompileTarget::Items) {
        resolve_items_file_with_overlays(path, overlays)
    } else {
        resolve_file_with_overlays(path, overlays)
    }
    .map_err(CompilerDiagnostic::from_import)?;
    analyze_resolved_file(resolved, path, requested_target)
}

pub(crate) fn analyze_resolved_file(
    resolved: uix_import::ResolvedDocument,
    path: &Path,
    requested_target: Option<CompileTarget>,
) -> Result<AnalyzedUnit, CompilerDiagnostic> {
    let source_id = resolved.source_graph.root();
    let target = requested_target.unwrap_or_else(|| inferred_target(&resolved.document));
    let ir = lower_document(
        resolved.document,
        target,
        source_id,
        &resolved.declaration_sources,
    )
    .map_err(|failure| {
        semantic_diagnostic(
            &resolved.source_graph,
            path,
            failure.source_id,
            "UIX2000",
            failure.diagnostic,
        )
    })?;
    Ok(AnalyzedUnit {
        tracked_files: resolved.tracked_files,
        source_graph: resolved.source_graph,
        ir,
    })
}

// 自动入口只依据成功解析后的根元素，不依赖诊断文案或二次编译。
pub(crate) fn inferred_target(document: &uix_lang::Document) -> CompileTarget {
    if document.root.name == "App" {
        CompileTarget::App
    } else {
        CompileTarget::View
    }
}

// 让 AOT 与 check 执行同一完整 lowering Gate，AOT 额外进入 Rust Emitter。
pub(crate) fn compile_analyzed(
    analyzed: AnalyzedUnit,
) -> Result<CompileOutput, CompilerDiagnostic> {
    let plan = lower_analyzed(&analyzed)?;
    compile_lowered(analyzed, plan)
}

// 把共享 lowering Gate 投影为可被会话缓存的稳定结果。
pub(crate) fn lower_analyzed(analyzed: &AnalyzedUnit) -> Result<RustUiPlan, CompilerDiagnostic> {
    lower_rust_plan(&analyzed.ir).map_err(|failure| {
        semantic_diagnostic(
            &analyzed.source_graph,
            Path::new("<unknown>"),
            failure.source_id,
            failure.code,
            failure.diagnostic,
        )
    })
}

// 让会话在复用 lowering 计划后只执行真正的 Rust Emit 阶段。
pub(crate) fn compile_lowered(
    analyzed: AnalyzedUnit,
    plan: RustUiPlan,
) -> Result<CompileOutput, CompilerDiagnostic> {
    let compilation_key = CompilationKey::new(&analyzed.source_graph, analyzed.ir.target());
    let emitted = RustEmitter::emit(plan, &analyzed.source_graph, &analyzed.ir)
        .map_err(|message| emit_diagnostic(&analyzed.source_graph, message))?;
    Ok(CompileOutput {
        tokens: emitted.tokens,
        tracked_files: analyzed.tracked_files,
        source_graph: analyzed.source_graph,
        ir: analyzed.ir,
        source_map: emitted.source_map,
        compilation_key,
    })
}

// 检查阶段执行完整 lowering Gate，但不进入 Rust Emitter。
pub(crate) fn check_analyzed(analyzed: AnalyzedUnit) -> Result<CheckOutput, CompilerDiagnostic> {
    lower_analyzed(&analyzed)?;
    Ok(check_lowered(analyzed))
}

// 让会话在确认共享 lowering 成功后构造不含 Rust Emit 的检查结果。
pub(crate) fn check_lowered(analyzed: AnalyzedUnit) -> CheckOutput {
    let compilation_key = CompilationKey::new(&analyzed.source_graph, analyzed.ir.target());
    CheckOutput {
        tracked_files: analyzed.tracked_files,
        source_graph: analyzed.source_graph,
        ir: analyzed.ir,
        compilation_key,
    }
}

// 把 lowering 错误绑定到本次源码图中的真实节点来源。
fn semantic_diagnostic(
    source_graph: &source_graph::SourceGraph,
    fallback: &Path,
    source_id: SourceId,
    code: &'static str,
    diagnostic: Diagnostic,
) -> CompilerDiagnostic {
    let source_name = source_graph
        .file(source_id)
        .map(|file| file.path.clone())
        .unwrap_or_else(|| fallback.display().to_string());
    CompilerDiagnostic::from_language(
        source_name,
        source_id,
        DiagnosticPhase::Semantic,
        code,
        diagnostic,
    )
}

// 把 Rust Emitter 的内部不变量失败固定到当前根源码，避免向 Adapter 泄漏解析错误。
fn emit_diagnostic(
    source_graph: &source_graph::SourceGraph,
    message: String,
) -> CompilerDiagnostic {
    let source_id = source_graph.root();
    let source_name = source_graph
        .file(source_id)
        .map(|file| file.path.clone())
        .unwrap_or_else(|| "<unknown>".to_string());
    CompilerDiagnostic::from_language(
        source_name,
        source_id,
        DiagnosticPhase::Emit,
        "UIX3000",
        Diagnostic::new(
            SourceSpan {
                start: 0,
                end: 0,
                line: 1,
                column: 1,
            },
            message,
            "报告 uix-lang 编译器内部生成错误",
        ),
    )
}

// 保存已通过全部语义 Gate、可直接交付 Rust Emitter 的不可变计划。
#[derive(Debug, Clone)]
pub(crate) struct RustUiPlan {
    tokens: TokenStream,
}

struct LoweringDiagnostic {
    code: &'static str,
    source_id: SourceId,
    diagnostic: Diagnostic,
}

// 把类型化 UI IR 降低为 Rust UI 计划；check 与 AOT 必须共同执行本阶段。
fn lower_rust_plan(ir: &TypedUiIr) -> Result<RustUiPlan, LoweringDiagnostic> {
    if let Some(requirement) = ir
        .capability_requirements()
        .into_iter()
        .find(|requirement| !capability_enabled(requirement.capability))
    {
        let subject = requirement
            .attribute
            .as_deref()
            .map(|attribute| format!("组件 {} 的属性 {attribute}", requirement.component))
            .unwrap_or_else(|| format!("组件 {}", requirement.component));
        return Err(LoweringDiagnostic {
            code: "UIX2001",
            source_id: requirement.span.source_id,
            diagnostic: Diagnostic::new(
                SourceSpan {
                    start: requirement.span.start,
                    end: requirement.span.end,
                    line: requirement.span.line,
                    column: requirement.span.column,
                },
                format!("{subject} 需要 capability {}", requirement.capability),
                format!("在 uix 依赖上启用 Cargo feature {}", requirement.capability),
            ),
        });
    }
    let document = ir.emission_document();
    let root_source = ir.root().span.source_id;
    let widget_sources = ir.widget_source_ids();
    let record_sources = ir.record_source_ids();
    let visual_sources = ir.visual_source_ids();
    let tokens = match ir.target() {
        CompileTarget::View => with_source_markers(
            root_source,
            widget_sources,
            record_sources,
            visual_sources,
            || generate_document_view(&document),
        ),
        CompileTarget::App => with_source_markers(
            root_source,
            widget_sources,
            record_sources,
            visual_sources,
            || generate_document_app(&document),
        ),
        CompileTarget::Items => with_source_markers(
            root_source,
            widget_sources,
            record_sources,
            visual_sources,
            || generate_document_items(&document),
        ),
    }
    .map_err(|diagnostic| {
        let source_id = diagnostic.source_id.unwrap_or(ir.root().span.source_id);
        LoweringDiagnostic {
            code: "UIX2000",
            source_id,
            diagnostic,
        }
    })?;
    Ok(RustUiPlan { tokens })
}

// 只负责把已验证计划物化为公开 Rust 输出及 SourceMap。
struct RustEmitter;

struct EmittedRust {
    tokens: TokenStream,
    source_map: SourceMap,
}

impl RustEmitter {
    fn emit(
        plan: RustUiPlan,
        source_graph: &source_graph::SourceGraph,
        ir: &TypedUiIr,
    ) -> Result<EmittedRust, String> {
        let marked = plan.tokens.to_string();
        let mapped = SourceMap::from_marked(&marked, source_graph, ir);
        let tokens = mapped
            .source
            .parse::<TokenStream>()
            .map_err(|error| format!("无法解析已清理来源标记的 Rust 输出：{error}"))?;
        let normalized = tokens.to_string();
        let mut source_map = mapped.source_map;
        source_map.normalize_generated_offsets(&mapped.source, &normalized)?;
        Ok(EmittedRust { tokens, source_map })
    }
}

fn enabled_capabilities() -> Vec<&'static str> {
    UI_PROJECTION_SCHEMA
        .capabilities()
        .iter()
        .filter(|capability| capability_enabled(capability.name))
        .map(|capability| capability.name)
        .collect()
}

fn capability_enabled(name: &str) -> bool {
    match name {
        "image-codecs" => cfg!(feature = "image-codecs"),
        "qrcode" => cfg!(feature = "qrcode"),
        "form-pattern" => cfg!(feature = "form-pattern"),
        "rich-text" => cfg!(feature = "rich-text"),
        "charts" => cfg!(feature = "charts"),
        "table" => cfg!(feature = "table"),
        "navigation" => cfg!(feature = "navigation"),
        "feedback" => cfg!(feature = "feedback"),
        "tree-widgets" => cfg!(feature = "tree-widgets"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        CompileTarget, CompilerSystem, DiagnosticPhase, QueryEntry, QueryKind, check_inline,
        compile_inline, format_inline,
    };
    use crate::source_graph::SourceId;

    #[test]
    fn shared_commands_compile_all_public_target_shapes() {
        let view = compile_inline("<Text>Hello</Text>", "<view>", CompileTarget::View)
            .expect("View 编译应成功");
        assert!(view.tokens.to_string().contains("label"));
        assert!(!view.tokens.to_string().contains("__uix_source_marker"));
        assert!(view.tracked_files.is_empty());
        assert!(!view.source_map.entries().is_empty());
        assert_eq!(
            view.source_map.entries()[0].source_id,
            view.source_graph.root()
        );
        assert_eq!(
            view.compilation_key.root_content_hash,
            view.source_graph
                .file(view.source_graph.root())
                .expect("根源码必须存在")
                .content_hash
        );

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
    fn items_target_accepts_declaration_only_resources_without_relaxing_views() {
        let source = r#"<Visual name="ONLY_VISUAL" type="OnlyVisual" value={1.0} />"#;
        let inline = compile_inline(source, "<visual-items>", CompileTarget::Items)
            .expect("Items 内嵌资源应允许只有模块级声明");
        assert!(inline.tokens.to_string().contains("ONLY_VISUAL"));

        let strict = compile_inline(source, "<visual-view>", CompileTarget::View)
            .expect_err("View 入口仍必须声明根元素");
        assert_eq!(strict.code, "UIX1000");
        assert!(strict.message.contains("缺少根元素"));

        let file = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/uix_lang/items/visual_only.uix");
        let output = CompilerSystem::new()
            .compile_file(&file, CompileTarget::Items)
            .expect("Items 文件资源应允许只有模块级声明");
        assert!(output.tokens.to_string().contains("ONLY_VISUAL"));
        let strict_file = CompilerSystem::new()
            .compile_file(&file, CompileTarget::View)
            .expect_err("同一文件作为 View 时仍必须拒绝缺失根元素");
        assert_eq!(strict_file.code, "UIX1000");
    }

    #[test]
    fn auto_check_selects_target_from_parsed_root() {
        let system = CompilerSystem::new();
        let view = system
            .check_inline_auto("<Text>Hello</Text>", "<view-auto>")
            .expect("普通根应自动选择 View");
        assert_eq!(view.ir.target(), CompileTarget::View);

        let app = system
            .check_inline_auto("<App title=\"Auto\"><Text>Hello</Text></App>", "<app-auto>")
            .expect("App 根应自动选择 App");
        assert_eq!(app.ir.target(), CompileTarget::App);
    }

    #[test]
    fn source_map_preserves_recursive_import_sources() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/uix_lang/imports/root.uix");
        let output = CompilerSystem::new()
            .compile_file(&root, CompileTarget::View)
            .expect("递归导入闭包必须可编译");
        let sources = output
            .source_map
            .entries()
            .iter()
            .map(|entry| entry.source_name.as_str())
            .collect::<Vec<_>>();
        assert!(
            sources
                .iter()
                .any(|source| source.ends_with("pages/page.uix")),
            "SourceMap 必须保留直接导入来源：{sources:?}"
        );
        assert!(
            sources
                .iter()
                .any(|source| source.ends_with("shared/helper.uix")),
            "SourceMap 必须保留递归导入来源：{sources:?}"
        );
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

    #[test]
    fn check_and_aot_share_semantic_diagnostic_contract() {
        let source = "<Mystery />";
        let aot = compile_inline(source, "same.uix", CompileTarget::View)
            .expect_err("AOT 必须拒绝未知标签");
        let check = check_inline(source, "same.uix", CompileTarget::View)
            .expect_err("check 必须拒绝未知标签");
        assert_eq!(aot, check);
    }

    #[test]
    fn compiler_system_owns_schema_and_returns_typed_ir() {
        let system = CompilerSystem::new();
        assert_eq!(system.schema().version(), 3);
        let checked = system
            .check_inline("<Text>Hello</Text>", "typed.uix", CompileTarget::View)
            .expect("合法文档必须通过检查");
        assert_eq!(checked.ir.root().name, "Text");
        assert_eq!(
            checked.ir.root().span.source_id,
            checked.source_graph.root()
        );
    }

    #[test]
    fn shared_formatter_returns_lossless_idempotent_result() {
        let first = format_inline("<Column><Text>A</Text></Column>", "fmt.uix")
            .expect("合法源码必须可格式化");
        assert!(first.changed);
        assert_eq!(first.cst.reconstruct(), first.formatted);
        let second =
            format_inline(&first.formatted, "fmt.uix").expect("格式化结果必须可再次格式化");
        assert!(!second.changed);
        assert_eq!(second.formatted, first.formatted);
    }

    #[test]
    fn compiler_system_query_and_incremental_key_share_schema_versions() {
        let system = CompilerSystem::new();
        let queried = system.query(QueryKind::Components);
        assert_eq!(queried.kind.as_str(), "components");
        assert!(matches!(queried.entries[0], QueryEntry::Component(_)));

        let first = system
            .check_inline("<Text>A</Text>", "key.uix", CompileTarget::View)
            .expect("合法源码必须可检查");
        let changed = system
            .check_inline("<Text>B</Text>", "key.uix", CompileTarget::View)
            .expect("变更源码必须可检查");
        assert_ne!(first.compilation_key, changed.compilation_key);
        assert_eq!(
            first.compilation_key.schema_version,
            system.schema().version()
        );
        assert_eq!(
            first.compilation_key.language_version,
            system.schema().language_version()
        );
    }

    #[cfg(not(feature = "qrcode"))]
    #[test]
    fn missing_capability_is_a_shared_semantic_diagnostic() {
        let source = "<QRCode value=\"hello\" />";
        let aot = compile_inline(source, "capability.uix", CompileTarget::View)
            .expect_err("未启用 qrcode 时 AOT 必须拒绝组件");
        let check = check_inline(source, "capability.uix", CompileTarget::View)
            .expect_err("未启用 qrcode 时 check 必须拒绝组件");
        assert_eq!(aot, check);
        assert_eq!(aot.code, "UIX2001");
        assert!(aot.message.contains("qrcode"));
    }

    #[cfg(not(feature = "image-codecs"))]
    #[test]
    fn attribute_capability_does_not_reject_unrelated_component_shape() {
        check_inline("<Avatar text=\"UI\" />", "avatar.uix", CompileTarget::View)
            .expect("纯文字 Avatar 不依赖 image-codecs");
        let error = check_inline(
            "<Avatar src=\"avatar.png\" />",
            "avatar.uix",
            CompileTarget::View,
        )
        .expect_err("Avatar src 必须要求 image-codecs");
        assert_eq!(error.code, "UIX2001");
        assert!(error.message.contains("src"));
    }

    #[test]
    fn overlay_imports_preserve_unsaved_content_and_source_identity() {
        let directory = std::env::temp_dir().join(format!(
            "uix-lang-overlay-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("系统时间必须有效")
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).expect("临时目录必须可创建");
        let root = directory.join("root.uix");
        let card = directory.join("card.uix");
        std::fs::write(&root, "@import('./card.uix', 'Card')\n<Card />").expect("根文件必须可写");
        std::fs::write(
            &card,
            "@export('Card')\n<Widget name=\"Card\"><Text>Disk</Text></Widget>\n<Text />",
        )
        .expect("导入文件必须可写");
        let overlays = std::collections::BTreeMap::from([(
            card.clone(),
            "@export('Card')\n<Widget name=\"Card\"><Mystery /></Widget>\n<Text />".to_string(),
        )]);
        let error = CompilerSystem::new()
            .check_file_with_overlays(&root, &overlays, CompileTarget::View)
            .expect_err("未保存的未知标签必须产生诊断");
        assert!(error.message.contains("Mystery"));
        assert_eq!(
            error.source_name,
            std::fs::canonicalize(&card)
                .expect("导入文件必须可规范化")
                .display()
                .to_string()
        );
        std::fs::remove_dir_all(&directory).expect("临时目录必须可清理");
    }
}

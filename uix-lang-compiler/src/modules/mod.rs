//! 可移植应用模块前端。原有 UIX parser 负责标签、属性、表达式和 do 块；
//! 本模块只拥有应用模块的名称、类型、效果与执行产物，不拥有运行实例。

mod check;
mod expression;
mod emit;
mod view;
use crate::{CompilerDiagnostic, DiagnosticPhase, source_graph::{SourceGraph, SourceId}, uix_lang::*};
use std::{collections::BTreeMap, path::Path};
use uix_lang_runtime::*;

/// 一次共享分析结果；运行期使用 module，工具消费来源图与声明位置。
#[derive(Debug, Clone)]
pub struct ModuleOutput {
    pub module: Module,
    pub source_graph: SourceGraph,
    pub symbols: Vec<ModuleSymbol>,
}

#[derive(Debug, Clone)]
pub struct ModuleSymbol {
    pub name: String,
    pub detail: String,
    pub start: usize,
    pub end: usize,
}

/// 读取明确路径；不扫描、监听或隐式寻找来源。
pub fn check_file(path: &Path) -> Result<ModuleOutput, CompilerDiagnostic> {
    let source = std::fs::read_to_string(path).map_err(|e| crate::source_io_diagnostic(path, e))?;
    check_inline(&source, &path.display().to_string())
}

/// 分析与冻结可移植模块；源码只在本入口处理一次。
pub fn check_inline(source: &str, name: &str) -> Result<ModuleOutput, CompilerDiagnostic> {
    let graph = SourceGraph::inline(name, source);
    let diagnostic = |d, phase| CompilerDiagnostic::from_language(name,
        SourceId::from_source_name(name), phase, if phase == DiagnosticPhase::Syntax { "UIX1000" } else { "UIX2100" }, d);
    // 在递归语法解析前限制输入与嵌套，避免来自文件的堆栈耗尽。
    preflight(source).map_err(|d| diagnostic(d, DiagnosticPhase::Syntax))?;
    let document = parse_items_document(source).map_err(|d| diagnostic(d, DiagnosticPhase::Syntax))?;
    let (module, symbols) = check::module(&document, name)
        .map_err(|d| diagnostic(d, DiagnosticPhase::Semantic))?;
    Ok(ModuleOutput { module, source_graph: graph, symbols })
}

/// 生成真实 Rust 函数；不会把源码交给运行期解析器或动态语句解释器。
pub fn compile_inline(source: &str, name: &str) -> Result<proc_macro2::TokenStream, CompilerDiagnostic> {
    let output = check_inline(source, name)?;
    Ok(emit::module(&output.module))
}

pub fn compile_file(path: &Path) -> Result<proc_macro2::TokenStream, CompilerDiagnostic> {
    let output = check_file(path)?;
    Ok(emit::module(&output.module))
}

fn fail(span: SourceSpan, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(span, message, "按可移植应用模块规范修正声明、类型或效果")
}
fn location(name: &str, span: SourceSpan) -> Location {
    Location { source: name.to_string(), line: span.line, column: span.column }
}
fn attr<'a>(element: &'a Element, name: &str) -> Result<Option<&'a str>, Diagnostic> {
    match element.attributes.iter().find(|a| a.name == name) {
        None => Ok(None),
        Some(Attribute { value: AttributeValue::Literal(v), .. }) => Ok(Some(v)),
        Some(a) => Err(fail(a.span, format!("{name} 必须是字符串字面量"))),
    }
}
fn required<'a>(element: &'a Element, name: &str) -> Result<&'a str, Diagnostic> {
    attr(element, name)?.ok_or_else(|| fail(element.span, format!("{} 缺少 {name}", element.name)))
}
fn attributes(element: &Element, allowed: &[&str]) -> Result<(), Diagnostic> {
    let mut names = std::collections::BTreeSet::new();
    for a in &element.attributes {
        if !allowed.contains(&a.name.as_str()) || !names.insert(&a.name) {
            return Err(fail(a.span, format!("未知或重复属性 {}", a.name)));
        }
    }
    Ok(())
}
fn identifier(name: &str, span: SourceSpan) -> Result<(), Diagnostic> {
    if name.is_empty() || !name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        || name.starts_with("__uix_")
        || matches!(name, "setState" | "Some" | "None" | "Ok" | "Err") {
        Err(fail(span, format!("非法或保留名称 {name}")))
    } else { Ok(()) }
}

fn preflight(source: &str) -> Result<(), Diagnostic> {
    let span = SourceSpan { start: 0, end: 0, line: 1, column: 1 };
    if source.len() > 1_048_576 { return Err(fail(span, "模块源码超过 1 MiB")); }
    // 此保守上限覆盖标签、表达式与 do 块；字符串的定界符计入上限但不解释。
    let mut depth = 0usize;
    let mut quote = None;
    let mut escape = false;
    for c in source.chars() {
        if escape { escape = false; continue; }
        if quote.is_some() && c == '\\' { escape = true; continue; }
        if quote == Some(c) { quote = None; continue; }
        if quote.is_some() { continue; }
        if c == '\'' { quote = Some(c); continue; }
        if matches!(c, '(' | '[' | '{') { depth += 1; }
        if matches!(c, ')' | ']' | '}') { depth = depth.saturating_sub(1); }
        if depth > 64 { return Err(fail(span, "模块表达式嵌套超过 64 层")); }
    }
    // 解析器按标签递归；限制总标签数同时给出严格的栈深度上界。
    if source.matches('<').count() > 128 { return Err(fail(span, "模块标签与比较标记总数超过 128")); }
    Ok(())
}

fn parse_type(source: &str, records: &BTreeMap<String, Type>, span: SourceSpan) -> Result<Type, Diagnostic> {
    let source = source.trim();
    let simple = match source {
        "Unit" => Some(Type::Unit), "Bool" | "bool" => Some(Type::Bool),
        "Int" | "i64" => Some(Type::Int), "Float" | "f64" | "Number" => Some(Type::Float),
        "String" => Some(Type::String), "Bytes" => Some(Type::Bytes), _ => records.get(source).cloned(),
    };
    if let Some(ty) = simple { return Ok(ty); }
    for prefix in ["Array<", "Option<", "Result<"] {
        if let Some(inner) = source.strip_prefix(prefix).and_then(|s| s.strip_suffix('>')) {
            if source.matches('<').count() > 32 { return Err(fail(span, "类型嵌套过深")); }
            let pieces = split_types(inner);
            return match (prefix, pieces.len()) {
                ("Array<", 1) => Ok(Type::Array(Box::new(parse_type(pieces[0], records, span)?))),
                ("Option<", 1) => Ok(Type::Optional(Box::new(parse_type(pieces[0], records, span)?))),
                ("Result<", 2) => Ok(Type::Result(Box::new(parse_type(pieces[0], records, span)?), Box::new(parse_type(pieces[1], records, span)?))),
                _ => Err(fail(span, "类型参数数量无效")),
            };
        }
    }
    Err(fail(span, format!("动态绑定不支持类型 {source}")))
}
fn split_types(source: &str) -> Vec<&str> {
    let mut start = 0;
    let mut depth = 0usize;
    let mut parts = Vec::new();
    for (i, c) in source.char_indices() {
        match c { '<' => depth += 1, '>' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => { parts.push(source[start..i].trim()); start = i + 1; }, _ => {} }
    }
    if !source[start..].trim().is_empty() { parts.push(source[start..].trim()); }
    parts
}

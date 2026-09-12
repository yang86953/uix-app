//! 纯组件文件的导出与源码闭包。原生导入仅记录待绑定符号，不伪称已验签。

use super::*;
use crate::lang::compiler::{
    DiagnosticPhase,
    source_graph::{SourceGraphBuilder, SourceId},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_FILE_BYTES: usize = 1_048_576;
const MAX_GRAPH_BYTES: usize = 8 * MAX_FILE_BYTES;
const MAX_FILES: usize = 256;
const MAX_IMPORT_DEPTH: usize = 64;

/// 声明身份不依赖导入别名或合并后的全局字符串名。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeclarationId {
    pub source_id: SourceId,
    pub index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binding {
    Declaration(DeclarationId),
    /// 必须由后续共同类型检查匹配宿主导出签名；此时不表示库已存在或可调用。
    NativeImport {
        package: String,
        name: String,
    },
}

#[derive(Debug, Clone)]
pub struct SourceUnit {
    pub source_id: SourceId,
    pub imports: Vec<Import>,
    pub declarations: Vec<Declaration>,
    /// 文件词法顶层作用域；不会把依赖的私有声明暴露给调用方。
    pub bindings: BTreeMap<String, Binding>,
}

/// 已连接源码文件与导出，不含类型检查结论或运行实例。
#[derive(Debug, Clone)]
pub struct LinkedSource {
    pub source_graph: SourceGraph,
    pub units: BTreeMap<SourceId, SourceUnit>,
}

/// 从真实文件连接纯 UIX 导入；裸包导入留给原生签名绑定。
pub fn link_file(entry: &Path) -> Result<LinkedSource, CompilerDiagnostic> {
    link_file_with_overlays(entry, &BTreeMap::new())
}

/// 使用编辑器缓冲区替换对应文件。新文件允许尚未保存，但父目录须可定位。
/// 检查与生成消费返回的同一源码快照，不在后续阶段重新读取磁盘。
pub fn link_file_with_overlays(
    entry: &Path,
    overlays: &BTreeMap<PathBuf, String>,
) -> Result<LinkedSource, CompilerDiagnostic> {
    let root = canonical_path(entry).map_err(|message| source_error(entry, message))?;
    let mut normalized = BTreeMap::new();
    let mut overlay_bytes = 0usize;
    for (path, source) in overlays {
        overlay_bytes = overlay_bytes.saturating_add(source.len());
        if overlays.len() > MAX_FILES
            || overlay_bytes > MAX_GRAPH_BYTES
            || source.len() > MAX_FILE_BYTES
        {
            return Err(source_error(path, "编辑器源码缓冲区超过组件闭包上限"));
        }
        let path = canonical_path(path).map_err(|message| source_error(path, message))?;
        if let Some(previous) = normalized.insert(path.clone(), source.as_str()) {
            if previous != source {
                return Err(source_error(&path, "同一真实文件有相互冲突的编辑器缓冲区"));
            }
        }
    }
    let mut linker = Linker {
        graph: SourceGraphBuilder::new(&root),
        units: BTreeMap::new(),
        visiting: BTreeSet::new(),
        normalized,
        bytes: 0,
        files: 0,
    };
    linker.load(&root)?;
    Ok(LinkedSource {
        source_graph: linker.graph.finish(),
        units: linker.units,
    })
}

fn canonical_path(path: &Path) -> Result<PathBuf, String> {
    match std::fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(error) => {
            let parent = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let Some(name) = path.file_name() else {
                return Err(error.to_string());
            };
            std::fs::canonicalize(parent)
                .map(|parent| parent.join(name))
                .map_err(|_| error.to_string())
        }
    }
}

fn source_error(path: &Path, message: impl Into<String>) -> CompilerDiagnostic {
    diagnostic(
        &path.to_string_lossy(),
        "",
        Span { start: 0, end: 0 },
        DiagnosticPhase::Source,
        "component-source",
        message,
    )
}

fn diagnostic(
    source_name: &str,
    source: &str,
    span: Span,
    phase: DiagnosticPhase,
    code: &'static str,
    message: impl Into<String>,
) -> CompilerDiagnostic {
    let prefix = &source[..span.start];
    CompilerDiagnostic {
        code,
        phase,
        source_id: SourceId::from_source_name(source_name),
        source_name: source_name.into(),
        start: span.start,
        end: span.end,
        line: prefix.bytes().filter(|byte| *byte == b'\n').count() + 1,
        column: prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1,
        message: message.into(),
        suggestion: "检查此导入的路径、导出名和本文件作用域；纯 UIX 组件不需要另写登记表".into(),
    }
}

struct Linker<'a> {
    graph: SourceGraphBuilder,
    units: BTreeMap<SourceId, SourceUnit>,
    visiting: BTreeSet<SourceId>,
    normalized: BTreeMap<PathBuf, &'a str>,
    bytes: usize,
    files: usize,
}

impl Linker<'_> {
    fn load(&mut self, path: &Path) -> Result<SourceId, CompilerDiagnostic> {
        let source_name = path
            .to_str()
            .ok_or_else(|| source_error(path, "组件源码路径需要有效 UTF-8"))?;
        let source_id = SourceId::from_source_name(source_name);
        if self.units.contains_key(&source_id) {
            return Ok(source_id);
        }
        if self.files >= MAX_FILES || self.visiting.len() >= MAX_IMPORT_DEPTH {
            return Err(source_error(path, "组件导入文件数量或深度超过上限"));
        }
        if !self.visiting.insert(source_id) {
            return Err(source_error(path, "组件源码出现循环导入"));
        }
        let source = if let Some(source) = self.normalized.get(path) {
            (*source).to_owned()
        } else {
            let mut source = String::new();
            File::open(path)
                .and_then(|file| {
                    file.take(MAX_FILE_BYTES as u64 + 1)
                        .read_to_string(&mut source)
                })
                .map_err(|error| source_error(path, error.to_string()))?;
            source
        };
        self.bytes += source.len();
        self.files += 1;
        if self.bytes > MAX_GRAPH_BYTES {
            return Err(source_error(path, "组件源码闭包超过 8 MiB"));
        }
        let ParsedSource {
            imports,
            declarations,
            ..
        } = parse(&source, source_name)?;
        self.graph.insert_file(path, &source);
        let error = |span, code, message| {
            diagnostic(
                source_name,
                &source,
                span,
                DiagnosticPhase::Import,
                code,
                message,
            )
        };
        let mut bindings = BTreeMap::new();
        for (index, declaration) in declarations.iter().enumerate() {
            if bindings
                .insert(
                    declaration.name.text.clone(),
                    Binding::Declaration(DeclarationId { source_id, index }),
                )
                .is_some()
            {
                return Err(error(
                    declaration.name.span,
                    "component-duplicate-name",
                    format!("重复的顶层名称 {}", declaration.name.text),
                ));
            }
        }
        for import in &imports {
            let relative = import.source.starts_with("./") || import.source.starts_with("../");
            let dependency = if relative {
                let requested = path.parent().unwrap().join(&import.source);
                if requested
                    .extension()
                    .is_none_or(|extension| extension != "uix")
                {
                    return Err(error(
                        import.source_span,
                        "component-import-path",
                        "纯组件导入必须明确指向 .uix 文件".into(),
                    ));
                }
                let dependency_path = canonical_path(&requested).map_err(|message| {
                    error(import.source_span, "component-import-path", message)
                })?;
                let dependency_id = SourceId::from_source_name(&dependency_path.to_string_lossy());
                if self.visiting.contains(&dependency_id) {
                    return Err(error(
                        import.source_span,
                        "component-import-cycle",
                        "组件源码出现循环导入".into(),
                    ));
                }
                let imported_id = self.load(&dependency_path).map_err(|failure| {
                    // 文件不可读指回引用处；依赖内的语法/导出错误保留依赖原位置。
                    if failure.phase == DiagnosticPhase::Source {
                        error(
                            import.source_span,
                            "component-import-source",
                            format!(
                                "无法加载 {}：{}",
                                dependency_path.display(),
                                failure.message
                            ),
                        )
                    } else {
                        failure
                    }
                })?;
                let position = error(import.source_span, "component-import", String::new());
                self.graph
                    .add_import(path, &dependency_path, None, position.line, position.column);
                Some(imported_id)
            } else {
                if import.source.is_empty()
                    || import.source.starts_with(['/', '.', '\\'])
                    || import.source.contains(['\\', ':'])
                    || import.source.chars().any(char::is_whitespace)
                {
                    return Err(error(
                        import.source_span,
                        "component-import-path",
                        "导入需要相对 .uix 路径或明确的原生包名".into(),
                    ));
                }
                None
            };
            for name in &import.names {
                let binding = if let Some(dependency) = dependency {
                    let unit = &self.units[&dependency];
                    let Some((index, _)) =
                        unit.declarations
                            .iter()
                            .enumerate()
                            .find(|(_, declaration)| {
                                declaration.exported && declaration.name.text == name.imported.text
                            })
                    else {
                        return Err(error(
                            name.imported.span,
                            "component-import-export",
                            format!("{} 没有导出 {}", import.source, name.imported.text),
                        ));
                    };
                    Binding::Declaration(DeclarationId {
                        source_id: dependency,
                        index,
                    })
                } else {
                    Binding::NativeImport {
                        package: import.source.clone(),
                        name: name.imported.text.clone(),
                    }
                };
                if bindings.insert(name.local.text.clone(), binding).is_some() {
                    return Err(error(
                        name.local.span,
                        "component-duplicate-name",
                        format!("导入名称 {} 与本文件名称冲突", name.local.text),
                    ));
                }
            }
        }
        self.visiting.remove(&source_id);
        self.units.insert(
            source_id,
            SourceUnit {
                source_id,
                imports,
                declarations,
                bindings,
            },
        );
        Ok(source_id)
    }
}

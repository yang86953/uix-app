//! 明确文件入口的有界依赖读取；每次分析拥有独立且冻结的来源闭包。

use super::*;
use crate::source_graph::SourceGraphBuilder;
use std::{collections::BTreeSet, io::Read, path::PathBuf};

pub(super) fn file(
    path: &Path,
    overlays: &BTreeMap<PathBuf, String>,
) -> Result<ModuleOutput, CompilerDiagnostic> {
    let root = canonical(path).map_err(|e| crate::source_io_diagnostic(path, e))?;
    let package = root.parent().unwrap_or(Path::new("/")).to_path_buf();
    let overlays = overlays
        .iter()
        .filter_map(|(path, source)| canonical(path).ok().map(|path| (path, source.as_str())))
        .collect();
    let mut loader = Loader {
        package,
        overlays,
        graph: SourceGraphBuilder::new(&root),
        active: BTreeSet::new(),
        cache: BTreeMap::new(),
        bytes: 0,
    };
    let (module, symbols) = loader.load(&root, 0)?;
    Ok(ModuleOutput {
        module,
        symbols,
        source_graph: loader.graph.finish(),
    })
}

// 未保存的编辑器文件允许覆盖；其父目录仍必须可规范化且处于明确包边界。
fn canonical(path: &Path) -> std::io::Result<PathBuf> {
    match path.canonicalize() {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            Ok(parent.canonicalize()?.join(path.file_name().ok_or(error)?))
        }
        Err(error) => Err(error),
    }
}

struct Loader<'a> {
    package: PathBuf,
    overlays: BTreeMap<PathBuf, &'a str>,
    graph: SourceGraphBuilder,
    active: BTreeSet<PathBuf>,
    cache: BTreeMap<PathBuf, (Module, Vec<ModuleSymbol>)>,
    bytes: usize,
}
impl Loader<'_> {
    fn load(
        &mut self,
        path: &Path,
        depth: usize,
    ) -> Result<(Module, Vec<ModuleSymbol>), CompilerDiagnostic> {
        let name = path.to_str().ok_or_else(|| {
            crate::source_io_diagnostic(
                path,
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "模块文件路径必须是 UTF-8"),
            )
        })?;
        let initial = SourceSpan {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        };
        let error = |span, message: String| {
            diagnostic(&name, DiagnosticPhase::Semantic, fail(span, message))
        };
        if depth > 16 || self.active.len() + self.cache.len() >= 64 {
            return Err(error(initial, "依赖闭包超过 64 文件或 16 层".into()));
        }
        if !self.active.insert(path.to_path_buf()) {
            return Err(error(initial, "模块导入循环".into()));
        }
        if let Some(output) = self.cache.get(path) {
            self.active.remove(path);
            return Ok(output.clone());
        }
        let source = if let Some(source) = self.overlays.get(path) {
            (*source).to_string()
        } else {
            let file =
                std::fs::File::open(path).map_err(|e| crate::source_io_diagnostic(path, e))?;
            let mut source = String::new();
            file.take(1_048_577)
                .read_to_string(&mut source)
                .map_err(|e| crate::source_io_diagnostic(path, e))?;
            source
        };
        self.bytes += source.len();
        if self.bytes > 4 * 1_048_576 {
            return Err(error(initial, "依赖源码总量超过 4 MiB".into()));
        }
        preflight(&source).map_err(|d| diagnostic(&name, DiagnosticPhase::Syntax, d))?;
        let document = parse_items_document(&source)
            .map_err(|d| diagnostic(&name, DiagnosticPhase::Syntax, d))?;
        self.graph.insert_file(path, &source);
        let mut imports = Vec::new();
        let mut dependency_symbols = Vec::new();
        let mut aliases = BTreeSet::new();
        for element in document.root.children.iter().filter_map(|node| match node {
            Node::Element(element) if element.name == "Import" => Some(element),
            _ => None,
        }) {
            let semantic = |d| diagnostic(&name, DiagnosticPhase::Semantic, d);
            attributes(element, &["from", "as", "version"]).map_err(semantic)?;
            if !element
                .children
                .iter()
                .all(|node| matches!(node, Node::Text(t) if t.value.trim().is_empty()))
            {
                return Err(error(element.span, "Import 不允许主体".into()));
            }
            let from = required(element, "from").map_err(semantic)?;
            let alias = required(element, "as").map_err(semantic)?;
            let version = required(element, "version").map_err(semantic)?;
            identifier(alias, element.span).map_err(semantic)?;
            if !aliases.insert(alias.to_string()) {
                return Err(error(element.span, "重复导入别名".into()));
            }
            if from.is_empty()
                || Path::new(from).is_absolute()
                || from.contains([':', '\\'])
                || !from.ends_with(".uix")
            {
                return Err(error(
                    element.span,
                    "Import from 必须是包内相对 .uix 路径".into(),
                ));
            }
            let imported = canonical(&path.parent().unwrap().join(from))
                .map_err(|e| error(element.span, format!("依赖路径 {from} 不可用：{e}")))?;
            if !imported.starts_with(&self.package) {
                return Err(error(
                    element.span,
                    "Import 不得越过根文件所在包目录".into(),
                ));
            }
            if self.active.contains(&imported) {
                return Err(error(element.span, format!("模块导入循环：{from}")));
            }
            let (module, symbols) = self.load(&imported, depth + 1)?;
            if module.version != version {
                return Err(error(
                    element.span,
                    format!(
                        "依赖 {alias} 版本不符：需要 {version}，实际 {}",
                        module.version
                    ),
                ));
            }
            if module.view.is_some() {
                return Err(error(
                    element.span,
                    "首版 Import 只组合业务库；带 View 的模块由宿主显式挂载".into(),
                ));
            }
            self.graph.add_import(
                path,
                &imported,
                Some(alias.into()),
                element.span.line,
                element.span.column,
            );
            dependency_symbols.push(ModuleSymbol {
                name: alias.into(),
                detail: format!("Import {}@{}", module.name, module.version),
                start: 0,
                end: 0,
                source_id: SourceId::from_source_name(&imported.to_string_lossy()),
                scope_id: SourceId::from_source_name(&name),
                import_range: Some((element.span.start, element.span.end)),
                exported: false,
            });
            dependency_symbols.extend(
                symbols
                    .iter()
                    .filter(|symbol| {
                        symbol.exported
                            && symbol.scope_id
                                == SourceId::from_source_name(&imported.to_string_lossy())
                    })
                    .cloned()
                    .map(|mut symbol| {
                        symbol.name = format!("{alias}.{}", symbol.name);
                        symbol.scope_id = SourceId::from_source_name(&name);
                        // 依赖可供当前模块使用，但不能继续通过当前模块隐式再导出。
                        symbol.exported = false;
                        symbol
                    }),
            );
            // 工具可在依赖的词法范围内导航私有声明，不将其纳入父模块可调用名称。
            dependency_symbols.extend(symbols);
            imports.push((alias.to_string(), module));
        }
        let (module, mut symbols) = check::module(&document, &name, &source, &imports)
            .map_err(|d| diagnostic(&name, DiagnosticPhase::Semantic, d))?;
        symbols.extend(dependency_symbols);
        let mut seen_symbols = BTreeSet::new();
        symbols.retain(|symbol| seen_symbols.insert((symbol.scope_id, symbol.name.clone())));
        self.active.remove(path);
        self.cache
            .insert(path.to_path_buf(), (module.clone(), symbols.clone()));
        Ok((module, symbols))
    }
}

pub(super) fn diagnostic(
    name: &str,
    phase: DiagnosticPhase,
    diagnostic: Diagnostic,
) -> CompilerDiagnostic {
    CompilerDiagnostic::from_language(
        name,
        SourceId::from_source_name(name),
        phase,
        if phase == DiagnosticPhase::Syntax {
            "UIX1000"
        } else {
            "UIX2100"
        },
        diagnostic,
    )
}

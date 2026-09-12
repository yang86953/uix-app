// 引入确定性名称、缓存与依赖集合。
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
// 引入编译期文件读取能力。
use std::fs;
// 引入导入路径所有权。
use std::path::{Path, PathBuf};
use std::sync::Arc;

// 引入纯 UIX 文档、声明、节点与诊断契约。
use crate::lang::compiler::DiagnosticPhase;
use crate::lang::compiler::source_graph::{SourceGraph, SourceGraphBuilder, SourceId};
use crate::lang::compiler::uix_lang::{
    Declaration, Diagnostic, Document, Element, ImportDeclaration, Node, SourceSpan,
    WidgetDeclaration,
};

// 保存已经解析并完成导入合并的文件文档。
#[derive(Debug, Clone)]
pub(crate) struct ResolvedDocument {
    // 保存交给既有纯 codegen 的单一文档。
    pub(crate) document: Document,
    // 保存与合并声明顺序严格对齐的真实来源身份。
    pub(crate) declaration_sources: Vec<SourceId>,
    // 保存根文件及全部递归导入文件的规范路径。
    pub(crate) tracked_files: Vec<PathBuf>,
    // 保存稳定源码身份、快照与导入边。
    pub(crate) source_graph: SourceGraph,
}

// 保存带真实来源文件的入口诊断。
#[derive(Debug, Clone)]
pub(crate) struct ImportDiagnostic {
    // 保存跨入口稳定的诊断代码。
    pub(crate) code: &'static str,
    // 保存失败所属 Compiler System 阶段。
    pub(crate) phase: DiagnosticPhase,
    // 保存应展示给调用者的具体来源路径。
    pub(crate) source_name: String,
    // 保存既有结构化语言诊断。
    pub(crate) diagnostic: Diagnostic,
}

// 保存一条声明及其真实定义文件。
#[derive(Clone)]
struct SourcedDeclaration {
    // 保存纯 parser 产生的声明。
    declaration: Declaration,
    // 保存声明所属的规范文件路径。
    source: PathBuf,
}

// 保存一个文件完成递归合并后的可导入单元。
#[derive(Clone)]
struct ResolvedUnit {
    // 保存已经去除 import/export 指令的声明。
    declarations: Vec<SourcedDeclaration>,
    // 保存显式公开的组件名称顺序。
    exports: Vec<String>,
    // 保存本文件唯一根；被导入时不复制该根。
    root: Element,
}

// 保存跨 CompilerSession 调用复用的单文件 Syntax 阶段产物。
#[derive(Debug, Default)]
pub(crate) struct SourceStageCache {
    // 每个规范路径只保留最新源码对应的解析结果，避免编辑会话按版本无界增长。
    documents: BTreeMap<PathBuf, CachedDocument>,
    // 记录当前解析命令实际触及的文件，供失败源码图建立可释放的所有权边界。
    active_request_paths: Option<BTreeSet<PathBuf>>,
    // 测试只观测真正进入 parser 的次数，不把墙钟时间作为确定性门禁。
    #[cfg(test)]
    parse_runs: usize,
}

// 把源码快照与成功或失败的 Syntax 结论绑定，重复无效输入同样无需重解析。
#[derive(Debug, Clone)]
struct CachedDocument {
    source: String,
    allow_declaration_only: bool,
    result: Result<Document, ImportDiagnostic>,
}

impl SourceStageCache {
    // 开始记录一次根命令触及的 Syntax 缓存路径。
    pub(crate) fn begin_request(&mut self) {
        debug_assert!(self.active_request_paths.is_none());
        self.active_request_paths = Some(BTreeSet::new());
    }

    // 结束当前记录并返回失败或成功解析已经触及的文件集合。
    pub(crate) fn finish_request(&mut self) -> BTreeSet<PathBuf> {
        self.active_request_paths.take().unwrap_or_default()
    }

    // 解析一个规范文件；完全相同的源码直接复用不可变 AST 或稳定诊断。
    fn parse(
        &mut self,
        path: &Path,
        source: &str,
        allow_declaration_only: bool,
    ) -> Result<Document, ImportDiagnostic> {
        if let Some(paths) = self.active_request_paths.as_mut() {
            paths.insert(path.to_path_buf());
        }
        if let Some(cached) = self.documents.get(path)
            && cached.source == source
            && cached.allow_declaration_only == allow_declaration_only
        {
            return cached.result.clone();
        }
        #[cfg(test)]
        {
            self.parse_runs = self.parse_runs.saturating_add(1);
        }
        let result = if allow_declaration_only {
            crate::lang::compiler::uix_lang::parse_items_document(source)
        } else {
            crate::lang::compiler::uix_lang::parse_document(source)
        }
        .map_err(|diagnostic| ImportDiagnostic {
            code: "UIX1000",
            phase: DiagnosticPhase::Syntax,
            source_name: path.display().to_string(),
            diagnostic,
        });
        self.documents.insert(
            path.to_path_buf(),
            CachedDocument {
                source: source.to_owned(),
                allow_declaration_only,
                result: result.clone(),
            },
        );
        result
    }

    // 只保留仍被成功或失败源码图拥有的文件，关闭或改写根后释放不可达缓存。
    pub(crate) fn retain_paths(&mut self, retained: &BTreeSet<PathBuf>) {
        self.documents.retain(|path, _| retained.contains(path));
    }


}

// 保存递归解析期间的确定状态与唯一缓存。
struct ImportResolver<'a> {
    // 保存当前深度优先解析栈以拒绝循环。
    stack: Vec<PathBuf>,
    // 保存同一展开内已经解析的文件单元。
    cache: BTreeMap<PathBuf, Arc<ResolvedUnit>>,
    // 保存首次读取顺序中的 rustc 依赖路径。
    tracked_files: Vec<PathBuf>,
    // 保存依赖路径去重集合。
    tracked_set: BTreeSet<PathBuf>,
    // 保存与解析使用同一读取事实的源码图构建器。
    source_graph: SourceGraphBuilder,
    // 保存 LSP 会话提供的未落盘文件快照；正式 AOT 传入空集合。
    overlays: &'a BTreeMap<PathBuf, String>,
    // 借用 CompilerSession 拥有的单文件 Syntax 阶段缓存。
    source_cache: &'a mut SourceStageCache,
    // Items 资源允许根文件及导入资源只包含模块级声明。
    allow_declaration_only: bool,
}

// 保存已合并声明与各命名空间来源。
#[derive(Default)]
struct DeclarationMerge {
    // 保存源码顺序中的唯一声明。
    declarations: Vec<SourcedDeclaration>,
    // 保存声明命名空间、名称到来源文件的映射。
    names: BTreeMap<(DeclarationKind, String), (PathBuf, Declaration)>,
}

// 区分 parser 已定义的六类具名声明命名空间。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DeclarationKind {
    // 保存样式类命名空间。
    Style,
    // 保存主题命名空间。
    Theme,
    // 保存关键帧动画命名空间。
    Keyframes,
    // 保存 @media 条件块命名空间（以条件文本与源码位置区分）。
    Media,
    // 保存组件命名空间。
    Widget,
    // 保存 record 命名空间。
    Record,
    // 保存 Visual 常量命名空间。
    Visual,
}

// 相对一个真实 .uix 根文件解析完整编译单元。
pub(crate) fn resolve_file(path: &Path) -> Result<ResolvedDocument, ImportDiagnostic> {
    resolve_file_with_overlays(path, &BTreeMap::new())
}

// 相对真实路径解析允许省略视图根的 Items 资源及其导入闭包。
pub(crate) fn resolve_items_file(path: &Path) -> Result<ResolvedDocument, ImportDiagnostic> {
    resolve_items_file_with_overlays(path, &BTreeMap::new())
}

// 相对真实路径解析文件，同时让编辑器内存快照覆盖磁盘内容。
pub(crate) fn resolve_file_with_overlays(
    path: &Path,
    overlays: &BTreeMap<PathBuf, String>,
) -> Result<ResolvedDocument, ImportDiagnostic> {
    resolve_file_with_overlays_mode(path, overlays, false)
}

// 解析允许只有声明的 Items 资源，同时保留覆盖快照与导入图。
pub(crate) fn resolve_items_file_with_overlays(
    path: &Path,
    overlays: &BTreeMap<PathBuf, String>,
) -> Result<ResolvedDocument, ImportDiagnostic> {
    resolve_file_with_overlays_mode(path, overlays, true)
}

// 按入口目标选择声明资源语法，并保持其余解析机制完全共享。
fn resolve_file_with_overlays_mode(
    path: &Path,
    overlays: &BTreeMap<PathBuf, String>,
    allow_declaration_only: bool,
) -> Result<ResolvedDocument, ImportDiagnostic> {
    let mut source_cache = SourceStageCache::default();
    resolve_file_with_overlays_cached(path, overlays, &mut source_cache, allow_declaration_only)
}

// 相对真实路径解析文件，并跨同一 CompilerSession 复用未变化文件的 Syntax 产物。
pub(crate) fn resolve_file_with_overlays_cached(
    path: &Path,
    overlays: &BTreeMap<PathBuf, String>,
    source_cache: &mut SourceStageCache,
    allow_declaration_only: bool,
) -> Result<ResolvedDocument, ImportDiagnostic> {
    // 先取得根文件规范路径，避免同一文件通过不同相对路径绕过循环检测。
    let canonical = canonical_or_overlay(path, overlays).map_err(|error| {
        // 根文件尚无源码跨度，使用稳定的一行一列入口位置。
        source_error(
            // 保留用户解析后的实际路径。
            path,
            // 使用文件入口起点。
            entry_span(),
            // 报告具体文件系统错误。
            format!("无法读取 UIX 文件 {}：{error}", path.display()),
            // 给出调用 crate 基准、权限与编码修复方向。
            "确认路径相对调用 crate 的 CARGO_MANIFEST_DIR、文件存在且为 UTF-8",
        )
    })?;
    // 为本次宏展开创建唯一 resolver，不跨调用共享状态。
    let mut resolver =
        ImportResolver::new(&canonical, overlays, source_cache, allow_declaration_only);
    // 解析根及递归依赖。
    let unit = resolver.load_unit(&canonical)?;
    // 根单元不会被递归复用；移出私有缓存后恢复唯一所有权，直接交付最终 Document。
    let cached_root = resolver.cache.remove(&canonical);
    debug_assert!(cached_root.is_some());
    drop(cached_root);
    let unit = Arc::into_inner(unit).expect("根解析单元必须只由当前解析结果持有");
    // 把带来源声明投影回既有纯 Document 契约。
    let declaration_sources = unit
        // 在消费声明前保留每项真实定义文件。
        .declarations
        // 借用全部带来源声明。
        .iter()
        // 投影到 SourceGraph 使用的同一身份算法。
        .map(|entry| SourceId::from_source_name(&entry.source.display().to_string()))
        // 保持合并后的声明顺序。
        .collect();
    // 去除内部来源包装，交给既有 AST 消费者。
    let declarations = unit
        // 消费本次独立单元声明。
        .declarations
        // 转成拥有所有权的迭代器。
        .into_iter()
        // codegen 不需要文件系统来源元数据。
        .map(|entry| entry.declaration)
        // 恢复源码顺序。
        .collect();
    // 在消费 resolver 前完成稳定源码图。
    let source_graph = resolver.source_graph.finish();
    // 返回完整文档与依赖列表。
    Ok(ResolvedDocument {
        // 保留根文件唯一根并安装合并声明。
        document: Document {
            // 写入递归合并后的声明。
            declarations,
            // 根始终只属于入口文件。
            root: unit.root,
        },
        // 保留声明级文件身份供 Semantic Model 使用。
        declaration_sources,
        // 交给入口生成全部 include_str!。
        tracked_files: resolver.tracked_files,
        // 交给 Compiler System、check 与 LSP 共享。
        source_graph,
    })
}

// 拒绝没有稳定文件基准目录的内嵌源码导入。
pub(crate) fn reject_inline_imports(document: &Document) -> Result<(), Diagnostic> {
    // 查找第一条不能确定相对路径所有权的导入。
    if let Some(import) = document.declarations.iter().find_map(|declaration| {
        // 只提取 import 指令。
        match declaration {
            // 返回导入借用。
            Declaration::Import(import) => Some(import),
            // 其他声明不需要文件系统。
            _ => None,
        }
    }) {
        // 返回定向诊断，禁止继续静默忽略。
        return Err(Diagnostic::new(
            // 锚定具体 import 指令。
            import.span,
            // 说明内嵌来源没有相对路径基准。
            "内嵌 UIX 源码不能使用 @import",
            // 给出唯一稳定文件入口方案。
            "把源码保存为 .uix 文件并向 uix!、uix_app! 或 uix_items! 传入该路径",
        ));
    }
    // 没有 import 时保持原有内嵌代码生成路径。
    Ok(())
}

// 实现文件单元递归读取、缓存与循环检测。
impl<'a> ImportResolver<'a> {
    // 为一个真实根文件创建独占源码图解析器。
    fn new(
        root: &Path,
        overlays: &'a BTreeMap<PathBuf, String>,
        source_cache: &'a mut SourceStageCache,
        allow_declaration_only: bool,
    ) -> Self {
        Self {
            stack: Vec::new(),
            cache: BTreeMap::new(),
            tracked_files: Vec::new(),
            tracked_set: BTreeSet::new(),
            source_graph: SourceGraphBuilder::new(root),
            overlays,
            source_cache,
            allow_declaration_only,
        }
    }

    // 读取一个规范路径并完成其全部导入。
    fn load_unit(&mut self, path: &Path) -> Result<Arc<ResolvedUnit>, ImportDiagnostic> {
        // 已完成的单元可以在同一宏展开内安全共享不可变解析结果。
        if let Some(unit) = self.cache.get(path) {
            // 只复制共享句柄，避免为同一解析单元深拷贝完整 AST。
            return Ok(Arc::clone(unit));
        }
        // 进入解析栈前拒绝同一文件再次出现。
        if let Some(position) = self.stack.iter().position(|entry| entry == path) {
            // 构造包含完整回路的稳定诊断文本。
            let mut cycle = self.stack[position..]
                // 遍历回路已有节点。
                .iter()
                // 投影为可读路径。
                .map(|entry| entry.display().to_string())
                // 收集回路顺序。
                .collect::<Vec<_>>();
            // 闭合到重复目标。
            cycle.push(path.display().to_string());
            // 返回文件入口级循环诊断。
            return Err(import_error(
                // 当前重复目标是最具体来源。
                path,
                // 没有 import 跨度时使用入口位置。
                entry_span(),
                // 展示闭合依赖链。
                format!("UIX 导入形成循环：{}", cycle.join(" -> ")),
                // 给出打破循环的修复动作。
                "移除回边，保持 .uix 文件依赖图无环",
            ));
        }
        // 把当前文件压入唯一深度优先栈。
        self.stack.push(path.to_path_buf());
        // 执行可能失败的实际解析。
        let result = self.load_unit_inner(path).map(Arc::new);
        // 无论成功失败都弹出当前文件，避免污染后续诊断。
        let popped = self.stack.pop();
        // 栈顶必须与本次读取目标一致。
        debug_assert_eq!(popped.as_deref(), Some(path));
        // 成功时写入本次 resolver 私有缓存。
        if let Ok(unit) = &result {
            // 后续同路径导入共享不可变解析单元。
            self.cache.insert(path.to_path_buf(), Arc::clone(unit));
        }
        // 返回原始解析结果。
        result
    }

    // 执行一个文件的读取、解析、递归导入与 export 校验。
    fn load_unit_inner(&mut self, path: &Path) -> Result<ResolvedUnit, ImportDiagnostic> {
        // 第一次读取时登记 rustc 依赖顺序。
        if self.tracked_set.insert(path.to_path_buf()) {
            // 保存根优先的深度优先确定顺序。
            self.tracked_files.push(path.to_path_buf());
        }
        // 读取完整 UTF-8 源码。
        let source = if let Some(source) = overlay_source(self.overlays, path) {
            // LSP 会话在整个请求期间拥有 overlay，解析器直接借用而不复制完整源码。
            Cow::Borrowed(source)
        } else {
            Cow::Owned(fs::read_to_string(path).map_err(|error| {
                // 把文件系统错误归到具体目标文件。
                source_error(
                    // 展示规范来源。
                    path,
                    // 读取前没有更精确源码位置。
                    entry_span(),
                    // 保留系统原因。
                    format!("无法读取 UIX 文件 {}：{error}", path.display()),
                    // 给出权限与 UTF-8 修复方向。
                    "确认文件存在、可读且为 UTF-8",
                )
            })?)
        };
        // SourceGraph 与 parser 消费完全相同的不可变源码快照。
        self.source_graph.insert_file(path, &source);
        // 使用会话级 Source 缓存复用完全相同源码的 AST 或稳定语法诊断。
        let document = self
            .source_cache
            .parse(path, &source, self.allow_declaration_only)?;
        // 保存导出名及其声明跨度供跨指令去重和存在性校验。
        let mut export_requests = Vec::new();
        // 创建按命名空间去重的合并器。
        let mut merge = DeclarationMerge::default();
        // 逐条保持源文件声明顺序。
        for declaration in document.declarations {
            // 按声明种类选择文件边界行为。
            match declaration {
                // import 在当前位置展开选定组件及其支持声明。
                Declaration::Import(import) => {
                    // 解析并选择目标文件的公开组件。
                    let imported = self.resolve_import(path, &import)?;
                    // 在当前 import 位置合并全部选定声明。
                    for entry in imported {
                        // 重复声明诊断锚定当前 import。
                        merge.push(entry, path, import.span)?;
                    }
                }
                // export 只控制文件边界，不进入纯 codegen 文档。
                Declaration::Export(export) => {
                    // 保存每个公开组件与同一指令跨度。
                    for widget in export.widgets {
                        // 跨多个 export 指令重复也必须被拒绝。
                        if export_requests
                            // 遍历已有导出名。
                            .iter()
                            // 比较组件名称。
                            .any(|(name, _)| name == &widget)
                        {
                            // 返回当前文件内定向诊断。
                            return Err(import_error(
                                // 归属当前文件。
                                path,
                                // 锚定重复 export。
                                export.span,
                                // 报告具体重复名称。
                                format!("@export 重复列出组件 {widget}"),
                                // 要求每个组件只公开一次。
                                "每个组件只导出一次",
                            ));
                        }
                        // 保存首次导出请求。
                        export_requests.push((widget, export.span));
                    }
                }
                // 其他声明保持当前文件来源并进入合并器。
                declaration => {
                    // 使用声明自身跨度定位本地重复。
                    let span = declaration_span(&declaration);
                    // 合并当前文件真实声明。
                    merge.push(
                        // 保留定义来源。
                        SourcedDeclaration {
                            // 保存声明。
                            declaration,
                            // 保存当前规范文件。
                            source: path.to_path_buf(),
                        },
                        // 重复时归属当前文件。
                        path,
                        // 锚定当前声明。
                        span,
                    )?;
                }
            }
        }
        // 收集完成递归合并后实际存在的组件名。
        let widget_names = merge
            // 遍历唯一声明。
            .declarations
            // 借用声明序列。
            .iter()
            // 只保留组件名称。
            .filter_map(|entry| widget_name(&entry.declaration))
            // 收集成确定性集合。
            .collect::<BTreeSet<_>>();
        // 每个 export 必须指向当前编译单元实际组件。
        for (widget, span) in &export_requests {
            // 已存在时继续。
            if widget_names.contains(widget.as_str()) {
                // 验证下一项。
                continue;
            }
            // 缺失导出不能留到使用方得到未知标签错误。
            return Err(import_error(
                // 归属声明文件。
                path,
                // 锚定具体 export。
                *span,
                // 报告缺失组件。
                format!("@export 指定的组件 {widget} 不存在"),
                // 给出定义或删除两种动作。
                "在同一导入单元中定义该 Widget，或从 @export 删除该名称",
            ));
        }
        // 返回可缓存文件单元。
        Ok(ResolvedUnit {
            // 保存完成去重的声明。
            declarations: merge.declarations,
            // 只保留导出名称顺序。
            exports: export_requests
                // 消费导出请求。
                .into_iter()
                // 丢弃已完成校验的跨度。
                .map(|(name, _)| name)
                // 保持作者顺序。
                .collect(),
            // 根在导入时忽略，在入口文件中保留。
            root: document.root,
        })
    }

    // 解析一条 import 并取得选定组件闭包。
    fn resolve_import(
        &mut self,
        // 接收拥有 import 指令的当前文件。
        importer: &Path,
        // 接收结构化 import 声明。
        import: &ImportDeclaration,
    ) -> Result<Vec<SourcedDeclaration>, ImportDiagnostic> {
        // 相对 importing 文件目录解析目标，而非相对进程工作目录。
        let base = importer.parent().unwrap_or_else(|| Path::new("."));
        // 绝对路径保持不变，相对路径拼到 importing 文件目录。
        let requested = if Path::new(&import.path).is_absolute() {
            // 复制绝对导入目标。
            PathBuf::from(&import.path)
        } else {
            // 拼接当前文件目录。
            base.join(&import.path)
        };
        // 取得规范路径并暴露缺失文件原因。
        let canonical = canonical_or_overlay(&requested, self.overlays).map_err(|error| {
            // 错误归属 import 所在文件与指令。
            import_error(
                // 展示 importing 文件。
                importer,
                // 锚定 import。
                import.span,
                // 同时报告解析后目标与系统原因。
                format!("无法读取导入文件 {}：{error}", requested.display()),
                // 给出相对路径语义。
                "确认路径相对当前 importing .uix 文件且目标存在、可读、为 UTF-8",
            )
        })?;
        // 在递归读取前记录当前指令形成的有向边与选择器。
        self.source_graph.add_import(
            importer,
            &canonical,
            import.widget.clone(),
            import.span.line,
            import.span.column,
        );
        // 在递归调用前用当前 import 跨度提供更准确的循环诊断。
        if let Some(position) = self.stack.iter().position(|entry| entry == &canonical) {
            // 取得当前回路路径片段。
            let mut cycle = self.stack[position..]
                // 遍历已有节点。
                .iter()
                // 转为可读路径。
                .map(|entry| entry.display().to_string())
                // 收集顺序。
                .collect::<Vec<_>>();
            // 闭合回路。
            cycle.push(canonical.display().to_string());
            // 返回当前 import 上的循环错误。
            return Err(import_error(
                // 归属 importing 文件。
                importer,
                // 锚定形成回边的 import。
                import.span,
                // 展示完整回路。
                format!("UIX 导入形成循环：{}", cycle.join(" -> ")),
                // 给出无环修复方向。
                "移除回边，保持 .uix 文件依赖图无环",
            ));
        }
        // 递归读取目标文件。
        let unit = self.load_unit(&canonical)?;
        // 选择公开组件及其本地组件依赖。
        select_imported_declarations(importer, import, &unit)
    }
}

// 优先使用真实规范路径；未落盘文件只允许命中明确的 LSP overlay。
fn canonical_or_overlay(
    path: &Path,
    overlays: &BTreeMap<PathBuf, String>,
) -> std::io::Result<PathBuf> {
    match fs::canonicalize(path) {
        Ok(canonical) => Ok(canonical),
        Err(error) => {
            let normalized = normalized_overlay_path(path);
            if overlay_source(overlays, &normalized).is_some() {
                Ok(normalized)
            } else {
                Err(error)
            }
        }
    }
}

// 按规范或父目录规范化身份查询会话快照。
pub(crate) fn overlay_source<'a>(
    overlays: &'a BTreeMap<PathBuf, String>,
    path: &Path,
) -> Option<&'a str> {
    overlays
        .get(path)
        .or_else(|| {
            overlays
                .iter()
                .find(|(candidate, _)| normalized_overlay_path(candidate) == path)
                .map(|(_, source)| source)
        })
        .map(String::as_str)
}

// 为尚不存在的文件使用已存在父目录形成稳定绝对身份。
pub(crate) fn normalized_overlay_path(path: &Path) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(path) {
        return canonical;
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    absolute
        .parent()
        .and_then(|parent| fs::canonicalize(parent).ok())
        .and_then(|parent| absolute.file_name().map(|name| parent.join(name)))
        .unwrap_or(absolute)
}

// 实现具名声明的跨文件合并与重复诊断。
impl DeclarationMerge {
    // 合并一条声明；同一来源的重复支持声明可以幂等复用。
    fn push(
        &mut self,
        // 接收带定义来源的声明。
        entry: SourcedDeclaration,
        // 接收本次合并动作所属文件。
        diagnostic_source: &Path,
        // 接收本次合并位置。
        diagnostic_span: SourceSpan,
    ) -> Result<(), ImportDiagnostic> {
        // 无名称声明不应从解析后的导入单元到达这里。
        let Some((kind, name)) = declaration_identity(&entry.declaration) else {
            // import/export 已由 resolver 消费。
            return Ok(());
        };
        // 构造独立命名空间 key。
        let key = (kind, name.clone());
        // 检查已有同名定义。
        if let Some((source, declaration)) = self.names.get(&key) {
            // 同一文件同一声明经不同选择性导入重复出现时幂等跳过。
            if source == &entry.source && declaration == &entry.declaration {
                // 不复制同一支持声明。
                return Ok(());
            }
            // 不同定义的同名声明必须在入口阶段失败。
            return Err(import_error(
                // 归属触发冲突的当前合并动作。
                diagnostic_source,
                // 锚定当前 import 或本地声明。
                diagnostic_span,
                // 报告命名空间与既有来源。
                format!(
                    "导入后{} {name} 重复声明；已有定义来自 {}",
                    declaration_kind_name(kind),
                    source.display()
                ),
                // 给出消除冲突的确定动作。
                "重命名冲突声明，或删除重复 import",
            ));
        }
        // 登记首次命名定义与完整声明副本。
        self.names
            // 插入确定性 key。
            .insert(key, (entry.source.clone(), entry.declaration.clone()));
        // 保存源码顺序中的实际声明。
        self.declarations.push(entry);
        // 返回成功。
        Ok(())
    }
}

// 从目标文件选择公开组件、组件依赖与支持声明。
fn select_imported_declarations(
    // 接收 importing 文件用于诊断。
    importer: &Path,
    // 接收 import 选择器。
    import: &ImportDeclaration,
    // 接收完成递归解析的目标单元。
    unit: &ResolvedUnit,
) -> Result<Vec<SourcedDeclaration>, ImportDiagnostic> {
    // 没有显式 export 的文件不能被其他编译单元消费。
    if unit.exports.is_empty() {
        // 返回当前 import 上的边界错误。
        return Err(import_error(
            // 归属 importing 文件。
            importer,
            // 锚定 import。
            import.span,
            // 报告目标未公开组件。
            format!("导入文件 {} 没有 @export", import.path),
            // 要求目标显式声明公开组件。
            "在目标文件添加 @export('WidgetName')",
        ));
    }
    // 取得本次 import 的初始公开组件集合。
    let requested = if let Some(widget) = &import.widget {
        // 具名导入必须命中目标 export。
        if !unit.exports.contains(widget) {
            // 返回缺失公开组件诊断。
            return Err(import_error(
                // 归属 importing 文件。
                importer,
                // 锚定具名 import。
                import.span,
                // 展示具体名称与目标。
                format!("组件 {widget} 未由 {} 导出", import.path),
                // 给出目标 export 修复动作。
                format!("在目标文件添加 @export('{widget}')，或改为已导出的组件名"),
            ));
        }
        // 只从指定公开组件开始依赖闭包。
        vec![widget.clone()]
    } else {
        // 无选择器时使用目标全部显式 export。
        unit.exports.clone()
    };
    // 建立目标单元全部组件声明索引。
    let widgets = unit
        // 遍历完成合并的声明。
        .declarations
        // 借用声明序列。
        .iter()
        // 只收集组件声明。
        .filter_map(|entry| match &entry.declaration {
            // 保存名称到声明的映射。
            Declaration::Widget(widget) => Some((widget.name.clone(), widget)),
            // 其他声明不参与组件依赖闭包。
            _ => None,
        })
        // 收集确定性索引。
        .collect::<BTreeMap<_, _>>();
    // 初始化待扫描组件栈。
    let mut pending = requested;
    // 保存已经选择的组件名称。
    let mut selected = BTreeSet::new();
    // 深度优先收集本地组件依赖。
    while let Some(name) = pending.pop() {
        // 重复依赖不再扫描。
        if !selected.insert(name.clone()) {
            // 继续下一个待扫描名称。
            continue;
        }
        // export 存在性已经由目标单元校验保证。
        let Some(widget) = widgets.get(&name) else {
            // 防御性返回目标单元不一致错误。
            return Err(import_error(
                // 归属 importing 文件。
                importer,
                // 锚定 import。
                import.span,
                // 报告具体缺失组件。
                format!("导入单元缺少组件 {name}"),
                // 给出重新构建 export 的动作。
                "修复目标文件的 @export 与 Widget 声明",
            ));
        };
        // 收集组件体中对同单元自定义组件的引用。
        collect_widget_dependencies(widget, &widgets, &mut pending);
    }
    // 按目标单元原始合并顺序投影声明。
    Ok(unit
        // 遍历全部声明。
        .declarations
        // 借用声明序列。
        .iter()
        // 保留支持声明和选定组件。
        .filter(|entry| match &entry.declaration {
            // 组件必须位于依赖闭包。
            Declaration::Widget(widget) => selected.contains(&widget.name),
            // 样式、主题、关键帧、媒体块与 record 是所选组件的编译期支持声明。
            Declaration::StyleClass(_)
            | Declaration::Theme(_)
            | Declaration::Keyframes(_)
            | Declaration::Media(_)
            | Declaration::Record(_)
            | Declaration::Visual(_) => true,
            // import/export 已由 resolver 消费。
            Declaration::Import(_) | Declaration::Export(_) => false,
        })
        // 克隆为 importing 单元自己的合并输入。
        .cloned()
        // 保持目标声明顺序。
        .collect())
}

// 收集一个组件体引用的同单元组件名称。
fn collect_widget_dependencies(
    // 接收待扫描组件。
    widget: &WidgetDeclaration,
    // 接收同单元组件索引。
    widgets: &BTreeMap<String, &WidgetDeclaration>,
    // 接收待扫描栈。
    pending: &mut Vec<String>,
) {
    // 遍历组件所有直接节点。
    for node in &widget.children {
        // 递归扫描元素树。
        collect_node_dependencies(node, widgets, pending);
    }
}

// 递归收集节点树中的自定义组件引用。
fn collect_node_dependencies(
    // 接收当前节点。
    node: &Node,
    // 接收同单元组件索引。
    widgets: &BTreeMap<String, &WidgetDeclaration>,
    // 接收待扫描栈。
    pending: &mut Vec<String>,
) {
    // 只有元素节点能引用组件。
    let Node::Element(element) = node else {
        // 文本与插值没有组件依赖。
        return;
    };
    // 同名本地组件进入依赖闭包。
    if widgets.contains_key(&element.name) {
        // 保存待扫描名称。
        pending.push(element.name.clone());
    }
    // 继续扫描该元素全部子节点。
    for child in &element.children {
        // 递归发现深层引用。
        collect_node_dependencies(child, widgets, pending);
    }
}

// 返回声明的命名空间与名称。
fn declaration_identity(declaration: &Declaration) -> Option<(DeclarationKind, String)> {
    // 按声明种类提取名称。
    match declaration {
        // 样式类使用包含伪状态的完整限定名，允许基础类与状态变体共同导入。
        Declaration::StyleClass(style) => Some((
            // 保持样式命名空间不变。
            DeclarationKind::Style,
            // 状态变体以 `类名:状态` 形成独立身份。
            style
                // 读取可选伪状态。
                .state
                // 存在状态时拼接限定名。
                .map(|state| format!("{}:{}", style.name, state.as_str()))
                // 基础类继续只使用原始名称。
                .unwrap_or_else(|| style.name.clone()),
        )),
        // 主题使用主题命名空间。
        Declaration::Theme(theme) => Some((DeclarationKind::Theme, theme.name.clone())),
        // 关键帧使用动画命名空间。
        Declaration::Keyframes(keyframes) => {
            // 返回关键帧声明名称。
            Some((DeclarationKind::Keyframes, keyframes.name.clone()))
        }
        // 媒体块没有名称，用条件文本与起始偏移形成稳定身份，允许同条件多块并存。
        Declaration::Media(media) => Some((
            DeclarationKind::Media,
            format!("{}@{}", media.query.canonical_text(), media.span.start),
        )),
        // 组件使用组件命名空间。
        Declaration::Widget(widget) => {
            // 返回组件名称。
            Some((DeclarationKind::Widget, widget.name.clone()))
        }
        // record 使用 record 命名空间。
        Declaration::Record(record) => Some((DeclarationKind::Record, record.name.clone())),
        // Visual 使用模块级常量命名空间。
        Declaration::Visual(visual) => Some((DeclarationKind::Visual, visual.name.clone())),
        // import/export 不进入 codegen 文档。
        Declaration::Import(_) | Declaration::Export(_) => None,
    }
}

// 返回声明用于诊断的中文种类名。
fn declaration_kind_name(kind: DeclarationKind) -> &'static str {
    // 每类命名空间映射稳定文案。
    match kind {
        // 样式类文案。
        DeclarationKind::Style => "样式类",
        // 主题文案。
        DeclarationKind::Theme => "主题",
        // 关键帧文案。
        DeclarationKind::Keyframes => "关键帧",
        // 媒体块文案。
        DeclarationKind::Media => "@media 块",
        // 组件文案。
        DeclarationKind::Widget => "组件",
        // record 文案。
        DeclarationKind::Record => "Record",
        // Visual 文案。
        DeclarationKind::Visual => "Visual",
    }
}

// 返回声明起始跨度。
fn declaration_span(declaration: &Declaration) -> SourceSpan {
    // 每类声明都拥有完整跨度。
    match declaration {
        // 返回 import 跨度。
        Declaration::Import(value) => value.span,
        // 返回 export 跨度。
        Declaration::Export(value) => value.span,
        // 返回样式类跨度。
        Declaration::StyleClass(value) => value.span,
        // 返回主题跨度。
        Declaration::Theme(value) => value.span,
        // 返回关键帧跨度。
        Declaration::Keyframes(value) => value.span,
        // 返回媒体块跨度。
        Declaration::Media(value) => value.span,
        // 返回组件跨度。
        Declaration::Widget(value) => value.span,
        // 返回 record 跨度。
        Declaration::Record(value) => value.span,
        // 返回 Visual 跨度。
        Declaration::Visual(value) => value.span,
    }
}

// 返回可借用的组件名称。
fn widget_name(declaration: &Declaration) -> Option<&str> {
    // 只接受组件声明。
    match declaration {
        // 返回组件名称。
        Declaration::Widget(widget) => Some(widget.name.as_str()),
        // 其他声明没有组件名称。
        _ => None,
    }
}

// 构造文件入口尚无源码时的一行一列跨度。
fn entry_span() -> SourceSpan {
    // 返回空半开区间的一基位置。
    SourceSpan {
        // 起始字节为零。
        start: 0,
        // 结束字节为零。
        end: 0,
        // 行号为一。
        line: 1,
        // 列号为一。
        column: 1,
    }
}

// 构造带真实文件来源的结构化导入诊断。
fn import_error(
    // 接收失败所属文件。
    source: &Path,
    // 接收精确源码跨度。
    span: SourceSpan,
    // 接收失败原因。
    message: impl Into<String>,
    // 接收修复建议。
    suggestion: impl Into<String>,
) -> ImportDiagnostic {
    // 返回入口诊断。
    ImportDiagnostic {
        // 文件读取与模块图错误共享稳定模块代码。
        code: "UIX1100",
        // 这条辅助入口只构造源码或导入阶段错误。
        phase: DiagnosticPhase::Import,
        // 保存可读路径。
        source_name: source.display().to_string(),
        // 复用语言层结构化诊断。
        diagnostic: Diagnostic::new(span, message, suggestion),
    }
}

// 构造读取或编码失败的结构化源码诊断。
fn source_error(
    // 接收失败所属文件。
    source: &Path,
    // 接收文件入口跨度。
    span: SourceSpan,
    // 接收失败原因。
    message: impl Into<String>,
    // 接收修复建议。
    suggestion: impl Into<String>,
) -> ImportDiagnostic {
    // 返回保持源码阶段身份的入口诊断。
    ImportDiagnostic {
        // 读取失败拥有独立稳定代码。
        code: "UIX0001",
        // 不把文件系统错误误报为模块语义。
        phase: DiagnosticPhase::Source,
        // 保存可读路径。
        source_name: source.display().to_string(),
        // 复用语言层位置与文案契约。
        diagnostic: Diagnostic::new(span, message, suggestion),
    }
}

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../tests-src/uix_import_inner_tests.rs"]
mod uix_import_inner_tests;

// 增量缓存观测方法位于 tests-src（同模块 include!）。
#[cfg(test)]
include!("../tests-src/uix_import_probe_fns.rs");

//! 编辑会话 Module：文档与 overlay 状态、诊断记账、分析快照与编译会话生命周期。

use crate::lang::lsp::protocol::uri_to_path;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use crate::lang::compiler::{CheckOutput, CompilerSession, CompilerSystem, DocumentOutput, modules};
use crate::lang::compiler::component_source::{self, ComponentOutput};
mod components;

// 编辑器只保留工具所需的来源与符号，及时释放运行执行产物。
#[derive(Debug)]
pub(crate) struct ModuleAnalysis {
    pub source_graph: crate::lang::compiler::source_graph::SourceGraph,
    pub symbols: Vec<modules::ModuleSymbol>,
}

#[derive(Debug)]
pub(crate) enum OpenDocument {
    // 文件文档的源码只由 overlays 拥有，文档索引不保存第二份 String。
    File(PathBuf),
    // 非 file URI 没有可传给 CompilerSession 的稳定路径，继续由 LSP 会话拥有源码。
    Virtual(String),
}

// 持有编辑器会话的全部状态；编译会话同样由本 Module 独占。
#[derive(Debug, Default)]
pub(crate) struct Session {
    // 打开文档索引：URI 到文件路径或虚拟源码。
    documents: BTreeMap<String, OpenDocument>,
    // LSP Adapter 持久拥有唯一文件源码快照，诊断请求直接借用而不重建整表。
    overlays: BTreeMap<PathBuf, String>,
    // 按根 URI 记账的已发布诊断，供关闭与覆盖时汇聚清理。
    diagnostics_by_root: BTreeMap<String, BTreeMap<String, Vec<Value>>>,
    // 按根 URI 缓存的最近一次成功分析，供导航与悬停复用。
    analyses: BTreeMap<String, CheckOutput>,
    // 最多缓存 32 份模块分析；失效后保留有界根闭包身份以定位依赖编辑上下文。
    module_analyses: BTreeMap<String, Arc<ModuleAnalysis>>,
    module_roots: BTreeMap<String, (PathBuf, BTreeSet<PathBuf>)>,
    component_analyses: BTreeMap<String, Arc<ComponentOutput>>,
    component_roots: BTreeMap<String, (PathBuf, BTreeSet<PathBuf>)>,
    // LSP Adapter 独占编译会话；进程退出或文档关闭时释放对应阶段缓存。
    compiler: CompilerSession,
    // shutdown 已收到但尚未 exit；exit 是否已经请求。
    shutdown: bool,
    exit: bool,
}

impl Session {
    pub(crate) fn declaration_uris(&self) -> Vec<String> {
        self.documents
            .keys()
            .filter(|uri| {
                self.document_source(uri)
                    .is_some_and(|source| modules::recognizes_source(source) || component_source::recognizes_source(source))
            })
            .cloned()
            .collect()
    }

    pub(crate) fn check_document_with_overlays(
        &mut self,
        path: &Path,
    ) -> Result<DocumentOutput, crate::lang::compiler::CompilerDiagnostic> {
        let source = self
            .overlays
            .get(path)
            .cloned()
            .or_else(|| component_source::document_prefix(path).ok())
            .unwrap_or_default();
        if component_source::recognizes_source(&source) {
            CompilerSystem::new().check_component_file_with_overlays(path, &self.overlays).map(DocumentOutput::Component)
        } else if modules::recognizes_source(&source) {
            let root = self
                .module_roots
                .values()
                .find(|(root, files)| {
                    !same_document_path(root, path)
                        && files.iter().any(|file| same_document_path(file, path))
                })
                .map(|(root, _)| root.clone())
                .unwrap_or_else(|| path.to_path_buf());
            modules::check_file_with_overlays(&root, &self.overlays).map(DocumentOutput::Module)
        } else {
            self.check_file_with_overlays_auto(path)
                .map(DocumentOutput::Ui)
        }
    }

    pub(crate) fn set_module_analysis(&mut self, uri: &str, output: modules::ModuleOutput) {
        self.forget_component_root(uri);
        if self.module_analyses.len() >= 32 && !self.module_analyses.contains_key(uri) {
            if let Some(oldest) = self.module_analyses.keys().next().cloned() {
                self.module_analyses.remove(&oldest);
            }
        }
        if self.module_roots.len() >= 32 && !self.module_roots.contains_key(uri) {
            if let Some(oldest) = self.module_roots.keys().next().cloned() {
                self.module_roots.remove(&oldest);
            }
        }
        let root = output
            .source_graph
            .file(output.source_graph.root())
            .unwrap();
        if Path::new(&root.path).is_absolute() {
            self.module_roots.insert(
                uri.to_string(),
                (
                    PathBuf::from(&root.path),
                    output
                        .source_graph
                        .files()
                        .iter()
                        .map(|file| PathBuf::from(&file.path))
                        .collect(),
                ),
            );
        }
        self.module_analyses.insert(
            uri.to_string(),
            Arc::new(ModuleAnalysis {
                source_graph: output.source_graph,
                symbols: output.symbols,
            }),
        );
        self.analyses.remove(uri);
    }

    pub(crate) fn module_analysis(&self, uri: &str) -> Option<Arc<ModuleAnalysis>> {
        self.module_analyses.get(uri).cloned().or_else(|| {
            let path = uri_to_path(uri)?;
            self.module_analyses
                .values()
                .find(|analysis| {
                    analysis
                        .source_graph
                        .files()
                        .iter()
                        .any(|file| same_document_path(Path::new(&file.path), &path))
                })
                .cloned()
        })
    }

    pub(crate) fn check_module(&mut self, uri: &str, source: &str) -> Option<Arc<ModuleAnalysis>> {
        let result = match uri_to_path(uri) {
            Some(path) => self.check_document_with_overlays(&path),
            None => CompilerSystem::new().check_document_inline(source, uri),
        };
        match result.ok()? {
            DocumentOutput::Module(output) => {
                self.set_module_analysis(uri, output);
                self.module_analysis(uri)
            }
            DocumentOutput::Ui(_) | DocumentOutput::Component(_) => None,
        }
    }

    pub(crate) fn forget_module_root(&mut self, uri: &str) {
        self.module_roots.remove(uri);
        self.module_analyses.remove(uri);
        if let Some(path) = uri_to_path(uri) {
            self.module_roots
                .retain(|_, (root, _)| !same_document_path(root, &path));
            self.module_analyses.retain(|_, analysis| {
                analysis
                    .source_graph
                    .file(analysis.source_graph.root())
                    .is_none_or(|root| !same_document_path(Path::new(&root.path), &path))
            });
        }
    }
    // 打开或更新一个文档；同一 URI 更新前先释放旧路径快照。
    pub(crate) fn store_document(&mut self, uri: String, source: String) {
        // URI 改类（file ↔ 虚拟）时不能残留旧类快照。
        self.remove_document(&uri);
        if let Some(path) = uri_to_path(&uri) {
            // 内容变化后覆盖该路径的旧分析全部失效。
            self.drop_analyses_covering(&path);
            self.overlays.insert(path.clone(), source);
            self.documents.insert(uri, OpenDocument::File(path));
        } else {
            self.drop_analyses_covering_uri(&uri);
            self.documents.insert(uri, OpenDocument::Virtual(source));
        }
    }

    // 关闭一个文档并返回其文件路径（虚拟文档返回 None）。
    pub(crate) fn remove_document(&mut self, uri: &str) -> Option<PathBuf> {
        match self.documents.remove(uri) {
            Some(OpenDocument::File(path)) => {
                // URI 别名仍指向同一路径时保留共享快照，直到最后一个文档所有者关闭。
                let still_open = self.documents.values().any(|document| {
                    matches!(document, OpenDocument::File(candidate) if candidate == &path)
                });
                if !still_open {
                    self.overlays.remove(&path);
                }
                Some(path)
            }
            Some(OpenDocument::Virtual(_)) | None => None,
        }
    }

    // 读取打开文档的当前源码；未打开的 URI 没有会话内源码。
    pub(crate) fn document_source(&self, uri: &str) -> Option<&str> {
        match self.documents.get(uri)? {
            OpenDocument::File(path) => self.overlays.get(path).map(String::as_str),
            OpenDocument::Virtual(source) => Some(source.as_str()),
        }
    }

    // 返回全部仍打开的文件文档（URI 与路径），供失效复核使用。
    pub(crate) fn open_file_documents(&self) -> Vec<(String, PathBuf)> {
        self.documents
            .iter()
            .filter_map(|(uri, document)| match document {
                OpenDocument::File(path) => Some((uri.clone(), path.clone())),
                OpenDocument::Virtual(_) => None,
            })
            .collect()
    }

    // 暴露只读 overlay 表，供检查入口借用。
    pub(crate) fn overlays(&self) -> &BTreeMap<PathBuf, String> {
        &self.overlays
    }

    // 使用编译会话检查真实文件（overlay 覆盖磁盘），自动选择目标。
    // 与 compiler 公开命令保持同一错误形状；大诊断按既有契约接受。
    #[allow(clippy::result_large_err)]
    pub(crate) fn check_file_with_overlays_auto(
        &mut self,
        path: &Path,
    ) -> Result<CheckOutput, crate::lang::compiler::CompilerDiagnostic> {
        self.compiler
            .check_file_with_overlays_auto(path, &self.overlays)
    }

    // 把当前文档作为根重新检查；成功结果缓存供后续特性请求复用。
    pub(crate) fn check_as_root(&mut self, uri: &str) -> Option<CheckOutput> {
        match self.documents.get(uri) {
            Some(OpenDocument::File(path)) => {
                // 借用文档路径调用会话缓存检查，成功后登记分析快照。
                let result = self
                    .compiler
                    .check_file_with_overlays_auto(path, &self.overlays);
                result.ok().inspect(|analysis| {
                    self.analyses.insert(uri.to_string(), analysis.clone());
                })
            }
            Some(OpenDocument::Virtual(source)) => {
                // 虚拟文档没有稳定路径，走内嵌检查入口。
                let result = CompilerSystem::new().check_inline_auto(source, uri);
                result.ok().inspect(|analysis| {
                    self.analyses.insert(uri.to_string(), analysis.clone());
                })
            }
            None => None,
        }
    }

    // 登记一次诊断流程产出的成功分析。
    pub(crate) fn set_analysis(&mut self, uri: &str, analysis: CheckOutput) {
        self.forget_module_root(uri);
        self.forget_component_root(uri);
        self.analyses.insert(uri.to_string(), analysis);
    }

    // 查询按 URI 缓存的分析。
    pub(crate) fn analysis(&self, uri: &str) -> Option<&CheckOutput> {
        self.analyses.get(uri)
    }

    // 汇集全部分析源码图内的文件快照（规范路径与源码），供补全扫描闭包。
    pub(crate) fn graph_file_sources(&self) -> Vec<(String, String)> {
        let mut seen = BTreeMap::new();
        for analysis in self.analyses.values() {
            for file in analysis.source_graph.files() {
                seen.entry(file.path.clone())
                    .or_insert_with(|| file.source.clone());
            }
        }
        seen.into_iter().collect()
    }

    // 查询源码图覆盖指定路径的分析；多个命中时任取稳定的一个。
    pub(crate) fn analysis_covering(&self, path: &Path) -> Option<&CheckOutput> {
        self.analyses
            .values()
            .find(|analysis| analysis_covering_path(analysis, path))
    }

    // 丢弃源码图覆盖指定路径的全部分析。
    pub(crate) fn drop_analyses_covering(&mut self, path: &Path) {
        self.component_analyses.retain(|_, output| !components::covers(output, path));
        self.module_analyses.retain(|_, analysis| {
            !analysis
                .source_graph
                .files()
                .iter()
                .any(|file| same_document_path(Path::new(&file.path), path))
        });
        let stale = self
            .analyses
            .keys()
            .filter(|uri| {
                self.analyses
                    .get(*uri)
                    .is_some_and(|analysis| analysis_covering_path(analysis, path))
            })
            .cloned()
            .collect::<Vec<_>>();
        for uri in stale {
            self.analyses.remove(&uri);
        }
    }

    // 丢弃虚拟文档对应的分析。
    pub(crate) fn drop_analyses_covering_uri(&mut self, uri: &str) {
        self.analyses.remove(uri);
        self.module_analyses.remove(uri);
        self.component_analyses.remove(uri);
    }

    // 释放直接或递归依赖指定文件的全部根流水线，并丢弃受影响分析。
    pub(crate) fn evict_file(&mut self, path: &Path) -> Vec<PathBuf> {
        let mut affected_roots = self.compiler.evict_file(path);
        affected_roots.extend(
            self.module_roots
                .values()
                .filter(|(_, files)| files.iter().any(|file| same_document_path(file, path)))
                .map(|(root, _)| root.clone()),
        );
        affected_roots.extend(self.component_roots.values()
            .filter(|(_, files)| files.iter().any(|file| same_document_path(file, path)))
            .map(|(root, _)| root.clone()));
        self.drop_analyses_covering(path);
        affected_roots
    }

    // 取出按根记账的已发布诊断。
    pub(crate) fn take_diagnostics(&mut self, uri: &str) -> BTreeMap<String, Vec<Value>> {
        self.diagnostics_by_root.remove(uri).unwrap_or_default()
    }

    // 覆盖按根记账的已发布诊断。
    pub(crate) fn set_diagnostics(&mut self, uri: &str, by_document: BTreeMap<String, Vec<Value>>) {
        self.diagnostics_by_root
            .insert(uri.to_string(), by_document);
    }

    // 移除按根记账的已发布诊断。
    pub(crate) fn remove_diagnostics(&mut self, uri: &str) {
        self.diagnostics_by_root.remove(uri);
    }

    // 查询其他根是否仍在发布针对指定文档的诊断。
    pub(crate) fn diagnostics_for_document(&self, document_uri: &str) -> Vec<&Vec<Value>> {
        self.diagnostics_by_root
            .values()
            .filter_map(|by_document| by_document.get(document_uri))
            .collect()
    }

    // 请求关闭前的协议握手；exit 之前仍可处理残余请求。
    pub(crate) fn request_shutdown(&mut self) {
        self.shutdown = true;
    }

    // 处理 exit 通知。
    pub(crate) fn request_exit(&mut self) {
        self.exit = true;
    }

    // exit 是否已被请求。
    pub(crate) fn exit_requested(&self) -> bool {
        self.exit
    }

    // LSP 规范：先 shutdown 再 exit 返回 0，否则返回 1。
    pub(crate) fn exit_code(&self) -> u8 {
        u8::from(!self.shutdown)
    }
}

// 判断一份分析的源码图是否覆盖指定路径。
fn analysis_covering_path(analysis: &CheckOutput, path: &Path) -> bool {
    analysis
        .source_graph
        .files()
        .iter()
        .any(|file| same_document_path(Path::new(&file.path), path))
}

// 判断两个路径是否指向同一文档；兼容符号链接与规范化差异。
pub(crate) fn same_document_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{OpenDocument, Session};

    #[test]
    fn file_documents_keep_one_persistent_overlay_source() {
        let first_path =
            std::env::temp_dir().join(format!("uix-lsp-overlay-first-{}.uix", std::process::id()));
        let second_path =
            std::env::temp_dir().join(format!("uix-lsp-overlay-second-{}.uix", std::process::id()));
        let first_uri = format!("file://{}", first_path.display());
        let second_uri = format!("file://{}", second_path.display());
        let mut session = Session::default();
        session.store_document(first_uri.clone(), "<App><Text>一</Text></App>".to_string());
        session.store_document(second_uri, "<App><Text>二</Text></App>".to_string());
        let first_pointer = session
            .document_source(&first_uri)
            .expect("打开文档必须可读")
            .as_ptr();
        assert_eq!(
            first_pointer,
            session
                .overlays
                .get(&first_path)
                .expect("文件文档必须持有 overlay")
                .as_ptr(),
            "文档索引与编译 overlay 必须借用同一份源码"
        );
        assert_eq!(session.overlays.len(), 2);
    }

    #[test]
    fn closing_one_uri_alias_keeps_shared_overlay_until_last_owner() {
        let path =
            std::env::temp_dir().join(format!("uix-lsp-overlay-alias-{}.uix", std::process::id()));
        let mut session = Session::default();
        session.overlays.insert(path.clone(), "<App />".to_string());
        session.documents.insert(
            "file:///alias-a.uix".to_string(),
            OpenDocument::File(path.clone()),
        );
        session.documents.insert(
            "file:///alias-b.uix".to_string(),
            OpenDocument::File(path.clone()),
        );

        session.remove_document("file:///alias-a.uix");
        assert!(session.overlays.contains_key(&path));
        session.remove_document("file:///alias-b.uix");
        assert!(!session.overlays.contains_key(&path));
    }

    #[test]
    fn storing_a_dependency_invalidates_root_analysis_covering_it() {
        // 真实导入闭包 fixture：根成功检查后，分析覆盖依赖路径。
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("系统时钟必须可用")
            .as_nanos();
        let fixture = std::env::temp_dir().join(format!("uix-lsp-invalidate-{unique}"));
        std::fs::create_dir(&fixture).expect("必须创建 LSP fixture");
        let helper = fixture.join("helper.uix");
        let root = fixture.join("main.uix");
        std::fs::write(
            &helper,
            "@export('Helper')\n<Widget name=\"Helper\"><Text>共享</Text></Widget>\n<Helper />",
        )
        .expect("必须写入依赖文件");
        std::fs::write(
            &root,
            "@import('./helper.uix', 'Helper')\n<App><Helper /></App>",
        )
        .expect("必须写入根文件");
        let root_uri = format!("file://{}", root.display());
        let helper_uri = format!("file://{}", helper.display());
        let mut session = Session::default();
        session.store_document(
            root_uri.clone(),
            std::fs::read_to_string(&root).expect("根必须可读"),
        );
        assert!(session.check_as_root(&root_uri).is_some(), "根必须通过检查");
        assert!(
            session.analysis_covering(&helper).is_some(),
            "成功分析必须覆盖依赖文件"
        );

        session.store_document(helper_uri, "<App><Unknown /></App>".to_string());

        assert!(
            session.analysis_covering(&helper).is_none(),
            "依赖内容变化后覆盖它的分析必须失效"
        );
        let _ = std::fs::remove_dir_all(fixture);
    }

    #[test]
    fn exit_code_follows_shutdown_handshake() {
        let mut session = Session::default();
        session.request_exit();
        // 未握手 shutdown 的 exit 属于异常退出。
        assert_eq!(session.exit_code(), 1);
        session.request_shutdown();
        assert_eq!(session.exit_code(), 0);
    }
}

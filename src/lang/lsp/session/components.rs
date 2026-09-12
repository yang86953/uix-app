//! 有界编辑器检查快照，只保留来源/语义，不生成或缓存执行程序。
use super::*;

pub(super) fn covers(output: &ComponentOutput, path: &Path) -> bool {
    output
        .checked
        .source()
        .source_graph
        .files()
        .iter()
        .any(|file| same_document_path(Path::new(&file.path), path))
        || output
            .interface_files
            .iter()
            .any(|file| same_document_path(file, path))
}
impl Session {
    pub(crate) fn is_component_document(&self, uri: &str, source: &str) -> bool {
        component_source::editor_source_hint(source).unwrap_or_else(|| {
            self.component_documents.contains(uri)
                || uri_to_path(uri).is_some_and(|path| self.is_component_path(&path, source))
        })
    }
    pub(super) fn is_component_path(&self, path: &Path, source: &str) -> bool {
        component_source::editor_source_hint(source).unwrap_or_else(|| self.component_documents.iter().any(|uri|
            matches!(self.documents.get(uri), Some(OpenDocument::File(candidate)) if same_document_path(path, candidate))))
    }
    pub(crate) fn component_edit_root(&self, path: &Path) -> PathBuf {
        self.component_roots
            .values()
            .filter(|(_, files)| files.iter().any(|file| same_document_path(file, path)))
            .max_by_key(|(_, files)| files.len())
            .map_or_else(|| path.to_path_buf(), |(root, _)| root.clone())
    }
    pub(crate) fn component_analysis(
        &mut self,
        uri: &str,
        source: &str,
    ) -> Option<Arc<ComponentOutput>> {
        let path = uri_to_path(uri);
        // 导入文件优先复用覆盖它的完整根闭包，使定义与引用具有同一来源上下文。
        let cached = self
            .component_analyses
            .iter()
            .filter(|(key, output)| {
                *key == uri || path.as_ref().is_some_and(|path| covers(output, path))
            })
            .max_by_key(|(_, output)| output.checked.source().source_graph.files().len())
            .map(|(_, output)| output.clone());
        if cached.is_some() {
            return cached;
        }
        let output = if let Some(path) = path {
            let root = self
                .component_roots
                .values()
                .filter(|(_, files)| files.iter().any(|file| same_document_path(file, &path)))
                .max_by_key(|(_, files)| files.len())
                .map_or(path, |(root, _)| root.clone());
            CompilerSystem::new()
                .check_component_file_with_overlays(&root, &self.overlays)
                .ok()?
        } else {
            CompilerSystem::new()
                .check_component_inline(source, uri, &BTreeMap::new())
                .ok()?
        };
        self.set_component_analysis(uri, output);
        self.component_analyses.get(uri).cloned()
    }
    pub(crate) fn set_component_analysis(&mut self, uri: &str, output: ComponentOutput) {
        self.forget_module_root(uri);
        self.analyses.remove(uri);
        if self.component_analyses.len() >= 32 && !self.component_analyses.contains_key(uri) {
            if let Some(first) = self.component_analyses.keys().next().cloned() {
                self.component_analyses.remove(&first);
            }
        }
        if self.component_roots.len() >= 32 && !self.component_roots.contains_key(uri) {
            if let Some(first) = self.component_roots.keys().next().cloned() {
                self.component_roots.remove(&first);
            }
        }
        let graph = &output.checked.source().source_graph;
        if let Some(root) = graph
            .file(graph.root())
            .filter(|root| Path::new(&root.path).is_absolute())
        {
            let files = graph
                .files()
                .iter()
                .map(|file| PathBuf::from(&file.path))
                .chain(output.interface_files.iter().cloned())
                .collect();
            self.component_roots
                .insert(uri.into(), (PathBuf::from(&root.path), files));
        }
        self.component_analyses.insert(uri.into(), Arc::new(output));
    }
    pub(crate) fn forget_component_root(&mut self, uri: &str) {
        self.component_roots.remove(uri);
        self.component_analyses.remove(uri);
        if let Some(path) = uri_to_path(uri) {
            self.component_roots
                .retain(|_, (root, _)| !same_document_path(root, &path));
            self.component_analyses.retain(|_, output| {
                let graph = &output.checked.source().source_graph;
                graph
                    .file(graph.root())
                    .is_none_or(|root| !same_document_path(Path::new(&root.path), &path))
            });
        }
    }
}

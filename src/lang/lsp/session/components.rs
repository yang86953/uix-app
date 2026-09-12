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

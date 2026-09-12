//! 从真实检查过程产生的词法范围，不用 owner 相同或同名文本推断可见性。
use super::symbols::SymbolTarget;
use super::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
mod context;
mod query;
pub use query::{Completion, CompletionItem, CompletionKind, declaration_completion};

/// 编辑器专用只读事实。可携带语义失败，不是 CheckedSource，不能生成或执行程序。
#[derive(Debug)]
pub struct Inspection {
    pub(super) linked: LinkedSource,
    pub(super) scopes: Vec<LexicalScope>,
    pub(super) bindings: BTreeMap<NodeId, BindingFact>,
    pub(super) types: BTreeMap<NodeId, Type>,
    pub(super) components: BTreeMap<DeclarationId, ComponentSignature>,
    pub(super) functions: BTreeMap<NodeId, FunctionSignature>,
    pub(super) native_imports: NativeLibraries,
    pub(super) diagnostic: Option<CompilerDiagnostic>,
}
impl Inspection {
    pub fn source(&self) -> &LinkedSource {
        &self.linked
    }
    pub fn scopes(&self) -> &[LexicalScope] {
        &self.scopes
    }
    pub fn diagnostic(&self) -> Option<&CompilerDiagnostic> {
        self.diagnostic.as_ref()
    }
}

pub fn inspect_inline(
    source: &str,
    name: &str,
    libraries: &NativeLibraries,
) -> Result<Inspection, CompilerDiagnostic> {
    Ok(check::inspect(link_inline(source, name)?, libraries))
}
pub fn inspect_file_with_overlays(
    path: &Path,
    overlays: &BTreeMap<PathBuf, String>,
) -> Result<Inspection, CompilerDiagnostic> {
    let interfaces = interface::load_project(path)?;
    Ok(check::inspect(
        link_file_with_overlays(path, overlays)?,
        &interfaces.libraries,
    ))
}

#[derive(Debug, Clone)]
pub struct ScopeChange {
    pub at: usize,
    pub name: String,
    /// None 隐藏外层/全局同名绑定，例如尚未绑定的默认参数。
    pub target: Option<SymbolTarget>,
}

#[derive(Debug, Clone)]
pub struct LexicalScope {
    pub range: NodeId,
    pub(super) initial: BTreeMap<String, Option<SymbolTarget>>,
    pub(super) changes: Vec<ScopeChange>,
    pub(super) finished: bool,
    pub(super) checked_until: usize,
}
impl LexicalScope {
    pub fn checked_until(&self) -> usize {
        self.checked_until
    }
    pub fn is_finished(&self) -> bool {
        self.finished
    }
    pub fn visible_at(&self, offset: usize) -> BTreeMap<String, Option<SymbolTarget>> {
        let mut visible = self.initial.clone();
        for change in self.changes.iter().filter(|change| change.at <= offset) {
            visible.insert(change.name.clone(), change.target.clone());
        }
        visible
    }
    /// JSX 名称按现有检查器只被实际局部值遮蔽，未绑定参数禁用仅适用于值读取。
    pub fn variables_at(&self, offset: usize) -> BTreeMap<String, SymbolTarget> {
        let mut visible = self
            .initial
            .iter()
            .filter_map(|(name, target)| {
                target.as_ref().map(|target| (name.clone(), target.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        for change in self.changes.iter().filter(|change| change.at <= offset) {
            if let Some(target) = &change.target {
                visible.insert(change.name.clone(), target.clone());
            }
        }
        visible
    }
}

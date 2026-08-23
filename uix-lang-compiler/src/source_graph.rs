//! UIX Lang 源文件、内容摘要与递归导入关系。

use std::{collections::BTreeMap, path::Path};

/// 一个编译图内稳定且可排序的源码身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId(u64);

impl SourceId {
    /// 从规范路径或内嵌来源标签建立稳定身份。
    pub fn from_source_name(source_name: &str) -> Self {
        Self(stable_hash(source_name.replace('\\', "/").as_bytes()))
    }

    /// 返回可用于缓存键和协议传输的原始身份。
    pub const fn value(self) -> u64 {
        self.0
    }

    // 从编译器内部来源标记恢复稳定身份，不向外暴露任意身份构造。
    pub(crate) const fn from_value(value: u64) -> Self {
        Self(value)
    }
}

/// 保存 Compiler System 实际读取的一份 UTF-8 源文件快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub id: SourceId,
    pub path: String,
    pub content_hash: u64,
    pub source: String,
}

/// 保存一条经过规范路径解析的有向导入边。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportEdge {
    pub importer: SourceId,
    pub imported: SourceId,
    pub selected_widget: Option<String>,
    pub line: usize,
    pub column: usize,
}

/// 保存根优先、无环且可确定重建的源码闭包。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceGraph {
    root: SourceId,
    files: Vec<SourceFile>,
    imports: Vec<ImportEdge>,
}

impl SourceGraph {
    /// 为没有文件系统导入基准的内嵌源码建立单节点图。
    pub fn inline(source_name: impl Into<String>, source: impl Into<String>) -> Self {
        let source_name = source_name.into();
        let source = source.into();
        let id = SourceId::from_source_name(&source_name);
        Self {
            root: id,
            files: vec![SourceFile {
                id,
                path: source_name,
                content_hash: stable_hash(source.as_bytes()),
                source,
            }],
            imports: Vec::new(),
        }
    }

    /// 返回根文件稳定身份。
    pub const fn root(&self) -> SourceId {
        self.root
    }

    /// 返回根优先首次读取顺序中的文件快照。
    pub fn files(&self) -> &[SourceFile] {
        &self.files
    }

    /// 返回源码顺序中的直接导入边。
    pub fn imports(&self) -> &[ImportEdge] {
        &self.imports
    }

    /// 按稳定身份查询源码快照。
    pub fn file(&self, id: SourceId) -> Option<&SourceFile> {
        self.files.iter().find(|file| file.id == id)
    }

    /// 组合全部内容摘要，形成递归依赖缓存键。
    pub fn dependency_hash(&self) -> u64 {
        let mut bytes = Vec::with_capacity(self.files.len() * 16);
        for file in &self.files {
            bytes.extend_from_slice(&file.id.0.to_le_bytes());
            bytes.extend_from_slice(&file.content_hash.to_le_bytes());
        }
        stable_hash(&bytes)
    }
}

/// 在 Source Module 内按真实读取顺序构建图。
#[derive(Debug)]
pub(crate) struct SourceGraphBuilder {
    root_path: String,
    files: BTreeMap<String, SourceFile>,
    order: Vec<String>,
    imports: Vec<ImportEdge>,
}

impl SourceGraphBuilder {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            root_path: normalized_path(root),
            files: BTreeMap::new(),
            order: Vec::new(),
            imports: Vec::new(),
        }
    }

    pub(crate) fn insert_file(&mut self, path: &Path, source: &str) {
        let path = normalized_path(path);
        if self.files.contains_key(&path) {
            return;
        }
        let id = source_id(&path);
        self.order.push(path.clone());
        self.files.insert(
            path.clone(),
            SourceFile {
                id,
                path,
                content_hash: stable_hash(source.as_bytes()),
                source: source.to_owned(),
            },
        );
    }

    pub(crate) fn add_import(
        &mut self,
        importer: &Path,
        imported: &Path,
        selected_widget: Option<String>,
        line: usize,
        column: usize,
    ) {
        self.imports.push(ImportEdge {
            importer: source_id(&normalized_path(importer)),
            imported: source_id(&normalized_path(imported)),
            selected_widget,
            line,
            column,
        });
    }

    pub(crate) fn finish(self) -> SourceGraph {
        let files = self
            .order
            .into_iter()
            .filter_map(|path| self.files.get(&path).cloned())
            .collect();
        SourceGraph {
            root: source_id(&self.root_path),
            files,
            imports: self.imports,
        }
    }
}

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn source_id(path: &str) -> SourceId {
    SourceId::from_source_name(path)
}

fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::SourceGraph;

    #[test]
    fn inline_graph_has_stable_identity_and_dependency_hash() {
        let first = SourceGraph::inline("demo.uix", "<Text>Hello</Text>");
        let second = SourceGraph::inline("demo.uix", "<Text>Hello</Text>");
        assert_eq!(first.root(), second.root());
        assert_eq!(first.dependency_hash(), second.dependency_hash());
        assert_ne!(
            first.dependency_hash(),
            SourceGraph::inline("demo.uix", "<Text>Changed</Text>").dependency_hash()
        );
    }
}

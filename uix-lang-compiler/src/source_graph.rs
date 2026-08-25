//! UIX Lang 源文件、内容摘要与递归导入关系。

use std::{collections::BTreeMap, path::Path, sync::Arc};

const STABLE_HASH_OFFSET: u64 = 0xcbf29ce484222325;
const STABLE_HASH_PRIME: u64 = 0x100000001b3;

/// 一个编译图内稳定且可排序的源码身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId(u64);

impl SourceId {
    /// 从规范路径或内嵌来源标签建立稳定身份。
    pub fn from_source_name(source_name: &str) -> Self {
        // 逐字节规范化路径分隔符，避免只为哈希构造临时 String。
        let hash = source_name.bytes().fold(STABLE_HASH_OFFSET, |hash, byte| {
            extend_stable_hash(hash, if byte == b'\\' { b'/' } else { byte })
        });
        Self(hash)
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
    // 文件快照构建后不可变，跨公开检查结果共享同一拥有型序列。
    files: Arc<[SourceFile]>,
    // 导入边构建后不可变，克隆源码图时只复制共享句柄。
    imports: Arc<[ImportEdge]>,
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
            }]
            .into(),
            imports: Vec::new().into(),
        }
    }

    /// 返回根文件稳定身份。
    pub const fn root(&self) -> SourceId {
        self.root
    }

    /// 返回根优先首次读取顺序中的文件快照。
    pub fn files(&self) -> &[SourceFile] {
        self.files.as_ref()
    }

    /// 返回源码顺序中的直接导入边。
    pub fn imports(&self) -> &[ImportEdge] {
        self.imports.as_ref()
    }

    /// 按稳定身份查询源码快照。
    pub fn file(&self, id: SourceId) -> Option<&SourceFile> {
        self.files.iter().find(|file| file.id == id)
    }

    /// 组合全部内容摘要，形成递归依赖缓存键。
    pub fn dependency_hash(&self) -> u64 {
        // 直接延续同一 FNV-1a 状态，保持既有字节序列与哈希值但不分配临时 Vec。
        let mut hash = STABLE_HASH_OFFSET;
        for file in self.files.iter() {
            for byte in file.id.0.to_le_bytes() {
                hash = extend_stable_hash(hash, byte);
            }
            for byte in file.content_hash.to_le_bytes() {
                hash = extend_stable_hash(hash, byte);
            }
        }
        hash
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
        // 构建器已被消费，按读取顺序移出 SourceFile，避免克隆其中的完整源码 String。
        let Self {
            root_path,
            mut files,
            order,
            imports,
        } = self;
        let files: Vec<_> = order
            .into_iter()
            .filter_map(|path| files.remove(&path))
            .collect();
        SourceGraph {
            root: source_id(&root_path),
            files: files.into(),
            imports: imports.into(),
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
    let mut hash = STABLE_HASH_OFFSET;
    for byte in bytes {
        hash = extend_stable_hash(hash, *byte);
    }
    hash
}

#[inline]
fn extend_stable_hash(hash: u64, byte: u8) -> u64 {
    (hash ^ u64::from(byte)).wrapping_mul(STABLE_HASH_PRIME)
}

#[cfg(test)]
mod tests {
    use super::{SourceGraph, SourceGraphBuilder, normalized_path, stable_hash};
    use std::path::Path;
    use std::sync::Arc;

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

    #[test]
    fn dependency_hash_matches_the_original_concatenated_bytes() {
        let graph = SourceGraph::inline("demo.uix", "<Text>Hello</Text>");
        let mut bytes = Vec::new();
        for file in graph.files() {
            bytes.extend_from_slice(&file.id.value().to_le_bytes());
            bytes.extend_from_slice(&file.content_hash.to_le_bytes());
        }
        assert_eq!(graph.dependency_hash(), stable_hash(&bytes));
    }

    #[test]
    fn clone_shares_immutable_source_snapshot_storage() {
        let graph = SourceGraph::inline("demo.uix", "<Text>Hello</Text>");
        let cloned = graph.clone();

        // 公开值语义保持相等，内部不可变快照不再深拷贝源码字符串。
        assert_eq!(graph, cloned);
        assert!(Arc::ptr_eq(&graph.files, &cloned.files));
        assert!(Arc::ptr_eq(&graph.imports, &cloned.imports));
    }

    #[test]
    fn builder_finish_moves_source_allocations_in_read_order() {
        let root = Path::new("root.uix");
        let helper = Path::new("helper.uix");
        let mut builder = SourceGraphBuilder::new(root);
        builder.insert_file(root, "<App />");
        builder.insert_file(helper, "<Widget name=\"Helper\" />");
        let root_key = normalized_path(root);
        let source_pointer = builder
            .files
            .get(&root_key)
            .expect("构建器必须持有根源码")
            .source
            .as_ptr();

        let graph = builder.finish();

        assert_eq!(graph.files()[0].path, root_key);
        assert_eq!(graph.files()[0].source.as_ptr(), source_pointer);
        assert_eq!(graph.files()[1].path, normalized_path(helper));
    }
}

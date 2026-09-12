//! `uix-lang-compiler/source_graph.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

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

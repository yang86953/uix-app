//! `uix-lang-compiler/source_map.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::SourceMap;
use crate::CompileTarget;
use crate::semantic_ir::lower_document;
use crate::source_graph::{SourceGraph, SourceId};
use crate::uix_lang::parse_document;

#[test]
fn source_map_resolves_marker_to_attribute_semantic_node() {
    let source = "<Button @click=\"save()\">Save</Button>";
    let graph = SourceGraph::inline("demo.uix", source);
    let source_id = graph.root();
    let document = parse_document(source).expect("测试源码必须可解析");
    let ir = lower_document(document, CompileTarget::View, source_id, &[])
        .expect("测试源码必须可降低");
    let mapped = SourceMap::from_marked("{ __uix_source_marker_1_16 save ( ) }", &graph, &ir);
    assert!(!mapped.source.contains("__uix_source_marker"));
    let entry = &mapped.source_map.entries()[0];
    assert_eq!(entry.source_id, SourceId::from_source_name("demo.uix"));
    assert!(
        entry
            .semantic_node_id
            .as_deref()
            .is_some_and(|id| id.starts_with("attribute."))
    );
}

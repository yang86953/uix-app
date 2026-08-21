//! Rust token 文本到 UIX 源码与语义节点的确定映射。

use crate::semantic_ir::TypedUiIr;
use crate::source_graph::{SourceGraph, SourceId};

const MARKER_PREFIX: &str = "__uix_source_marker_";

/// 保存一段生成 token 文本的 UIX 来源。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMapEntry {
    pub generated_start: usize,
    pub generated_end: usize,
    pub source_id: SourceId,
    pub source_name: String,
    pub source_start: usize,
    pub line: usize,
    pub column: usize,
    pub semantic_node_id: Option<String>,
}

/// 保存按生成位置排序且覆盖全部标记片段的来源表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMap {
    entries: Vec<SourceMapEntry>,
}

impl SourceMap {
    /// 返回生成顺序中的全部映射条目。
    pub fn entries(&self) -> &[SourceMapEntry] {
        &self.entries
    }

    /// 按 token 文本字节位置查询最近的来源条目。
    pub fn lookup_generated(&self, offset: usize) -> Option<&SourceMapEntry> {
        self.entries
            .iter()
            .find(|entry| offset >= entry.generated_start && offset < entry.generated_end)
    }

    pub(crate) fn from_generated(
        generated: &str,
        source_graph: &SourceGraph,
        ir: &TypedUiIr,
    ) -> Self {
        let mut markers = marker_positions(generated);
        if markers.is_empty() {
            markers.push((0, 1, 1));
        }
        let mut entries = Vec::with_capacity(markers.len());
        for (index, (generated_start, line, column)) in markers.iter().copied().enumerate() {
            let generated_end = markers
                .get(index + 1)
                .map(|entry| entry.0)
                .unwrap_or(generated.len());
            let (source_id, source_name, source_start, semantic_node_id) =
                resolve_source(source_graph, ir, line, column);
            entries.push(SourceMapEntry {
                generated_start,
                generated_end,
                source_id,
                source_name,
                source_start,
                line,
                column,
                semantic_node_id,
            });
        }
        Self { entries }
    }
}

fn marker_positions(generated: &str) -> Vec<(usize, usize, usize)> {
    let mut markers = Vec::new();
    let mut cursor = 0usize;
    while let Some(relative) = generated[cursor..].find(MARKER_PREFIX) {
        let start = cursor + relative;
        let numbers = start + MARKER_PREFIX.len();
        let Some(separator_relative) = generated[numbers..].find('_') else {
            break;
        };
        let separator = numbers + separator_relative;
        let column_start = separator + 1;
        let column_end = generated[column_start..]
            .find(|character: char| !character.is_ascii_digit())
            .map(|relative| column_start + relative)
            .unwrap_or(generated.len());
        let Ok(line) = generated[numbers..separator].parse::<usize>() else {
            cursor = column_end;
            continue;
        };
        let Ok(column) = generated[column_start..column_end].parse::<usize>() else {
            cursor = column_end;
            continue;
        };
        markers.push((start, line, column));
        cursor = column_end;
    }
    markers
}

fn resolve_source(
    source_graph: &SourceGraph,
    ir: &TypedUiIr,
    line: usize,
    column: usize,
) -> (SourceId, String, usize, Option<String>) {
    let root = source_graph.root();
    let mut fallback = None;
    for file in source_graph.files() {
        let Some(offset) = offset_at(&file.source, line, column) else {
            continue;
        };
        if file.id == root {
            fallback = Some((file.id, file.path.clone(), offset));
        }
        if let Some(node) = ir.semantic_node_at(file.id, offset) {
            return (file.id, file.path.clone(), offset, Some(node.id));
        }
    }
    if let Some((source_id, source_name, source_start)) = fallback {
        return (source_id, source_name, source_start, None);
    }
    let source_name = source_graph
        .file(root)
        .map(|file| file.path.clone())
        .unwrap_or_else(|| "<unknown>".to_string());
    (root, source_name, 0, None)
}

fn offset_at(source: &str, target_line: usize, target_column: usize) -> Option<usize> {
    if target_line == 0 || target_column == 0 {
        return None;
    }
    let mut line = 1usize;
    let mut column = 1usize;
    for (offset, character) in source.char_indices() {
        if line == target_line && column == target_column {
            return Some(offset);
        }
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line == target_line && column == target_column).then_some(source.len())
}

#[cfg(test)]
mod tests {
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
        let map = SourceMap::from_generated("{ __uix_source_marker_1_16 save ( ) }", &graph, &ir);
        let entry = &map.entries()[0];
        assert_eq!(entry.source_id, SourceId::from_source_name("demo.uix"));
        assert!(
            entry
                .semantic_node_id
                .as_deref()
                .is_some_and(|id| id.starts_with("attribute."))
        );
    }
}

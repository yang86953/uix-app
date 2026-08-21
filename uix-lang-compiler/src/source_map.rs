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

/// 保存已移除内部 marker 的合法 Rust token 文本及对应映射。
pub(crate) struct MappedGenerated {
    pub(crate) source: String,
    pub(crate) source_map: SourceMap,
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

    pub(crate) fn from_marked(
        marked: &str,
        source_graph: &SourceGraph,
        ir: &TypedUiIr,
    ) -> MappedGenerated {
        let markers = marker_positions(marked);
        if markers.is_empty() {
            let (source_id, source_name, source_start, semantic_node_id) =
                resolve_source(source_graph, ir, 1, 1);
            return MappedGenerated {
                source: marked.to_string(),
                source_map: Self {
                    entries: vec![SourceMapEntry {
                        generated_start: 0,
                        generated_end: marked.len(),
                        source_id,
                        source_name,
                        source_start,
                        line: 1,
                        column: 1,
                        semantic_node_id,
                    }],
                },
            };
        }
        let mut source = String::with_capacity(marked.len());
        let mut entries: Vec<SourceMapEntry> = Vec::with_capacity(markers.len());
        let mut cursor = 0usize;
        for marker in markers {
            source.push_str(&marked[cursor..marker.start]);
            if let Some(previous) = entries.last_mut() {
                previous.generated_end = source.len();
            }
            let (source_id, source_name, source_start, semantic_node_id) =
                resolve_source(source_graph, ir, marker.line, marker.column);
            entries.push(SourceMapEntry {
                generated_start: source.len(),
                generated_end: 0,
                source_id,
                source_name,
                source_start,
                line: marker.line,
                column: marker.column,
                semantic_node_id,
            });
            cursor = marker.end;
        }
        source.push_str(&marked[cursor..]);
        if let Some(previous) = entries.last_mut() {
            previous.generated_end = source.len();
        }
        MappedGenerated {
            source,
            source_map: Self { entries },
        }
    }
}

struct Marker {
    start: usize,
    end: usize,
    line: usize,
    column: usize,
}

fn marker_positions(generated: &str) -> Vec<Marker> {
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
        markers.push(Marker {
            start,
            end: column_end,
            line,
            column,
        });
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
    let mut best: Option<(usize, SourceId, String, usize, String)> = None;
    for file in source_graph.files() {
        let Some(offset) = offset_at(&file.source, line, column) else {
            continue;
        };
        if file.id == root {
            fallback = Some((file.id, file.path.clone(), offset));
        }
        if let Some(node) = ir.semantic_node_at(file.id, offset) {
            let length = node.span.end.saturating_sub(node.span.start);
            if best
                .as_ref()
                .is_none_or(|(best_length, ..)| length < *best_length)
            {
                best = Some((length, file.id, file.path.clone(), offset, node.id));
            }
        }
    }
    if let Some((_, source_id, source_name, source_start, semantic_node_id)) = best {
        return (source_id, source_name, source_start, Some(semantic_node_id));
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
}

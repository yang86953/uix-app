//! 基于无损 CST 的保守结构格式化与 AST 等价校验。

use crate::lossless_cst::{CstTokenKind, LosslessCst};
use crate::source_graph::SourceId;
use crate::uix_lang::{Declaration, Diagnostic, Document, Node, SourceSpan, parse_document};

#[derive(Debug)]
pub(crate) struct FormattedSource {
    pub(crate) formatted: String,
    pub(crate) cst: LosslessCst,
}

#[derive(Debug)]
pub(crate) struct FormatFailure {
    pub(crate) diagnostic: Diagnostic,
    pub(crate) invariant: bool,
}

pub(crate) fn format_source(
    source: &str,
    source_id: SourceId,
) -> Result<FormattedSource, FormatFailure> {
    let parse = if crate::modules::recognizes_source(source) {
        crate::modules::parse_source
    } else {
        parse_document
    };
    let before = parse(source).map_err(|diagnostic| FormatFailure {
        diagnostic,
        invariant: false,
    })?;
    let original_cst = LosslessCst::from_source(source_id, source);
    let formatted = format_tag_trivia(&original_cst);
    let after = parse(&formatted).map_err(|diagnostic| FormatFailure {
        diagnostic,
        invariant: true,
    })?;
    if semantic_shape(before) != semantic_shape(after) {
        return Err(FormatFailure {
            diagnostic: Diagnostic::new(
                entry_span(),
                "格式化结果未保持 UIX AST 等价",
                "保留原文件并报告 formatter 输入以修复无损 CST 规则",
            ),
            invariant: true,
        });
    }
    let cst = LosslessCst::from_source(source_id, formatted.clone());
    Ok(FormattedSource { formatted, cst })
}

fn format_tag_trivia(cst: &LosslessCst) -> String {
    let tokens = cst.tokens();
    let mut output = String::with_capacity(cst.source().len());
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        if token.kind == CstTokenKind::Trivia
            && index > 0
            && index + 1 < tokens.len()
            && is_tag(tokens[index - 1].kind)
            && is_tag(tokens[index + 1].kind)
        {
            let indent = if tokens[index + 1].kind == CstTokenKind::ClosingTag {
                depth.saturating_sub(1)
            } else {
                depth
            };
            output.push('\n');
            output.push_str(&"  ".repeat(indent));
            continue;
        }
        if is_tag(token.kind) && index > 0 && is_tag(tokens[index - 1].kind) {
            let indent = if token.kind == CstTokenKind::ClosingTag {
                depth.saturating_sub(1)
            } else {
                depth
            };
            output.push('\n');
            output.push_str(&"  ".repeat(indent));
        }
        if token.kind == CstTokenKind::ClosingTag {
            depth = depth.saturating_sub(1);
        }
        output.push_str(cst.text(token));
        if token.kind == CstTokenKind::OpeningTag {
            depth += 1;
        }
    }
    output
}

const fn is_tag(kind: CstTokenKind) -> bool {
    matches!(
        kind,
        CstTokenKind::OpeningTag | CstTokenKind::ClosingTag | CstTokenKind::SelfClosingTag
    )
}

fn semantic_shape(mut document: Document) -> String {
    for declaration in &mut document.declarations {
        if let Declaration::Widget(widget) = declaration {
            normalize_nodes(&mut widget.children);
        }
    }
    normalize_nodes(&mut document.root.children);
    strip_source_spans(format!("{document:?}"))
}

fn normalize_nodes(nodes: &mut Vec<Node>) {
    nodes.retain(|node| !matches!(node, Node::Text(text) if text.value.trim().is_empty()));
    for node in nodes {
        if let Node::Element(element) = node {
            normalize_nodes(&mut element.children);
        }
    }
}

fn strip_source_spans(mut debug: String) -> String {
    const PREFIX: &str = "SourceSpan {";
    let mut search_start = 0usize;
    while let Some(relative) = debug[search_start..].find(PREFIX) {
        let start = search_start + relative;
        let Some(end_relative) = debug[start..].find('}') else {
            break;
        };
        let end = start + end_relative + 1;
        debug.replace_range(start..end, "SourceSpan");
        search_start = start + "SourceSpan".len();
    }
    debug
}

const fn entry_span() -> SourceSpan {
    SourceSpan {
        start: 0,
        end: 0,
        line: 1,
        column: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::format_source;
    use crate::source_graph::SourceId;

    #[test]
    fn formatter_is_idempotent_and_preserves_semantic_ast() {
        let source = "<Column><Text>A</Text><Button>B</Button></Column>";
        let source_id = SourceId::from_source_name("demo.uix");
        let first = format_source(source, source_id).expect("首次格式化必须成功");
        assert_eq!(
            first.formatted,
            "<Column>\n  <Text>A</Text>\n  <Button>B</Button>\n</Column>"
        );
        let second = format_source(&first.formatted, source_id).expect("二次格式化必须成功");
        assert_eq!(second.formatted, first.formatted);
        assert_eq!(second.cst.reconstruct(), second.formatted);
    }
}

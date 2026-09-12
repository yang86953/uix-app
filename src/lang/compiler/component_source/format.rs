//! 仅重排解析器确认的代码空白；文本/字面量/注释字节不交给第二个 lexer。
use super::*;
use crate::lang::compiler::{
    DiagnosticPhase, FormatOutput,
    lossless_cst::{CstToken, CstTokenKind, LosslessCst},
    source_graph::SourceId,
};
use parser::concrete::{Kind, Token, parse_concrete};
mod shape;

const MAX_OUTPUT: usize = 1_048_576;

pub fn format(source: &str, source_name: &str) -> Result<FormatOutput, CompilerDiagnostic> {
    let (mut before, tokens) = parse_concrete(source, source_name.into())?;
    validate_cover(source, &tokens, source_name)?;
    let formatted = render(source, &tokens).ok_or_else(|| {
        failure(
            source_name,
            "component-format-limit",
            "格式化结果超过组件单文件 1 MiB 上限",
        )
    })?;
    let (mut after, new_tokens) =
        parse_concrete(&formatted, source_name.into()).map_err(|error| {
            failure(
                source_name,
                "component-format-invariant",
                format!("格式化结果不能重新解析：{}", error.message),
            )
        })?;
    validate_cover(&formatted, &new_tokens, source_name)?;
    // 除 AST 外还核对具体字节，防止改变注释、转义写法或 String 子内容。
    if !token_contents(source, &tokens).eq(token_contents(&formatted, &new_tokens)) {
        return Err(failure(
            source_name,
            "component-format-invariant",
            "格式化改变了非空白 token",
        ));
    }
    shape::clear(&mut before);
    shape::clear(&mut after);
    if before.imports != after.imports || before.declarations != after.declarations {
        return Err(failure(
            source_name,
            "component-format-invariant",
            "格式化没有保持组件 AST 等价",
        ));
    }
    let source_id = SourceId::from_source_name(source_name);
    let cst = LosslessCst::from_tokens(
        source_id,
        formatted.clone(),
        new_tokens
            .into_iter()
            .map(|token| CstToken {
                kind: match token.kind {
                    Kind::Whitespace => CstTokenKind::Trivia,
                    Kind::LineComment => CstTokenKind::LineComment,
                    Kind::BlockComment => CstTokenKind::BlockComment,
                    Kind::Text => CstTokenKind::Text,
                    Kind::Literal => CstTokenKind::Literal,
                    _ => CstTokenKind::Code,
                },
                start: token.span.start,
                end: token.span.end,
                tag_name: None,
            })
            .collect(),
    );
    Ok(FormatOutput {
        source_name: source_name.into(),
        source_id,
        changed: source != formatted,
        formatted,
        cst,
    })
}

fn failure(name: &str, code: &'static str, message: impl Into<String>) -> CompilerDiagnostic {
    CompilerDiagnostic {
        code,
        phase: DiagnosticPhase::Emit,
        source_id: SourceId::from_source_name(name),
        source_name: name.into(),
        start: 0,
        end: 0,
        line: 1,
        column: 1,
        message: message.into(),
        suggestion: "保留原文件并修复格式化器；不写入不等价或超限的候选".into(),
    }
}
fn validate_cover(source: &str, tokens: &[Token], name: &str) -> Result<(), CompilerDiagnostic> {
    let mut end = 0;
    for token in tokens {
        if token.span.start != end
            || token.span.end <= end
            || !source.is_char_boundary(token.span.end)
        {
            return Err(failure(
                name,
                "component-format-invariant",
                "具体语法没有连续覆盖原始来源",
            ));
        }
        end = token.span.end;
    }
    if end != source.len() {
        return Err(failure(
            name,
            "component-format-invariant",
            "具体语法遗漏来源字节",
        ));
    }
    Ok(())
}
fn token_contents<'a>(
    source: &'a str,
    tokens: &'a [Token],
) -> impl Iterator<Item = (Kind, &'a str)> {
    tokens
        .iter()
        .filter(|token| token.kind != Kind::Whitespace)
        .map(|token| (token.kind, text(source, token)))
}
fn text<'a>(source: &'a str, token: &Token) -> &'a str {
    &source[token.span.start..token.span.end]
}

#[derive(Clone, Copy)]
enum Gap {
    None,
    Space,
    Line,
}

fn gap(source: &str, previous: &Token, current: &Token, inline_semicolon: bool) -> Gap {
    use Kind::*;
    let left = text(source, previous);
    let right = text(source, current);
    // 所有 JSX 文本原样保留；不能往标签/子表达式之间新增看不见的 String 内容。
    if previous.kind == Text || current.kind == Text {
        return Gap::None;
    }
    if previous.kind == LineComment {
        return Gap::Line;
    }
    if matches!(previous.kind, LineComment | BlockComment)
        || matches!(current.kind, LineComment | BlockComment)
    {
        if source[previous.span.end..current.span.start].contains('\n') {
            return Gap::Line;
        }
    }
    if current.kind == BlockClose {
        return if previous.kind == BlockOpen {
            Gap::None
        } else {
            Gap::Line
        };
    }
    if previous.kind == BlockOpen {
        return Gap::Line;
    }
    if previous.kind == BlockClose {
        return if right == "else" {
            Gap::Space
        } else if matches!(right, ";" | "," | ")" | "]" | "(") || current.kind == ChildClose {
            Gap::None
        } else {
            Gap::Line
        };
    }
    if previous.kind == Code && left == ";" {
        return if inline_semicolon {
            Gap::Space
        } else {
            Gap::Line
        };
    }
    if current.kind == BlockOpen {
        return Gap::Space;
    }
    if matches!(current.kind, CloseTagStart) {
        return Gap::None;
    }
    if matches!(previous.kind, TagStart | CloseTagStart)
        || matches!(current.kind, TagEnd | CloseTagEnd)
    {
        return Gap::None;
    }
    if matches!(previous.kind, TagEnd | SelfClose | ChildClose | CloseTagEnd)
        && matches!(current.kind, TagStart | ChildOpen)
    {
        return Gap::None;
    }
    if current.kind == SelfClose {
        return Gap::Space;
    }
    if current.kind == AttributeEquals || previous.kind == AttributeEquals {
        return Gap::None;
    }
    if previous.kind == ChildOpen || current.kind == ChildClose {
        return Gap::None;
    }
    if previous.kind == GenericOpen || current.kind == GenericClose || current.kind == GenericOpen {
        return Gap::None;
    }
    if previous.kind == Unary {
        return if left == "-" && current.kind == Unary && right == "-" {
            Gap::Space
        } else {
            Gap::None
        };
    }
    if matches!(right, ";" | "," | ")" | "]" | "." | ":") {
        return Gap::None;
    }
    if matches!(left, "(" | "[" | ".") {
        return Gap::None;
    }
    if right == "(" {
        let callable = previous.kind == Literal
            || matches!(left, ")" | "]" | "}")
            || (previous.kind == Code
                && left
                    .chars()
                    .last()
                    .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
                && !matches!(left, "if" | "return"));
        return if callable { Gap::None } else { Gap::Space };
    }
    if right == "[" {
        return if current.kind == ArrayOpen {
            Gap::Space
        } else {
            Gap::None
        };
    }
    if left == "{" && right == "}" {
        return Gap::None;
    }
    Gap::Space
}

fn render(source: &str, tokens: &[Token]) -> Option<String> {
    let mut result = String::with_capacity(source.len());
    let mut previous = None;
    let mut depth = 0usize;
    // true 为表达式/类型 record，false 为语句或 JSX 表达式；只用于分号布局。
    let mut braces = Vec::new();
    let mut inline_semicolon = false;
    for token in tokens.iter().filter(|token| token.kind != Kind::Whitespace) {
        let value = text(source, token);
        if token.kind == Kind::BlockClose {
            depth = depth.saturating_sub(1);
        }
        let gap = previous.map_or(Gap::None, |previous| {
            gap(source, previous, token, inline_semicolon)
        });
        let extra = match gap {
            Gap::None => 0,
            Gap::Space => 1,
            Gap::Line => 1 + depth * 2,
        };
        if result
            .len()
            .saturating_add(extra)
            .saturating_add(value.len())
            > MAX_OUTPUT
        {
            return None;
        }
        match gap {
            Gap::None => {}
            Gap::Space => result.push(' '),
            Gap::Line => {
                if !result.ends_with('\n') {
                    result.push('\n');
                }
                result.push_str(&"  ".repeat(depth));
            }
        }
        result.push_str(value);
        if token.kind == Kind::BlockOpen {
            depth += 1;
        }
        match (token.kind, value) {
            (Kind::Code, "{") => braces.push(true),
            (Kind::BlockOpen | Kind::ChildOpen, _) => braces.push(false),
            (Kind::Code, "}") | (Kind::BlockClose | Kind::ChildClose, _) => {
                braces.pop();
            }
            _ => {}
        }
        inline_semicolon = braces.last() == Some(&true);
        previous = Some(token);
    }
    if !result.ends_with('\n') {
        if result.len() == MAX_OUTPUT {
            return None;
        }
        result.push('\n');
    }
    Some(result)
}

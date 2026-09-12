//! 保留 UIX 源码每个 UTF-8 字节的轻量无损具体语法流。

use crate::lang::compiler::source_graph::SourceId;

/// 区分格式化器需要理解的具体词法形状。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CstTokenKind {
    OpeningTag,
    ClosingTag,
    SelfClosingTag,
    Text,
    Trivia,
    LineComment,
    BlockComment,
}

/// 保存一个直接切片回原始源码的无损 token。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CstToken {
    pub kind: CstTokenKind,
    pub start: usize,
    pub end: usize,
    pub tag_name: Option<String>,
}

/// 保存来源身份、完整源码与覆盖全部字节的有序 token。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LosslessCst {
    source_id: SourceId,
    source: String,
    tokens: Vec<CstToken>,
}

impl LosslessCst {
    /// 为已通过 UIX parser 的源码建立无损具体语法流。
    pub(crate) fn from_source(source_id: SourceId, source: impl Into<String>) -> Self {
        let source = source.into();
        let tokens = tokenize(&source);
        Self {
            source_id,
            source,
            tokens,
        }
    }

    /// 返回与 SourceGraph 一致的来源身份。
    pub const fn source_id(&self) -> SourceId {
        self.source_id
    }

    /// 返回未经改写的完整源码。
    pub fn source(&self) -> &str {
        &self.source
    }

    /// 返回严格覆盖源码的有序 token。
    pub fn tokens(&self) -> &[CstToken] {
        &self.tokens
    }

    /// 借用一个 token 对应的原始文本。
    pub fn text(&self, token: &CstToken) -> &str {
        &self.source[token.start..token.end]
    }

    /// 按 token 顺序重建源码，用于协议与测试确认无损性。
    pub fn reconstruct(&self) -> String {
        self.tokens.iter().map(|token| self.text(token)).collect()
    }
}

fn tokenize(source: &str) -> Vec<CstToken> {
    let mut tokens = Vec::new();
    let mut offset = 0usize;
    while offset < source.len() {
        if source[offset..].starts_with("//") {
            let end = source[offset..]
                .find('\n')
                .map(|relative| offset + relative)
                .unwrap_or(source.len());
            push_token(&mut tokens, CstTokenKind::LineComment, offset, end, None);
            offset = end;
            continue;
        }
        if source[offset..].starts_with("/*") {
            let end = source[offset + 2..]
                .find("*/")
                .map(|relative| offset + 2 + relative + 2)
                .unwrap_or(source.len());
            push_token(&mut tokens, CstTokenKind::BlockComment, offset, end, None);
            offset = end;
            continue;
        }
        if source[offset..].starts_with('<') {
            let end = tag_end(source, offset).unwrap_or(source.len());
            let raw = &source[offset..end];
            let trimmed = raw.trim_end();
            let kind = if raw.starts_with("</") {
                CstTokenKind::ClosingTag
            } else if trimmed.ends_with("/>") {
                CstTokenKind::SelfClosingTag
            } else {
                CstTokenKind::OpeningTag
            };
            push_token(&mut tokens, kind, offset, end, tag_name(raw));
            offset = end;
            continue;
        }
        let character = source[offset..]
            .chars()
            .next()
            .expect("offset 必须位于源码字符边界");
        let whitespace = character.is_whitespace();
        let start = offset;
        offset += character.len_utf8();
        while offset < source.len() {
            if source[offset..].starts_with('<')
                || source[offset..].starts_with("//")
                || source[offset..].starts_with("/*")
            {
                break;
            }
            let next = source[offset..]
                .chars()
                .next()
                .expect("offset 必须位于源码字符边界");
            if next.is_whitespace() != whitespace {
                break;
            }
            offset += next.len_utf8();
        }
        push_token(
            &mut tokens,
            if whitespace {
                CstTokenKind::Trivia
            } else {
                CstTokenKind::Text
            },
            start,
            offset,
            None,
        );
    }
    tokens
}

fn tag_end(source: &str, start: usize) -> Option<usize> {
    let mut offset = start + 1;
    let mut quote = None;
    let mut escaped = false;
    let mut braces = 0usize;
    while offset < source.len() {
        let character = source[offset..].chars().next()?;
        offset += character.len_utf8();
        if escaped {
            escaped = false;
            continue;
        }
        if quote.is_some() && character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(expected) = quote {
            if character == expected {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
            continue;
        }
        match character {
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            '>' if braces == 0 => return Some(offset),
            _ => {}
        }
    }
    None
}

fn tag_name(raw: &str) -> Option<String> {
    let body = raw
        .strip_prefix("</")
        .or_else(|| raw.strip_prefix('<'))?
        .trim_start();
    let end = body
        .find(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .unwrap_or(body.len());
    (end > 0).then(|| body[..end].to_string())
}

fn push_token(
    tokens: &mut Vec<CstToken>,
    kind: CstTokenKind,
    start: usize,
    end: usize,
    tag_name: Option<String>,
) {
    if start < end {
        tokens.push(CstToken {
            kind,
            start,
            end,
            tag_name,
        });
    }
}

#[cfg(test)]
#[path = "../tests-src/lossless_cst_tests.rs"]
mod tests;


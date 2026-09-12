//! 由实际解析消费记录词法事实；不是供工具另行解释的第二个 lexer。
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Code,
    Literal,
    Text,
    Whitespace,
    LineComment,
    BlockComment,
    BlockOpen,
    BlockClose,
    TagStart,
    TagEnd,
    CloseTagStart,
    CloseTagEnd,
    SelfClose,
    ChildOpen,
    ChildClose,
    GenericOpen,
    GenericClose,
    AttributeEquals,
    Unary,
    ArrayOpen,
}
#[derive(Debug, Clone)]
pub(crate) struct Token {
    pub kind: Kind,
    pub span: Span,
}

impl Parser<'_> {
    pub(super) fn token(&mut self, start: usize, kind: Kind) -> Result<()> {
        if self
            .concrete
            .as_ref()
            .is_some_and(|tokens| tokens.len() >= 131_072)
        {
            return Err(self.error_at(
                start,
                "component-token-limit",
                "具体语法 token 超过 131072 项",
            ));
        }
        let span = self.span(start);
        if let Some(tokens) = &mut self.concrete {
            if span.end > span.start {
                tokens.push(Token { kind, span });
            }
        }
        Ok(())
    }
    pub(super) fn eat_as(&mut self, token: &str, kind: Kind) -> Result<bool> {
        if self.eat(token)? {
            if let Some(last) = self.concrete.as_mut().and_then(|tokens| tokens.last_mut()) {
                last.kind = kind;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub(super) fn expect_as(&mut self, token: &str, kind: Kind) -> Result<()> {
        if self.eat_as(token, kind)? {
            Ok(())
        } else {
            Err(self.error("component-token", format!("此处需要 {token}")))
        }
    }
}

pub(crate) fn parse_concrete(
    source: &str,
    name: String,
) -> std::result::Result<(ParsedSource, Vec<Token>), CompilerDiagnostic> {
    parse_with_tokens(source, name, true).map_err(|error| *error)
}

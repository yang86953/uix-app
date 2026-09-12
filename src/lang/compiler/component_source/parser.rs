use super::*;
use crate::lang::compiler::{DiagnosticPhase, source_graph::SourceId};

pub(super) mod concrete;
mod expressions;
mod limits;
mod recovery;
mod tokens;
pub(super) fn identifier_range(source: &str, offset: usize) -> Span {
    let offset = offset.min(source.len());
    let mut start = offset;
    while let Some(ch) = source[..start]
        .chars()
        .next_back()
        .filter(|ch| tokens::name_continue(*ch))
    {
        start -= ch.len_utf8();
    }
    if !source[start..]
        .chars()
        .next()
        .is_some_and(tokens::name_start)
    {
        return Span {
            start: offset,
            end: offset,
        };
    }
    let end = offset
        + source[offset..]
            .chars()
            .take_while(|ch| tokens::name_continue(*ch))
            .map(char::len_utf8)
            .sum::<usize>();
    Span { start, end }
}
use concrete::{Kind, Token};

// 完整诊断包含多个 String；递归解析的每个 ? 不应在成功栈帧中预留整份错误。
// 仅失败时分配一次，公开边界仍返回原来的 CompilerDiagnostic。
type Result<T> = std::result::Result<T, Box<CompilerDiagnostic>>;
const MAX_SOURCE: usize = 1_048_576;
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 16_384;
pub(super) const DECLARATION_WORDS: [&str; 5] =
    ["import", "export", "component", "function", "type"];

fn prefix_parser(source: &str) -> Parser<'_> {
    Parser {
        source,
        source_name: "<editor>",
        pos: 0,
        depth: 0,
        nodes: 0,
        concrete: None,
        recovery: None,
    }
}
/// 只供编辑器保持家族：显式旧 < / @ 前缀可切换，空白/未知片段不抹掉既有身份。
pub(crate) fn editor_source_hint(source: &str) -> Option<bool> {
    let mut end = source.len().min(MAX_SOURCE);
    while !source.is_char_boundary(end) {
        end -= 1;
    }
    let source = &source[..end];
    let mut parser = prefix_parser(source);
    let start = parser.start().ok()?;
    let rest = &source[start..];
    if rest.starts_with(['<', '@']) {
        return Some(false);
    }
    if recognizes_source(source) {
        return Some(true);
    }
    let word: String = rest
        .chars()
        .take_while(|ch| tokens::name_continue(*ch))
        .collect();
    if !word.is_empty()
        && rest[word.len()..].trim().is_empty()
        && DECLARATION_WORDS
            .iter()
            .any(|keyword| keyword.starts_with(&word))
    {
        return Some(true);
    }
    None
}
pub(super) fn declaration_prefix(source: &str, offset: usize) -> Option<Span> {
    if source.len() > MAX_SOURCE || offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let mut parser = prefix_parser(source);
    let start = parser.start().ok()?;
    let span = identifier_range(source, offset);
    if start == span.start
        && source[span.end..].trim().is_empty()
        && DECLARATION_WORDS
            .iter()
            .any(|word| word.starts_with(&source[span.start..offset]))
    {
        Some(span)
    } else {
        None
    }
}

/// 用同一解析器的注释和标识符边界识别组件家族；检查失败不回退旧语法。
pub fn recognizes_source(source: &str) -> bool {
    let mut parser = Parser {
        source,
        source_name: "<document>",
        pos: 0,
        depth: 0,
        nodes: 0,
        concrete: None,
        recovery: None,
    };
    DECLARATION_WORDS
        .into_iter()
        .any(|word| parser.at(word).unwrap_or(false))
}

pub(super) fn parse(
    source: &str,
    source_name: String,
) -> std::result::Result<ParsedSource, CompilerDiagnostic> {
    parse_with_tokens(source, source_name, false, false)
        .map(|(parsed, _, _)| parsed)
        .map_err(|error| *error)
}

pub(super) fn parse_editor(
    source: &str,
    name: String,
) -> std::result::Result<(ParsedSource, Vec<Token>, Vec<CompilerDiagnostic>), CompilerDiagnostic> {
    parse_with_tokens(source, name, true, true).map_err(|error| *error)
}

fn parse_with_tokens(
    source: &str,
    source_name: String,
    record: bool,
    recover: bool,
) -> Result<(ParsedSource, Vec<Token>, Vec<CompilerDiagnostic>)> {
    let mut parser = Parser {
        source,
        source_name: &source_name,
        pos: 0,
        depth: 0,
        nodes: 0,
        concrete: record.then(Vec::new),
        recovery: recover.then(Vec::new),
    };
    if source.len() > MAX_SOURCE {
        return Err(parser.error_at(0, "component-source-limit", "组件源码不能超过 1 MiB"));
    }
    let mut imports = Vec::new();
    let mut declarations = Vec::new();
    while !parser.eof()? {
        if parser.at("import")? {
            imports.push(parser.import()?);
        } else {
            declarations.push(parser.declaration()?);
        }
    }
    let tokens = parser.concrete.unwrap_or_default();
    let diagnostics = parser.recovery.unwrap_or_default();
    Ok((
        ParsedSource {
            source_graph: SourceGraph::inline(source_name, source),
            imports,
            declarations,
        },
        tokens,
        diagnostics,
    ))
}

struct Parser<'a> {
    source: &'a str,
    source_name: &'a str,
    pos: usize,
    depth: usize,
    nodes: usize,
    concrete: Option<Vec<Token>>,
    recovery: Option<Vec<CompilerDiagnostic>>,
}

impl Parser<'_> {
    fn nested<T>(&mut self, action: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        if self.depth >= MAX_DEPTH {
            return Err(self.error("component-depth-limit", "组件语法嵌套超过上限"));
        }
        self.depth += 1;
        let result = action(self);
        self.depth -= 1;
        result
    }

    fn node(&mut self) -> Result<()> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            Err(self.error("component-node-limit", "组件语法节点超过上限"))
        } else {
            Ok(())
        }
    }

    fn span(&self, start: usize) -> Span {
        Span {
            start,
            end: self.pos,
        }
    }

    fn error(&self, code: &'static str, message: impl Into<String>) -> Box<CompilerDiagnostic> {
        self.error_at(self.pos, code, message)
    }

    fn error_at(
        &self,
        start: usize,
        code: &'static str,
        message: impl Into<String>,
    ) -> Box<CompilerDiagnostic> {
        let prefix = &self.source[..start];
        Box::new(CompilerDiagnostic {
            code,
            phase: DiagnosticPhase::Syntax,
            source_id: SourceId::from_source_name(self.source_name),
            source_name: self.source_name.into(),
            start,
            end: start
                + self.source[start..]
                    .chars()
                    .next()
                    .map_or(0, char::len_utf8),
            line: prefix.bytes().filter(|byte| *byte == b'\n').count() + 1,
            column: prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1,
            message: message.into(),
            suggestion: "按类型化组件语法修改此处；此入口不接受旧 XML 声明或 Rust 表达式".into(),
        })
    }

    fn import(&mut self) -> Result<Import> {
        let start = self.start()?;
        self.expect("import")?;
        self.expect("{")?;
        let mut names = Vec::new();
        while !self.at_end("}")? {
            let imported = self.name()?;
            let local = if self.eat("as")? {
                self.name()?
            } else {
                imported.clone()
            };
            names.push(ImportName { imported, local });
            if !self.eat(",")? {
                break;
            }
        }
        self.expect("}")?;
        self.expect("from")?;
        let (source, source_span) = self.string()?;
        self.expect(";")?;
        Ok(Import {
            names,
            source,
            source_span,
            span: self.span(start),
        })
    }

    fn declaration(&mut self) -> Result<Declaration> {
        self.node()?;
        let start = self.start()?;
        let exported = self.eat("export")?;
        let (name, kind) = if self.eat("component")? {
            let name = self.name()?;
            let parameters = self.parameters(false)?;
            self.expect_as("{", Kind::BlockOpen)?;
            let mut body = Vec::new();
            while !self.at_end("}")? {
                let member_start = self.start()?;
                if self.eat("state")? {
                    let name = self.name()?;
                    self.expect(":")?;
                    let ty = self.ty()?;
                    self.expect("=")?;
                    let initial = self.expression()?;
                    self.expect(";")?;
                    body.push(ComponentMember::State {
                        name,
                        ty,
                        initial,
                        span: self.span(member_start),
                    });
                } else if self.eat("function")? {
                    let name = self.name()?;
                    let function = self.function()?;
                    body.push(ComponentMember::Function {
                        name,
                        function,
                        span: self.span(member_start),
                    });
                } else {
                    body.push(ComponentMember::Statement(self.statement()?));
                }
            }
            self.expect_as("}", Kind::BlockClose)?;
            (name, DeclarationKind::Component { parameters, body })
        } else if self.eat("function")? {
            let name = self.name()?;
            (name, DeclarationKind::Function(self.function()?))
        } else if self.eat("type")? {
            let name = self.name()?;
            self.expect("=")?;
            let ty = self.ty()?;
            self.expect(";")?;
            (name, DeclarationKind::Type(ty))
        } else {
            return Err(self.error(
                "component-declaration",
                "此处需要 component、function 或 type 声明",
            ));
        };
        Ok(Declaration {
            exported,
            name,
            kind,
            span: self.span(start),
        })
    }

    fn parameters(&mut self, inferred: bool) -> Result<Vec<Parameter>> {
        self.expect("(")?;
        let mut parameters = Vec::new();
        while !self.at_end(")")? {
            let start = self.start()?;
            let name = self.name()?;
            let ty = if self.eat(":")? {
                Some(self.ty()?)
            } else if inferred {
                None
            } else {
                return Err(
                    self.error("component-parameter-type", "公开组件和函数参数需要显式类型")
                );
            };
            let default = if self.eat("=")? {
                Some(self.expression()?)
            } else {
                None
            };
            parameters.push(Parameter {
                name,
                ty,
                default,
                span: self.span(start),
            });
            if !self.eat(",")? {
                break;
            }
        }
        self.expect(")")?;
        Ok(parameters)
    }

    fn function(&mut self) -> Result<Function> {
        let parameters = self.parameters(false)?;
        self.expect(":")?;
        let returns = self.ty()?;
        let body = self.block()?;
        Ok(Function {
            parameters,
            returns,
            body,
        })
    }

    fn ty(&mut self) -> Result<TypeNode> {
        self.nested(|parser| parser.type_inner())
    }

    fn type_inner(&mut self) -> Result<TypeNode> {
        self.node()?;
        if self.hole_boundary()? {
            return self.missing_type();
        }
        let start = self.start()?;
        let kind = if self.eat("{")? {
            let mut fields = Vec::new();
            while !self.at_end("}")? {
                let name = self.name()?;
                self.expect(":")?;
                fields.push((name, self.ty()?));
                if !self.eat(",")? && !self.eat(";")? {
                    break;
                }
            }
            self.expect("}")?;
            TypeKind::Record(fields)
        } else if self.eat("(")? {
            let mut parameters = Vec::new();
            while !self.at_end(")")? {
                let name = self.name()?;
                self.expect(":")?;
                parameters.push((name, self.ty()?));
                if !self.eat(",")? {
                    break;
                }
            }
            self.expect(")")?;
            self.expect("=>")?;
            TypeKind::Function {
                parameters,
                returns: Box::new(self.ty()?),
            }
        } else {
            let path = self.path()?;
            let mut arguments = Vec::new();
            if self.eat_as("<", Kind::GenericOpen)? {
                loop {
                    arguments.push(self.ty()?);
                    if !self.eat(",")? {
                        break;
                    }
                }
                self.expect_as(">", Kind::GenericClose)?;
            }
            TypeKind::Named { path, arguments }
        };
        let mut result = self.checked(TypeNode {
            kind,
            span: self.span(start),
        })?;
        let mut suffixes = 0;
        while self.eat("[")? {
            suffixes += 1;
            self.chain_limit(suffixes)?;
            self.expect("]")?;
            result = self.checked(TypeNode {
                kind: TypeKind::Array(Box::new(result)),
                span: self.span(start),
            })?;
        }
        Ok(result)
    }

    fn block(&mut self) -> Result<Block> {
        self.nested(|parser| {
            let start = parser.start()?;
            parser.expect_as("{", Kind::BlockOpen)?;
            let mut statements = Vec::new();
            while !parser.at_end("}")? {
                statements.push(parser.statement()?);
            }
            parser.expect_as("}", Kind::BlockClose)?;
            parser.checked(Block {
                statements,
                span: parser.span(start),
            })
        })
    }

    fn statement(&mut self) -> Result<Statement> {
        self.node()?;
        let start = self.start()?;
        let kind = if self.eat("let")? {
            let name = self.name()?;
            let ty = if self.eat(":")? {
                Some(self.ty()?)
            } else {
                None
            };
            self.expect("=")?;
            let value = self.expression()?;
            self.expect(";")?;
            StatementKind::Let { name, ty, value }
        } else if self.eat("return")? {
            let value = if self.at(";")? {
                None
            } else {
                Some(self.expression()?)
            };
            self.expect(";")?;
            StatementKind::Return(value)
        } else if self.eat("if")? {
            self.expect("(")?;
            let condition = self.expression()?;
            self.expect(")")?;
            let then_block = self.block()?;
            let else_block = if self.eat("else")? {
                Some(self.block()?)
            } else {
                None
            };
            StatementKind::If {
                condition,
                then_block,
                else_block,
            }
        } else {
            let target = self.expression()?;
            let kind = if self.eat("=")? {
                StatementKind::Assign {
                    target,
                    value: self.expression()?,
                }
            } else {
                StatementKind::Evaluate(target)
            };
            self.expect(";")?;
            kind
        };
        self.checked(Statement {
            kind,
            span: self.span(start),
        })
    }

    fn chain_limit(&self, length: usize) -> Result<()> {
        if self.depth + length > MAX_DEPTH {
            Err(self.error(
                "component-depth-limit",
                "表达式或类型链超过上限，请拆分为局部值",
            ))
        } else {
            Ok(())
        }
    }
}

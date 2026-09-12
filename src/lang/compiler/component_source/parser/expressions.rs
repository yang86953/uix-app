use super::*;

impl Parser<'_> {
    pub(super) fn expression(&mut self) -> Result<Expr> {
        self.precedence(0)
    }

    fn precedence(&mut self, minimum: u8) -> Result<Expr> {
        self.nested(|parser| parser.precedence_inner(minimum))
    }

    fn precedence_inner(&mut self, minimum: u8) -> Result<Expr> {
        self.node()?;
        let start = self.start()?;
        let mut left = self.prefix()?;
        let mut chain = 0;
        loop {
            // postfix 比一元与二元运算都紧；同一循环同时约束构造出的左深树。
            if self.at(".")? || self.at("[")? || self.at("(")? {
                let kind = if self.eat(".")? {
                    ExprKind::Member {
                        value: Box::new(left),
                        name: self.edit_name()?,
                    }
                } else if self.eat("[")? {
                    let index = self.expression()?;
                    self.expect("]")?;
                    ExprKind::Index {
                        value: Box::new(left),
                        index: Box::new(index),
                    }
                } else {
                    self.expect("(")?;
                    let mut arguments = Vec::new();
                    while !self.at_end(")")? {
                        arguments.push(self.expression()?);
                        if !self.eat(",")? {
                            break;
                        }
                    }
                    self.expect(")")?;
                    ExprKind::Call {
                        callee: Box::new(left),
                        arguments,
                    }
                };
                chain += 1;
                self.chain_limit(chain)?;
                self.node()?;
                left = self.checked(Expr {
                    kind,
                    span: self.span(start),
                })?;
                continue;
            }
            let mut operator = None;
            for (token, op, power) in [
                ("||", BinaryOp::Or, 1),
                ("&&", BinaryOp::And, 2),
                ("==", BinaryOp::Equal, 3),
                ("!=", BinaryOp::NotEqual, 3),
                ("<=", BinaryOp::LessEqual, 4),
                (">=", BinaryOp::GreaterEqual, 4),
                ("<", BinaryOp::Less, 4),
                (">", BinaryOp::Greater, 4),
                ("+", BinaryOp::Add, 5),
                ("-", BinaryOp::Subtract, 5),
                ("*", BinaryOp::Multiply, 6),
                ("/", BinaryOp::Divide, 6),
                ("%", BinaryOp::Remainder, 6),
            ] {
                if power >= minimum && self.at(token)? {
                    operator = Some((token, op, power));
                    break;
                }
            }
            if let Some((token, op, power)) = operator {
                chain += 1;
                self.chain_limit(chain)?;
                self.node()?;
                self.expect(token)?;
                let right = self.precedence(power + 1)?;
                left = self.checked(Expr {
                    kind: ExprKind::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    span: self.span(start),
                })?;
            } else {
                break;
            }
        }
        if minimum == 0 && self.eat("?")? {
            let yes = self.expression()?;
            self.expect(":")?;
            let no = self.expression()?;
            left = self.checked(Expr {
                kind: ExprKind::Conditional {
                    condition: Box::new(left),
                    yes: Box::new(yes),
                    no: Box::new(no),
                },
                span: self.span(start),
            })?;
        }
        Ok(left)
    }

    fn prefix(&mut self) -> Result<Expr> {
        let start = self.start()?;
        if self.hole_boundary()? {
            return self.missing_expression();
        }
        let kind = if self.eat_as("!", Kind::Unary)? {
            ExprKind::Unary {
                op: UnaryOp::Not,
                value: Box::new(self.precedence(7)?),
            }
        } else if self.eat_as("-", Kind::Unary)? {
            ExprKind::Unary {
                op: UnaryOp::Negate,
                value: Box::new(self.precedence(7)?),
            }
        } else if self.eat("true")? {
            ExprKind::Bool(true)
        } else if self.eat("false")? {
            ExprKind::Bool(false)
        } else if self.at("(")? {
            if self.lambda_ahead()? {
                let parameters = self.parameters(true)?;
                self.expect("=>")?;
                let body = if self.at("{")? {
                    LambdaBody::Block(self.block()?)
                } else {
                    LambdaBody::Expression(Box::new(self.expression()?))
                };
                ExprKind::Lambda { parameters, body }
            } else {
                self.expect("(")?;
                if self.eat(")")? {
                    ExprKind::Unit
                } else {
                    let mut value = self.expression()?;
                    self.expect(")")?;
                    value.span = self.span(start);
                    return Ok(value);
                }
            }
        } else if self.eat_as("[", Kind::ArrayOpen)? {
            let mut values = Vec::new();
            while !self.at_end("]")? {
                values.push(self.expression()?);
                if !self.eat(",")? {
                    break;
                }
            }
            self.expect("]")?;
            ExprKind::Array(values)
        } else if self.eat("{")? {
            let mut fields = Vec::new();
            while !self.at_end("}")? {
                let name = self.name()?;
                let value = if self.eat(":")? {
                    self.expression()?
                } else {
                    Expr {
                        kind: ExprKind::Name(name.clone()),
                        span: name.span,
                    }
                };
                fields.push((name, value));
                if !self.eat(",")? {
                    break;
                }
            }
            self.expect("}")?;
            ExprKind::Record(fields)
        } else if self.at("<")? {
            return self.element();
        } else if self.source[self.pos..].starts_with(['\'', '"']) {
            ExprKind::String(self.string()?.0)
        } else if self.source[self.pos..]
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_digit())
        {
            self.number()?
        } else {
            ExprKind::Name(self.name()?)
        };
        self.checked(Expr {
            kind,
            span: self.span(start),
        })
    }

    fn element(&mut self) -> Result<Expr> {
        self.nested(|parser| parser.element_inner())
    }

    fn element_inner(&mut self) -> Result<Expr> {
        self.node()?;
        let start = self.start()?;
        self.expect_as("<", Kind::TagStart)?;
        if self.eat_as(">", Kind::TagEnd)? {
            let children = self.view_children()?;
            self.expect_as("</", Kind::CloseTagStart)?;
            self.expect_as(">", Kind::CloseTagEnd)?;
            return self.checked(Expr {
                kind: ExprKind::Fragment(children),
                span: self.span(start),
            });
        }
        let name = self.edit_path()?;
        let mut end = name.last().unwrap().span.end;
        let mut attributes = Vec::new();
        while !self.at_end(">")? && !self.at("/>")? {
            let attribute_start = self.start()?;
            if attribute_start == end {
                return Err(self.error("component-attribute", "属性之间需要空白"));
            }
            let name = self.name()?;
            let value = if self.eat_as("=", Kind::AttributeEquals)? {
                if self.eat_as("{", Kind::ChildOpen)? {
                    let value = self.expression()?;
                    self.expect_as("}", Kind::ChildClose)?;
                    value
                } else if self.hole_boundary()? {
                    self.missing_expression()?
                } else {
                    let (value, span) = self.string()?;
                    Expr {
                        kind: ExprKind::String(value),
                        span,
                    }
                }
            } else {
                Expr {
                    kind: ExprKind::Bool(true),
                    span: name.span,
                }
            };
            // eat("=") 会跳过 trivia；真正属性终点不能吞掉下个属性之前的空白。
            end = if matches!(value.kind, ExprKind::Bool(true)) && value.span == name.span {
                name.span.end
            } else {
                self.pos
            };
            attributes.push(Attribute {
                name,
                value,
                span: Span {
                    start: attribute_start,
                    end,
                },
            });
        }
        let opening_span;
        let (children, closing_name) = if self.eat_as("/>", Kind::SelfClose)? {
            opening_span = self.span(start);
            (Vec::new(), None)
        } else {
            self.expect_as(">", Kind::TagEnd)?;
            opening_span = self.span(start);
            let children = self.view_children()?;
            if self.recover_closing_tag()? {
                return self.checked(Expr {
                    kind: ExprKind::Element(Element {
                        name,
                        opening_span,
                        closing_name: None,
                        attributes,
                        children,
                    }),
                    span: self.span(start),
                });
            }
            self.expect_as("</", Kind::CloseTagStart)?;
            let close_start = self.start()?;
            let close = self.edit_path()?;
            if !close.iter().any(|part| part.text.is_empty())
                && !name
                    .iter()
                    .map(|name| &name.text)
                    .eq(close.iter().map(|name| &name.text))
            {
                let error = self.error_at(
                    close_start,
                    "component-closing-tag",
                    "结束标签与开始标签不一致",
                );
                if self.recovery.is_some() && self.eof()? {
                    self.recover(error)?;
                } else {
                    return Err(error);
                }
            }
            self.expect_as(">", Kind::CloseTagEnd)?;
            (children, Some(close))
        };
        self.checked(Expr {
            kind: ExprKind::Element(Element {
                name,
                opening_span,
                closing_name,
                attributes,
                children,
            }),
            span: self.span(start),
        })
    }

    fn view_children(&mut self) -> Result<Vec<ViewChild>> {
        let mut children = Vec::new();
        loop {
            // 标签文本中的空白、//、引号不属于代码 trivia。
            let rest = &self.source[self.pos..];
            if rest.starts_with("</") {
                return Ok(children);
            }
            if rest.is_empty() {
                if self.recovery.is_some() {
                    return Ok(children);
                }
                return Err(self.error("component-closing-tag", "标签缺少结束标签"));
            }
            if rest.starts_with('<') {
                children.push(ViewChild::Expression(self.element()?));
            } else if rest.starts_with('{') {
                let start = self.pos;
                self.pos += 1;
                self.token(start, Kind::ChildOpen)?;
                if self.eat_as("}", Kind::ChildClose)? {
                    continue;
                } // JSX 空表达式或注释。
                let value = self.expression()?;
                self.expect_as("}", Kind::ChildClose)?;
                children.push(ViewChild::Expression(value));
            } else {
                self.node()?;
                let start = self.pos;
                self.pos += rest.find(['<', '{']).unwrap_or(rest.len());
                self.token(start, Kind::Text)?;
                children.push(ViewChild::Text {
                    value: self.source[start..self.pos].into(),
                    span: self.span(start),
                });
            }
        }
    }
}

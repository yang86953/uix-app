//! 应用表达式只消费共享 Expression AST，产生相同的类型与效果事实。

use super::check::Scope;
use super::*;

impl Scope<'_> {
    pub fn expr(
        &mut self,
        expression: &Expression,
        expected: Option<&Type>,
    ) -> Result<Expr, Diagnostic> {
        let span = expression.span;
        let (kind, ty) = match &expression.kind {
            ExpressionKind::Boolean(v) => (ExprKind::Literal(Value::Bool(*v)), Type::Bool),
            ExpressionKind::String(v) => {
                (ExprKind::Literal(Value::String(v.clone())), Type::String)
            }
            ExpressionKind::Number(v) => {
                if v.contains('.')
                    || v.contains('e')
                    || v.contains('E')
                    || expected == Some(&Type::Float)
                {
                    let v = v
                        .parse::<f64>()
                        .map_err(|_| fail(span, "Float 字面量无效"))?;
                    if !v.is_finite() {
                        return Err(fail(span, "Float 必须有限"));
                    }
                    (ExprKind::Literal(Value::Float(v)), Type::Float)
                } else {
                    let v = v
                        .parse::<i64>()
                        .map_err(|_| fail(span, "Int 字面量超出 i64 范围"))?;
                    (ExprKind::Literal(Value::Int(v)), Type::Int)
                }
            }
            ExpressionKind::Identifier(name) => {
                if name == "None" {
                    let Some(Type::Optional(ty)) = expected else {
                        return Err(fail(span, "None 需要 Option 类型上下文"));
                    };
                    (
                        ExprKind::Literal(Value::Optional(None)),
                        Type::Optional(ty.clone()),
                    )
                } else if let Some((slot, ty)) = self.locals.get(name) {
                    (ExprKind::Local(*slot), ty.clone())
                } else if let Some((slot, ty)) = self.states.get(name) {
                    if self.effect == Effect::Pure && !self.view {
                        return Err(fail(span, "纯函数不能读取模块状态"));
                    }
                    (ExprKind::State(*slot), ty.clone())
                } else {
                    return Err(fail(
                        span,
                        format!("未绑定名称 {name}；动态模块不捕获 Rust 作用域"),
                    ));
                }
            }
            ExpressionKind::Unary { operator, operand } => {
                let expr = self.expr(operand, expected)?;
                let negate = *operator == UnaryOperator::Negate;
                if if negate {
                    !matches!(expr.ty, Type::Int | Type::Float)
                } else {
                    expr.ty != Type::Bool
                } {
                    return Err(fail(span, "一元运算类型不符"));
                }
                let ty = expr.ty.clone();
                (
                    ExprKind::Unary {
                        negate,
                        value: Box::new(expr),
                    },
                    ty,
                )
            }
            ExpressionKind::Binary {
                operator,
                left,
                right,
            } => {
                let op = match operator {
                    BinaryOperator::Add => Binary::Add,
                    BinaryOperator::Subtract => Binary::Subtract,
                    BinaryOperator::Multiply => Binary::Multiply,
                    BinaryOperator::Divide => Binary::Divide,
                    BinaryOperator::Remainder => Binary::Remainder,
                    BinaryOperator::Equal => Binary::Equal,
                    BinaryOperator::NotEqual => Binary::NotEqual,
                    BinaryOperator::Less => Binary::Less,
                    BinaryOperator::LessEqual => Binary::LessEqual,
                    BinaryOperator::Greater => Binary::Greater,
                    BinaryOperator::GreaterEqual => Binary::GreaterEqual,
                    BinaryOperator::And => Binary::And,
                    BinaryOperator::Or => Binary::Or,
                };
                let left = self.expr(left, None)?;
                let right = self.expr(right, Some(&left.ty))?;
                use Binary::*;
                let ty = match op {
                    And | Or if left.ty == Type::Bool => Type::Bool,
                    Equal | NotEqual => Type::Bool,
                    Less | LessEqual | Greater | GreaterEqual
                        if matches!(left.ty, Type::Int | Type::Float | Type::String) =>
                    {
                        Type::Bool
                    }
                    Add if left.ty == Type::String => Type::String,
                    Add | Subtract | Multiply | Divide | Remainder
                        if matches!(left.ty, Type::Int | Type::Float) =>
                    {
                        left.ty.clone()
                    }
                    _ => return Err(fail(span, "二元运算不支持该类型")),
                };
                (
                    ExprKind::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    ty,
                )
            }
            ExpressionKind::Ternary {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition = self.expr(condition, Some(&Type::Bool))?;
                let yes = self.expr(then_branch, expected)?;
                let no = self.expr(else_branch, Some(&yes.ty))?;
                let ty = yes.ty.clone();
                (
                    ExprKind::Conditional {
                        condition: Box::new(condition),
                        yes: Box::new(yes),
                        no: Box::new(no),
                    },
                    ty,
                )
            }
            ExpressionKind::Array(values) => {
                let mut element_type = match expected {
                    Some(Type::Array(ty)) => Some(*ty.clone()),
                    _ => None,
                };
                let mut items = Vec::new();
                for value in values {
                    let value = self.expr(value, element_type.as_ref())?;
                    element_type = Some(value.ty.clone());
                    items.push(value);
                }
                let ty = element_type.ok_or_else(|| fail(span, "空数组需要 Array 类型上下文"))?;
                (ExprKind::Array(items), Type::Array(Box::new(ty)))
            }
            ExpressionKind::Object(fields) => {
                let expected_fields = match expected {
                    Some(Type::Record(fields)) => Some(fields),
                    _ => None,
                };
                let mut items = Vec::new();
                let mut types = BTreeMap::new();
                for field in fields {
                    let value = self.expr(
                        &field.value,
                        expected_fields.and_then(|f| f.get(&field.name)),
                    )?;
                    if types.insert(field.name.clone(), value.ty.clone()).is_some() {
                        return Err(fail(field.span, "重复记录字段"));
                    }
                    items.push((field.name.clone(), value));
                }
                (ExprKind::Record(items), Type::Record(types))
            }
            ExpressionKind::Member { object, member } => {
                let value = self.expr(object, None)?;
                let Type::Record(fields) = &value.ty else {
                    return Err(fail(span, "字段访问需要记录"));
                };
                let ty = fields
                    .get(member)
                    .cloned()
                    .ok_or_else(|| fail(span, format!("未知字段 {member}")))?;
                (
                    ExprKind::Member {
                        value: Box::new(value),
                        name: member.clone(),
                    },
                    ty,
                )
            }
            ExpressionKind::Index { object, index } => {
                let value = self.expr(object, None)?;
                let index = self.expr(index, Some(&Type::Int))?;
                let ty = match &value.ty {
                    Type::Array(ty) => *ty.clone(),
                    Type::String => Type::String,
                    Type::Bytes => Type::Int,
                    _ => return Err(fail(span, "下标只适用于数组、字节或字符串")),
                };
                (
                    ExprKind::Index {
                        value: Box::new(value),
                        index: Box::new(index),
                    },
                    ty,
                )
            }
            ExpressionKind::Call { callee, arguments } => {
                if arguments.iter().any(|a| a.name.is_some()) {
                    return Err(fail(span, "该位置不允许命名参数或状态修改"));
                }
                match &callee.kind {
                    ExpressionKind::Identifier(name) => {
                        if matches!(name.as_str(), "Some" | "None" | "Ok" | "Err") {
                            self.constructor(name, arguments, expected, span)?
                        } else {
                            let signature = self
                                .signatures
                                .get(name)
                                .ok_or_else(|| fail(span, format!("未登记函数或宿主端口 {name}")))?
                                .clone();
                            if signature.asynchronous {
                                return Err(fail(
                                    span,
                                    "异步入口需要 AsyncCommand 的 await 或宿主后台调用",
                                ));
                            }
                            if signature.effect > self.effect {
                                return Err(fail(
                                    span,
                                    format!(
                                        "{:?} 求值不能调用 {:?} {name}",
                                        self.effect, signature.effect
                                    ),
                                ));
                            }
                            if signature.parameters.len() != arguments.len() {
                                return Err(fail(span, "调用参数数量不符"));
                            }
                            let args = arguments
                                .iter()
                                .zip(&signature.parameters)
                                .map(|(a, (_, t))| self.expr(&a.value, Some(t)))
                                .collect::<Result<Vec<_>, _>>()?;
                            (
                                ExprKind::Call {
                                    name: name.clone(),
                                    arguments: args,
                                },
                                signature.returns,
                            )
                        }
                    }
                    ExpressionKind::Member { object, member } => {
                        if let Some(name) =
                            callable_name(callee).filter(|name| self.signatures.contains_key(name))
                        {
                            let call = Expression {
                                kind: ExpressionKind::Call {
                                    callee: Box::new(Expression {
                                        kind: ExpressionKind::Identifier(name),
                                        span: callee.span,
                                    }),
                                    arguments: arguments.clone(),
                                },
                                span,
                            };
                            return self.expr(&call, expected);
                        }
                        self.method(object, member, arguments, span)?
                    }
                    _ => return Err(fail(span, "动态调用目标必须是显式名称")),
                }
            }
            ExpressionKind::Closure { .. } | ExpressionKind::LoweredAction(_) => {
                return Err(fail(span, "不允许逃逸闭包或 Rust 绑定"));
            }
        };
        if expected.is_some_and(|t| *t != ty) {
            return Err(fail(
                span,
                format!("类型不符：需要 {expected:?}，实际 {ty:?}"),
            ));
        }
        self.type_nodes += type_cost(&ty, span)?;
        if self.type_nodes > 65_536 {
            return Err(fail(span, "函数表达式类型累计超过 65536 个节点"));
        }
        Ok(Expr {
            kind,
            ty,
            location: location(self.source, span),
        })
    }

    fn constructor(
        &mut self,
        name: &str,
        args: &[CallArgument],
        expected: Option<&Type>,
        span: SourceSpan,
    ) -> Result<(ExprKind, Type), Diagnostic> {
        let (ty, input) = match (name, expected) {
            ("None", Some(Type::Optional(t))) if args.is_empty() => {
                return Ok((
                    ExprKind::Literal(Value::Optional(None)),
                    Type::Optional(t.clone()),
                ));
            }
            ("Some", Some(Type::Optional(t))) => (expected.cloned(), Some(t.as_ref())),
            ("Ok", Some(Type::Result(t, _))) | ("Err", Some(Type::Result(_, t))) => {
                (expected.cloned(), Some(t.as_ref()))
            }
            ("Some", None) => (None, None),
            _ => return Err(fail(span, "结果或可选值构造需要完整类型上下文")),
        };
        if args.len() != 1 {
            return Err(fail(span, "构造需要一个参数"));
        }
        let value = self.expr(&args[0].value, input)?;
        let ty = ty.unwrap_or_else(|| Type::Optional(Box::new(value.ty.clone())));
        Ok((
            ExprKind::Call {
                name: name.to_string(),
                arguments: vec![value],
            },
            ty,
        ))
    }

    fn method(
        &mut self,
        object: &Expression,
        name: &str,
        arguments: &[CallArgument],
        span: SourceSpan,
    ) -> Result<(ExprKind, Type), Diagnostic> {
        let value = self.expr(object, None)?;
        if matches!(name, "map" | "filter") {
            let Type::Array(element) = &value.ty else {
                return Err(fail(span, "map/filter 需要数组"));
            };
            let Some(CallArgument {
                value:
                    Expression {
                        kind: ExpressionKind::Closure { parameter, body },
                        ..
                    },
                ..
            }) = arguments.first()
            else {
                return Err(fail(span, "map/filter 需要单参数纯闭包"));
            };
            if arguments.len() != 1 {
                return Err(fail(span, "map/filter 只接收一个闭包"));
            }
            let before = self.locals.clone();
            let slot = self.bind(parameter, *element.clone(), span)?;
            let old_effect = self.effect;
            let old_view = self.view;
            self.effect = Effect::Pure;
            self.view = false;
            let body = self.expr(
                body,
                if name == "filter" {
                    Some(&Type::Bool)
                } else {
                    None
                },
            )?;
            self.effect = old_effect;
            self.view = old_view;
            self.locals = before;
            let ty = if name == "filter" {
                value.ty.clone()
            } else {
                Type::Array(Box::new(body.ty.clone()))
            };
            return Ok((
                ExprKind::Map {
                    value: Box::new(value),
                    slot,
                    body: Box::new(body),
                    filter: name == "filter",
                },
                ty,
            ));
        }
        let (inputs, result) = match (&value.ty, name) {
            (Type::String | Type::Array(_) | Type::Bytes, "len") => (vec![], Type::Int),
            (Type::String, "trim" | "toUpperCase" | "toLowerCase") => (vec![], Type::String),
            (Type::String, "chars" | "words" | "lines") => {
                (vec![], Type::Array(Box::new(Type::String)))
            }
            (Type::String, "split") => (vec![Type::String], Type::Array(Box::new(Type::String))),
            (Type::String, "replace") => (vec![Type::String, Type::String], Type::String),
            (Type::String, "contains") => (vec![Type::String], Type::Bool),
            (Type::Array(t), "contains") => (vec![*t.clone()], Type::Bool),
            (Type::Array(t), "push") => (vec![*t.clone()], value.ty.clone()),
            (Type::Array(t), "get") => (vec![Type::Int], Type::Optional(t.clone())),
            (Type::Array(t), "join") if **t == Type::String => (vec![Type::String], Type::String),
            (Type::Optional(_), "isSome") | (Type::Result(_, _), "isOk") => (vec![], Type::Bool),
            (Type::Optional(t) | Type::Result(t, _), "unwrapOr") => (vec![*t.clone()], *t.clone()),
            (Type::String | Type::Int | Type::Float | Type::Bool, "toString") => {
                (vec![], Type::String)
            }
            _ => return Err(fail(span, format!("类型 {:?} 不支持方法 {name}", value.ty))),
        };
        if inputs.len() != arguments.len() {
            return Err(fail(span, "方法参数数量不符"));
        }
        let args = arguments
            .iter()
            .zip(&inputs)
            .map(|(a, t)| self.expr(&a.value, Some(t)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok((
            ExprKind::Method {
                value: Box::new(value),
                name: name.to_string(),
                arguments: args,
            },
            result,
        ))
    }
}

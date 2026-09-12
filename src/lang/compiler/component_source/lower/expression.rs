use super::*;
use rt::ir::{Attribute as A, Expr as E, ExprKind as K};
impl Lower<'_> {
    pub(super) fn expr(&self, source: SourceId, expression: &Expr) -> Result<E> {
        let id = node(source, expression.span);
        let fact = self
            .checked
            .expressions()
            .get(&id)
            .ok_or_else(|| self.error(id, "表达式缺少共同语义事实"))?;
        if let Some(value) = &fact.constant {
            return Ok(E {
                kind: K::Constant(value.clone()),
                location: self.location(id),
            });
        }
        let boxed = |expr: &Expr| self.expr(source, expr).map(Box::new);
        let values = |values: &[Expr]| {
            values
                .iter()
                .map(|expr| self.expr(source, expr))
                .collect::<Result<Vec<_>>>()
        };
        let kind = match &expression.kind {
            ExprKind::Unit => K::Constant(runtime::Value::Unit),
            ExprKind::Bool(value) => K::Constant(runtime::Value::Bool(*value)),
            ExprKind::Integer(value) => K::Constant(runtime::Value::Int(
                i64::try_from(*value).map_err(|_| self.error(id, "整数缺少标准化语义值"))?,
            )),
            ExprKind::Float(value) => K::Constant(runtime::Value::Float(*value)),
            ExprKind::String(value) => K::Constant(runtime::Value::String(value.clone())),
            ExprKind::Name(_) | ExprKind::Lambda { .. } => match &fact.resolution {
                Some(ResolvedName::Binding(id)) => K::Get(self.bindings[id]),
                Some(ResolvedName::Function(id)) => K::Closure(self.functions[id]),
                Some(ResolvedName::NativeFunction { package, name }) => {
                    K::NativeFunction(rt::ExportKey::new(package, name))
                }
                _ => return Err(self.error(id, "表达式缺少可执行名称身份")),
            },
            ExprKind::Unary { op, value } => K::Unary {
                negate: *op == UnaryOp::Negate,
                value: boxed(value)?,
            },
            ExprKind::Binary { op, left, right } => K::Binary {
                op: binary(*op),
                left: boxed(left)?,
                right: boxed(right)?,
            },
            ExprKind::Conditional { condition, yes, no } => K::Conditional {
                condition: boxed(condition)?,
                yes: boxed(yes)?,
                no: boxed(no)?,
            },
            ExprKind::Array(items) => K::Array {
                values: values(items)?,
                rich: !matches!(fact.ty, Type::Data(_)),
            },
            ExprKind::Record(fields) => K::Record(
                fields
                    .iter()
                    .map(|(name, expr)| {
                        self.expr(source, expr)
                            .map(|expr| (name.text.clone(), expr))
                    })
                    .collect::<Result<_>>()?,
            ),
            ExprKind::Member { value, name } => K::Member {
                value: boxed(value)?,
                name: name.text.clone(),
            },
            ExprKind::Index { value, index } => K::Index {
                value: boxed(value)?,
                index: boxed(index)?,
            },
            ExprKind::Call { callee, arguments } => {
                let callee_fact = &self.checked.expressions()[&node(source, callee.span)];
                if let Some(ResolvedName::Method(name)) = &callee_fact.resolution {
                    let ExprKind::Member { value, .. } = &callee.kind else {
                        return Err(self.error(id, "方法缺少接收者"));
                    };
                    if matches!(name.as_str(), "map" | "filter") {
                        K::Map {
                            value: boxed(value)?,
                            callback: boxed(&arguments[0])?,
                            filter: name == "filter",
                        }
                    } else {
                        K::Method {
                            value: boxed(value)?,
                            name: name.clone(),
                            arguments: values(arguments)?,
                        }
                    }
                } else {
                    K::Call {
                        callee: boxed(callee)?,
                        arguments: values(arguments)?,
                    }
                }
            }
            ExprKind::Element(element) => {
                let (target, signature) = match &fact.resolution {
                    Some(ResolvedName::Component(id)) => (
                        rt::Target::Component(self.components[id]),
                        &self.checked.components()[id],
                    ),
                    Some(ResolvedName::NativeComponent { package, name }) => {
                        let NativeExport::Component(signature) =
                            &self.checked.native_imports()[package][name]
                        else {
                            unreachable!()
                        };
                        (
                            rt::Target::Native(rt::ExportKey::new(package, name)),
                            signature,
                        )
                    }
                    _ => return Err(self.error(id, "组件缺少目标身份")),
                };
                let mut attributes = element
                    .attributes
                    .iter()
                    .map(|attribute| {
                        self.expr(source, &attribute.value).map(|expr| {
                            if attribute.name.text == "key" {
                                A::Key(expr)
                            } else {
                                A::Property(attribute.name.text.clone(), expr)
                            }
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                let string = signature.parameters.iter().any(|(name, ty)| {
                    name == "children" && *ty == Type::Data(runtime::Type::String)
                });
                if (!element.children.is_empty() && string) || element.children.iter().any(|child|!matches!(child,ViewChild::Text{value,..} if value.trim().is_empty())) {
                    let children=self.children(source,&element.children,string)?;attributes.push(A::Property("children".into(),E{kind:if string{K::Concat(children)}else{K::Fragment(children)},location:self.location(id)}));
                }
                K::Element {
                    site: self.sites[&id],
                    target,
                    attributes,
                }
            }
            ExprKind::Fragment(children) => K::Fragment(self.children(source, children, false)?),
        };
        Ok(E {
            kind,
            location: self.location(id),
        })
    }
    fn children(&self, source: SourceId, children: &[ViewChild], string: bool) -> Result<Vec<E>> {
        children
            .iter()
            .filter_map(|child| match child {
                ViewChild::Expression(expr) => Some(self.expr(source, expr)),
                ViewChild::Text { value, span } if string => Some(Ok(E {
                    kind: K::Constant(runtime::Value::String(value.clone())),
                    location: self.location(node(source, *span)),
                })),
                _ => None,
            })
            .collect()
    }
}
fn binary(op: BinaryOp) -> runtime::Binary {
    use runtime::Binary as B;
    match op {
        BinaryOp::Or => B::Or,
        BinaryOp::And => B::And,
        BinaryOp::Equal => B::Equal,
        BinaryOp::NotEqual => B::NotEqual,
        BinaryOp::Less => B::Less,
        BinaryOp::LessEqual => B::LessEqual,
        BinaryOp::Greater => B::Greater,
        BinaryOp::GreaterEqual => B::GreaterEqual,
        BinaryOp::Add => B::Add,
        BinaryOp::Subtract => B::Subtract,
        BinaryOp::Multiply => B::Multiply,
        BinaryOp::Divide => B::Divide,
        BinaryOp::Remainder => B::Remainder,
    }
}

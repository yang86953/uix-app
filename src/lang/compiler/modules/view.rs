//! 界面沿用共享 AST；编译时绑定词法捕获，提交时展开结构。
use super::check::Scope;
use super::*;
use std::collections::BTreeSet;

pub(super) fn check(
    element: &Element,
    source: &str,
    signatures: &BTreeMap<String, Signature>,
    states: &BTreeMap<String, (usize, Type)>,
    functions: &mut Vec<Function>,
    tasks: &mut Vec<Task>,
) -> Result<ViewTemplate, Diagnostic> {
    attributes(element, &[])?;
    let roots = children(element)?;
    if roots.len() != 1 || matches!(roots[0].name.as_str(), "If" | "ElseIf" | "Else" | "For") {
        return Err(fail(
            element.span,
            "View 需要一个普通根控件，条件和循环放在其内部",
        ));
    }
    Checker {
        source,
        signatures,
        states,
        functions,
        tasks,
        keys: BTreeSet::new(),
    }
    .node(roots[0], &[])
}
fn children(element: &Element) -> Result<Vec<&Element>, Diagnostic> {
    element
        .children
        .iter()
        .filter(|n| !matches!(n, Node::Text(t) if t.value.trim().is_empty()))
        .map(|n| match n {
            Node::Element(e) => Ok(e),
            _ => Err(fail(element.span, "模块 View 使用控件标签和属性表达式")),
        })
        .collect()
}
struct Checker<'a> {
    source: &'a str,
    signatures: &'a BTreeMap<String, Signature>,
    states: &'a BTreeMap<String, (usize, Type)>,
    functions: &'a mut Vec<Function>,
    tasks: &'a mut Vec<Task>,
    keys: BTreeSet<String>,
}
impl Checker<'_> {
    fn scope(
        &self,
        captures: &[(String, Type)],
        effect: Effect,
        view: bool,
    ) -> Result<Scope<'_>, Diagnostic> {
        let mut scope = Scope::new(
            self.source,
            self.signatures,
            self.states,
            effect,
            Type::Unit,
        );
        scope.view = view;
        for (name, ty) in captures {
            scope.bind(
                name,
                ty.clone(),
                SourceSpan {
                    start: 0,
                    end: 0,
                    line: 1,
                    column: 1,
                },
            )?;
        }
        Ok(scope)
    }
    fn function(
        &mut self,
        expr: Expr,
        parameters: Vec<(String, Type)>,
        local_count: usize,
        effect: Effect,
        span: SourceSpan,
    ) -> String {
        let name = format!("__uix_view_{}", self.functions.len());
        self.functions.push(Function {
            signature: Signature {
                name: name.clone(),
                parameters,
                returns: expr.ty.clone(),
                effect,
                asynchronous: false,
            },
            exported: false,
            local_count,
            body: Body::Dynamic(vec![Statement::Return(Some(expr))]),
            location: location(self.source, span),
        });
        name
    }
    fn binding(
        &mut self,
        ast: &Expression,
        captures: &[(String, Type)],
        expected: Option<&Type>,
    ) -> Result<(String, Type), Diagnostic> {
        let mut scope = self.scope(captures, Effect::Pure, true)?;
        let expr = scope.expr(ast, expected)?;
        let slots = scope.next_slot;
        let ty = expr.ty.clone();
        let name = self.function(expr, captures.to_vec(), slots, Effect::Query, ast.span);
        Ok((name, ty))
    }
    fn sequence(
        &mut self,
        nodes: &[&Element],
        captures: &[(String, Type)],
    ) -> Result<Vec<ViewTemplate>, Diagnostic> {
        let mut result = Vec::new();
        let mut cursor = 0;
        while cursor < nodes.len() {
            let element = nodes[cursor];
            if element.name == "If" {
                result.push(self.conditional(nodes, &mut cursor, captures)?);
            } else {
                if matches!(element.name.as_str(), "Else" | "ElseIf") {
                    return Err(fail(element.span, "Else/ElseIf 必须紧随 If 分支"));
                }
                result.push(self.node(element, captures)?);
                cursor += 1;
            }
        }
        Ok(result)
    }
    fn conditional(
        &mut self,
        nodes: &[&Element],
        cursor: &mut usize,
        captures: &[(String, Type)],
    ) -> Result<ViewTemplate, Diagnostic> {
        let element = nodes[*cursor];
        attributes(element, &[])?;
        let Some(ControlBinding::If(condition)) = &element.control else {
            return Err(fail(element.span, "条件分支缺少 Bool 表达式"));
        };
        let (condition, _) = self.binding(&condition.expression, captures, Some(&Type::Bool))?;
        let branch = self.sequence(&children(element)?, captures)?;
        *cursor += 1;
        let otherwise = match nodes.get(*cursor) {
            Some(next) if next.name == "ElseIf" => vec![self.conditional(nodes, cursor, captures)?],
            Some(next) if next.name == "Else" => {
                attributes(next, &[])?;
                *cursor += 1;
                self.sequence(&children(next)?, captures)?
            }
            _ => vec![],
        };
        Ok(control(
            ViewControl::If {
                condition,
                otherwise,
            },
            branch,
        ))
    }
    fn node(
        &mut self,
        element: &Element,
        captures: &[(String, Type)],
    ) -> Result<ViewTemplate, Diagnostic> {
        if element.name == "For" {
            attributes(element, &[])?;
            let Some(ControlBinding::For {
                binding,
                binding_span,
                index_binding,
                index_span,
                iterable,
                key,
            }) = &element.control
            else {
                return Err(fail(element.span, "For 缺少绑定"));
            };
            let (items, ty) = self.binding(&iterable.expression, captures, None)?;
            let Type::Array(item_type) = ty else {
                return Err(fail(iterable.expression.span, "For 数据源必须是 Array"));
            };
            let key = key
                .as_ref()
                .ok_or_else(|| fail(element.span, "动态 For 必须声明稳定 key 表达式"))?;
            let mut scope = self.scope(captures, Effect::Pure, true)?;
            scope.bind(binding, *item_type.clone(), *binding_span)?;
            if let Some(index) = index_binding {
                scope.bind(index, Type::Int, index_span.unwrap_or(*binding_span))?;
            }
            let mut captures = captures.to_vec();
            captures.push((binding.clone(), *item_type));
            if let Some(index) = index_binding {
                captures.push((index.clone(), Type::Int));
            }
            let (key, ty) = self.binding(&key.expression, &captures, None)?;
            if !matches!(ty, Type::String | Type::Int) {
                return Err(fail(element.span, "For key 必须是 String 或 Int"));
            }
            let body = self.sequence(&children(element)?, &captures)?;
            return Ok(control(
                ViewControl::For {
                    items,
                    key,
                    indexed: index_binding.is_some(),
                },
                body,
            ));
        }
        let declaration = crate::lang::compiler::components::declaration(&element.name)
            .ok_or_else(|| fail(element.span, format!("未声明模块控件 {}", element.name)))?;
        let contract = declaration.module_view.as_ref()
            .ok_or_else(|| fail(element.span, format!("控件 {} 未声明模块投影合同", element.name)))?;
        crate::lang::compiler::components::use_unit(&declaration.unit);
        let kind = element.name.clone();
        let mut allowed = vec!["key"];
        allowed.extend(contract.properties.keys().map(String::as_str));
        allowed.extend(contract.event.as_deref());
        attributes(element, &allowed)?;
        let defaults = contract.properties.iter().map(|(name, value)| {
            let value = match value {
                serde_json::Value::String(v) => Value::String(v.clone()),
                serde_json::Value::Bool(v) => Value::Bool(*v),
                serde_json::Value::Number(v) => Value::Float(v.as_f64().ok_or_else(|| fail(element.span, "模块数值默认值超出范围"))?),
                _ => return Err(fail(element.span, "模块属性默认值必须是字符串、布尔或数值")),
            };
            Ok((name, value))
        }).collect::<Result<Vec<_>, Diagnostic>>()?;
        let key = required(element, "key")?.to_string();
        identifier(&key, element.span)?;
        if !self.keys.insert(key.clone()) {
            return Err(fail(element.span, "View 静态 key 必须在模块内唯一"));
        }
        let mut properties = BTreeMap::new();
        for (name, default) in defaults {
            let ty = match &default {
                Value::String(_) => Type::String,
                Value::Bool(_) => Type::Bool,
                _ => Type::Float,
            };
            let function = match element.attributes.iter().find(|a| a.name == *name) {
                Some(attribute) => {
                    let ast = match &attribute.value {
                        AttributeValue::Expression(value) => value.expression.clone(),
                        AttributeValue::Literal(value) if ty == Type::String => Expression {
                            kind: ExpressionKind::String(value.clone()),
                            span: attribute.span,
                        },
                        _ => return Err(fail(attribute.span, "非字符串界面属性需要 {表达式}")),
                    };
                    self.binding(&ast, captures, Some(&ty))?.0
                }
                None => self.function(
                    Expr {
                        kind: ExprKind::Literal(default.clone()),
                        ty,
                        location: location(self.source, element.span),
                    },
                    captures.to_vec(),
                    captures.len(),
                    Effect::Query,
                    element.span,
                ),
            };
            properties.insert(name.to_string(), function);
        }
        let handler = element
            .attributes
            .iter()
            .find(|a| contract.event.as_deref() == Some(a.name.as_str()))
            .map(|attribute| {
                let AttributeValue::Expression(value) = &attribute.value else {
                    return Err(fail(attribute.span, "事件需要 {命令调用} 表达式"));
                };
                let mut scope = self.scope(captures, Effect::Command, false)?;
                let mut parameters = captures.to_vec();
                if !contract.event_fields.is_empty() {
                    let fields = contract.event_fields.iter().map(|(name, ty)| {
                        let ty = match ty.as_str() {
                            "String" => Type::String, "Bool" => Type::Bool,
                            "Int" => Type::Int, "Float" => Type::Float,
                            _ => return Err(fail(attribute.span, format!("未知事件字段类型 {ty}"))),
                        };
                        Ok((name.clone(), ty))
                    }).collect::<Result<BTreeMap<_, _>, Diagnostic>>()?;
                    let event = Type::Record(fields);
                    scope
                        .locals
                        .insert("$event".into(), (scope.next_slot, event.clone()));
                    scope.next_slot += 1;
                    parameters.push(("$event".into(), event));
                }
                if let ExpressionKind::Call { callee, arguments } = &value.expression.kind {
                    if let Some(target) = callable_name(callee) {
                        if let Some(signature) = self
                            .signatures
                            .get(&target)
                            .filter(|signature| signature.asynchronous)
                        {
                            if !self.tasks.iter().any(|task| task.signature.name == target) {
                                return Err(fail(
                                    attribute.span,
                                    "界面异步事件必须调用 AsyncCommand，而不是直接调用端口",
                                ));
                            }
                            let arguments = super::tasks::arguments_for(
                                &mut scope,
                                arguments,
                                signature,
                                attribute.span,
                            )?;
                            let slots = scope.next_slot;
                            let name = format!("__uix_event_task_{}", self.tasks.len());
                            let task = Task {
                                signature: Signature {
                                    name: name.clone(),
                                    parameters,
                                    returns: signature.returns.clone(),
                                    effect: Effect::Command,
                                    asynchronous: true,
                                },
                                exported: false,
                                local_count: slots,
                                stages: vec![TaskStage {
                                    body: TaskBody::Dynamic {
                                        statements: vec![],
                                        exit: TaskExit::TailCall {
                                            name: target.clone(),
                                            arguments,
                                        },
                                    },
                                    location: location(self.source, attribute.span),
                                }],
                            };
                            self.tasks.push(task);
                            return Ok(name);
                        }
                    }
                }
                let expr = scope.expr(&value.expression, None)?;
                let slots = scope.next_slot;
                Ok(self.function(expr, parameters, slots, Effect::Command, attribute.span))
            })
            .transpose()?;
        let children = children(element)?;
        if !contract.children && !children.is_empty() {
            return Err(fail(element.span, "叶控件不能有子节点"));
        }
        let children = self.sequence(&children, captures)?;
        Ok(ViewTemplate {
            kind,
            key,
            properties,
            handler,
            children,
            control: None,
        })
    }
}
fn control(control: ViewControl, children: Vec<ViewTemplate>) -> ViewTemplate {
    ViewTemplate {
        kind: String::new(),
        key: String::new(),
        properties: BTreeMap::new(),
        handler: None,
        children,
        control: Some(control),
    }
}

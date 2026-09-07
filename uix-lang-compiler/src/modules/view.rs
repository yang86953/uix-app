//! 界面沿用共享节点与表达式 AST；只在模块前端限定可移植控件合同。

use super::*;
use super::check::Scope;
use std::collections::BTreeSet;

pub(super) fn check(element: &Element, source: &str,
    signatures: &BTreeMap<String, Signature>, states: &BTreeMap<String, (usize, Type)>,
    functions: &mut Vec<Function>) -> Result<ViewTemplate, Diagnostic> {
    attributes(element, &[])?;
    let roots = children(element)?;
    if roots.len() != 1 { return Err(fail(element.span, "View 需要一个根控件")); }
    node(roots[0], source, signatures, states, functions, &mut BTreeSet::new())
}

fn children(element: &Element) -> Result<Vec<&Element>, Diagnostic> {
    element.children.iter().filter(|n| !matches!(n, Node::Text(t) if t.value.trim().is_empty()))
        .map(|n| match n { Node::Element(e) => Ok(e), _ => Err(fail(element.span, "模块 View 使用控件标签和属性表达式")) }).collect()
}

fn node(element: &Element, source: &str, signatures: &BTreeMap<String, Signature>,
    states: &BTreeMap<String, (usize, Type)>, functions: &mut Vec<Function>, keys: &mut BTreeSet<String>) -> Result<ViewTemplate, Diagnostic> {
    let (kind, allowed, defaults): (ViewKind, &[&str], &[(&str, Value)]) = match element.name.as_str() {
        "Column" => (ViewKind::Column, &["key", "gap", "padding"], &[("gap", Value::Float(8.0)), ("padding", Value::Float(12.0))]),
        "Row" => (ViewKind::Row, &["key", "gap", "padding"], &[("gap", Value::Float(8.0)), ("padding", Value::Float(0.0))]),
        "Text" => (ViewKind::Text, &["key", "text"], &[("text", Value::String(String::new()))]),
        "Input" => (ViewKind::Input, &["key", "value", "placeholder", "onChange"], &[("value", Value::String(String::new())), ("placeholder", Value::String(String::new()))]),
        "Button" => (ViewKind::Button, &["key", "text", "disabled", "onClick"], &[("text", Value::String(String::new())), ("disabled", Value::Bool(false))]),
        _ => return Err(fail(element.span, format!("模块 View 暂不支持控件 {}", element.name))),
    };
    attributes(element, allowed)?;
    let key = required(element, "key")?.to_string();
    identifier(&key, element.span)?;
    if !keys.insert(key.clone()) { return Err(fail(element.span, "View key 必须在模块内唯一")); }
    let mut properties = BTreeMap::new();
    for (name, default) in defaults {
        let ty = match default { Value::String(_) => Type::String, Value::Bool(_) => Type::Bool, _ => Type::Float };
        let mut scope = Scope::new(source, signatures, states, Effect::Pure, ty.clone());
        scope.view = true;
        let expr = match element.attributes.iter().find(|a| a.name == *name) {
            Some(a) => {
                let ast = match &a.value {
                    AttributeValue::Expression(v) => v.expression.clone(),
                    AttributeValue::Literal(v) if ty == Type::String => Expression { kind: ExpressionKind::String(v.clone()), span: a.span },
                    _ => return Err(fail(a.span, "非字符串界面属性需要 {表达式}")),
                };
                scope.expr(&ast, Some(&ty))?
            }
            None => Expr { kind: ExprKind::Literal(default.clone()), ty: ty.clone(), location: location(source, element.span) },
        };
        let generated = format!("__uix_view_{}", functions.len());
        functions.push(Function { signature: Signature { name: generated.clone(), parameters: vec![], returns: ty,
            effect: Effect::Query }, exported: false, local_count: scope.next_slot,
            body: Body::Dynamic(vec![Statement::Return(Some(expr))]), location: location(source, element.span) });
        properties.insert(name.to_string(), generated);
    }
    let event = if kind == ViewKind::Input { "onChange" } else { "onClick" };
    let handler = element.attributes.iter().find(|a| a.name == event).map(|a| {
        let AttributeValue::Expression(value) = &a.value else { return Err(fail(a.span, "事件需要 {命令调用} 表达式")); };
        let mut scope = Scope::new(source, signatures, states, Effect::Command, Type::Unit);
        let parameters = if kind == ViewKind::Input {
            let event = Type::Record(BTreeMap::from([("value".into(), Type::String)]));
            scope.locals.insert("$event".into(), (0, event.clone()));
            scope.next_slot = 1;
            vec![("$event".into(), event)]
        } else { vec![] };
        let expr = scope.expr(&value.expression, None)?;
        let generated = format!("__uix_event_{}", functions.len());
        functions.push(Function { signature: Signature { name: generated.clone(), parameters, returns: expr.ty.clone(), effect: Effect::Command },
            exported: false, local_count: scope.next_slot, body: Body::Dynamic(vec![Statement::Return(Some(expr))]), location: location(source, a.span) });
        Ok(generated)
    }).transpose()?;
    let children = children(element)?;
    if !matches!(kind, ViewKind::Column | ViewKind::Row) && !children.is_empty() { return Err(fail(element.span, "叶控件不能有子节点")); }
    let children = children.into_iter().map(|e| node(e, source, signatures, states, functions, keys)).collect::<Result<_, _>>()?;
    Ok(ViewTemplate { kind, key, properties, handler, children })
}

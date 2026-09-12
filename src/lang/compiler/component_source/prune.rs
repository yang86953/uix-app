//! 按入口的声明可达闭包裁剪；不删语句、不折叠分支、不重排初始化或效果。
use crate::lang::runtime::components as rt;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Component,
    Function,
    Binding,
}
enum Link<'a> {
    Id(Kind, &'a mut usize),
    Native(&'a mut rt::ExportKey),
}

pub(super) fn reachable(mut program: rt::Program) -> rt::Program {
    let mut seen = BTreeSet::new();
    let mut natives = BTreeSet::new();
    let mut pending = vec![(Kind::Component, program.entry)];
    while let Some((kind, id)) = pending.pop() {
        if !seen.insert((kind, id)) {
            continue;
        }
        let mut visit = |link: Link<'_>| match link {
            Link::Id(kind, id) => pending.push((kind, *id)),
            Link::Native(key) => {
                natives.insert(key.clone());
            }
        };
        match kind {
            Kind::Component => component(&mut program.components[id], &mut visit),
            Kind::Function => function(&mut program.functions[id], &mut visit),
            Kind::Binding => binding(&mut program.bindings[id], &mut visit),
        }
    }
    let mut remap = BTreeMap::new();
    for kind in [Kind::Component, Kind::Function, Kind::Binding] {
        for (next, (_, id)) in seen.iter().filter(|(old, _)| *old == kind).enumerate() {
            remap.insert((kind, *id), next);
        }
    }
    program.entry = remap[&(Kind::Component, program.entry)];
    program.components = program
        .components
        .into_iter()
        .enumerate()
        .filter_map(|(id, item)| seen.contains(&(Kind::Component, id)).then_some(item))
        .collect();
    program.functions = program
        .functions
        .into_iter()
        .enumerate()
        .filter_map(|(id, item)| seen.contains(&(Kind::Function, id)).then_some(item))
        .collect();
    program.bindings = program
        .bindings
        .into_iter()
        .enumerate()
        .filter_map(|(id, item)| seen.contains(&(Kind::Binding, id)).then_some(item))
        .collect();
    program.natives.retain(|key, _| natives.contains(key));
    let mut visit = |link: Link<'_>| {
        if let Link::Id(kind, id) = link {
            *id = remap[&(kind, *id)];
        }
    };
    for item in &mut program.components {
        component(item, &mut visit);
    }
    for item in &mut program.functions {
        function(item, &mut visit);
    }
    for item in &mut program.bindings {
        binding(item, &mut visit);
    }
    program
}

fn component(item: &mut rt::Component, visit: &mut impl FnMut(Link<'_>)) {
    visit(Link::Id(Kind::Function, &mut item.render));
    for id in &mut item.inputs {
        visit(Link::Id(Kind::Binding, id));
    }
    for initial in item.defaults.iter_mut().flatten() {
        body(initial, visit);
    }
    for (id, initial) in &mut item.states {
        visit(Link::Id(Kind::Binding, id));
        body(initial, visit);
    }
}
fn function(item: &mut rt::Function, visit: &mut impl FnMut(Link<'_>)) {
    if let Some(id) = &mut item.component {
        visit(Link::Id(Kind::Component, id));
    }
    for id in item.parameters.iter_mut().chain(&mut item.captures) {
        visit(Link::Id(Kind::Binding, id));
    }
    for default in item.defaults.iter_mut().flatten() {
        body(default, visit);
    }
    body(&mut item.body, visit);
}
fn binding(item: &mut rt::Binding, visit: &mut impl FnMut(Link<'_>)) {
    match &mut item.owner {
        rt::Owner::Component(id) => visit(Link::Id(Kind::Component, id)),
        rt::Owner::Function(id) => visit(Link::Id(Kind::Function, id)),
    }
    if let rt::BindingKind::Function(id) = &mut item.kind {
        visit(Link::Id(Kind::Function, id));
    }
}
fn body(body: &mut rt::Body, visit: &mut impl FnMut(Link<'_>)) {
    let rt::Body::Dynamic(items) = body else {
        unreachable!("lowering only produces build IR")
    };
    statements(Arc::make_mut(items), visit);
}
fn statements(items: &mut [rt::ir::Statement], visit: &mut impl FnMut(Link<'_>)) {
    use rt::ir::StatementKind as S;
    for item in items {
        match &mut item.kind {
            S::Let(id, value) | S::Set(id, value) => {
                visit(Link::Id(Kind::Binding, id));
                expression(value, visit);
            }
            S::Evaluate(value) | S::Return(Some(value)) => expression(value, visit),
            S::Return(None) => {}
            S::If(condition, yes, no) => {
                expression(condition, visit);
                statements(yes, visit);
                statements(no, visit);
            }
        }
    }
}
fn expression(item: &mut rt::ir::Expr, visit: &mut impl FnMut(Link<'_>)) {
    use rt::ir::{Attribute, ExprKind as E};
    match &mut item.kind {
        E::Constant(_) => {}
        E::Get(id) => visit(Link::Id(Kind::Binding, id)),
        E::Closure(id) => visit(Link::Id(Kind::Function, id)),
        E::NativeFunction(key) => visit(Link::Native(key)),
        E::Unary { value, .. } | E::Member { value, .. } => expression(value, visit),
        E::Binary { left, right, .. } => {
            expression(left, visit);
            expression(right, visit);
        }
        E::Conditional { condition, yes, no } => {
            expression(condition, visit);
            expression(yes, visit);
            expression(no, visit);
        }
        E::Array { values, .. } | E::Fragment(values) | E::Concat(values) => {
            for value in values {
                expression(value, visit);
            }
        }
        E::Record(values) => {
            for (_, value) in values {
                expression(value, visit);
            }
        }
        E::Index { value, index } => {
            expression(value, visit);
            expression(index, visit);
        }
        E::Call { callee, arguments }
        | E::Method {
            value: callee,
            arguments,
            ..
        } => {
            expression(callee, visit);
            for value in arguments {
                expression(value, visit);
            }
        }
        E::Map {
            value, callback, ..
        } => {
            expression(value, visit);
            expression(callback, visit);
        }
        E::Element {
            target, attributes, ..
        } => {
            match target {
                rt::Target::Component(id) => visit(Link::Id(Kind::Component, id)),
                rt::Target::Native(key) => visit(Link::Native(key)),
            }
            for attribute in attributes {
                let (Attribute::Key(value) | Attribute::Property(_, value)) = attribute;
                expression(value, visit);
            }
        }
    }
}

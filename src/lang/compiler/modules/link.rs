//! 将受检依赖链接为同一个状态权威和执行单元；不创建隐式实例或宿主发现。

use super::*;

pub(super) fn imports(
    root: &mut Module,
    imports: &[(String, Module)],
    span: SourceSpan,
) -> Result<(), Diagnostic> {
    let count = imports
        .iter()
        .map(|(_, module)| module.dependencies.len() + 1)
        .sum::<usize>();
    if count >= 64 {
        return Err(fail(span, "组合超过 64 个模块实例"));
    }
    if imports.iter().any(|(_, module)| {
        module
            .dependencies
            .iter()
            .any(|dependency| dependency.alias.split('.').count() + 1 > 16)
    }) {
        return Err(fail(span, "组合实例依赖超过 16 层"));
    }
    // 在克隆和重定位前检查整体容量，别名扇出不能放大为无界执行产物。
    let cost = module_cost(root)
        + imports
            .iter()
            .map(|(_, module)| module_cost(module))
            .sum::<usize>();
    if cost > 262_144 {
        return Err(fail(span, "组合执行产物超过 262144 个节点"));
    }
    for (alias, module) in imports {
        let offset = root.states.len();
        let prefix = format!("{alias}.");
        root.dependencies.push(ModuleDependency {
            alias: alias.clone(),
            name: module.name.clone(),
            version: module.version.clone(),
            state_schema: module.state_schema,
        });
        root.dependencies
            .extend(module.dependencies.iter().cloned().map(|mut dependency| {
                dependency.alias = format!("{prefix}{}", dependency.alias);
                dependency
            }));
        root.states
            .extend(module.states.iter().cloned().map(|mut state| {
                state.name = format!("{prefix}{}", state.name);
                state
            }));
        root.ports
            .extend(module.ports.iter().cloned().map(|mut port| {
                port.name = format!("{prefix}{}", port.name);
                port
            }));
        root.functions
            .extend(module.functions.iter().cloned().map(|mut function| {
                function.signature.name = format!("{prefix}{}", function.signature.name);
                function.exported = false;
                if let Body::Dynamic(body) = &mut function.body {
                    block(body, &prefix, offset);
                }
                function
            }));
        root.tasks
            .extend(module.tasks.iter().cloned().map(|mut task| {
                task.signature.name = format!("{prefix}{}", task.signature.name);
                task.exported = false;
                for stage in &mut task.stages {
                    if let TaskBody::Dynamic { statements, exit } = &mut stage.body {
                        block(statements, &prefix, offset);
                        match exit {
                            TaskExit::Branch { condition, .. } => {
                                expression(condition, &prefix, offset)
                            }
                            TaskExit::Await {
                                name, arguments, ..
                            }
                            | TaskExit::TailCall { name, arguments } => {
                                *name = format!("{prefix}{name}");
                                arguments
                                    .iter_mut()
                                    .for_each(|argument| expression(argument, &prefix, offset));
                            }
                            _ => {}
                        }
                    }
                }
                task
            }));
    }
    Ok(())
}

fn block(body: &mut [Statement], prefix: &str, offset: usize) {
    for statement in body {
        match statement {
            Statement::Local(_, value)
            | Statement::Evaluate(value)
            | Statement::Return(Some(value)) => expression(value, prefix, offset),
            Statement::State(updates) => {
                for (slot, value) in updates {
                    *slot += offset;
                    expression(value, prefix, offset);
                }
            }
            Statement::If(condition, yes, no) => {
                expression(condition, prefix, offset);
                block(yes, prefix, offset);
                block(no, prefix, offset);
            }
            Statement::Return(None) => {}
        }
    }
}
fn expression(expr: &mut Expr, prefix: &str, offset: usize) {
    match &mut expr.kind {
        ExprKind::State(slot) => *slot += offset,
        ExprKind::Unary { value, .. } | ExprKind::Member { value, .. } => {
            expression(value, prefix, offset)
        }
        ExprKind::Binary { left, right, .. } => {
            expression(left, prefix, offset);
            expression(right, prefix, offset);
        }
        ExprKind::Conditional { condition, yes, no } => {
            expression(condition, prefix, offset);
            expression(yes, prefix, offset);
            expression(no, prefix, offset);
        }
        ExprKind::Array(values) => values
            .iter_mut()
            .for_each(|value| expression(value, prefix, offset)),
        ExprKind::Record(values) => values
            .iter_mut()
            .for_each(|(_, value)| expression(value, prefix, offset)),
        ExprKind::Index { value, index } => {
            expression(value, prefix, offset);
            expression(index, prefix, offset);
        }
        ExprKind::Call { name, arguments } => {
            if !matches!(name.as_str(), "Some" | "None" | "Ok" | "Err") {
                *name = format!("{prefix}{name}");
            }
            arguments
                .iter_mut()
                .for_each(|argument| expression(argument, prefix, offset));
        }
        ExprKind::Method {
            value, arguments, ..
        } => {
            expression(value, prefix, offset);
            arguments
                .iter_mut()
                .for_each(|argument| expression(argument, prefix, offset));
        }
        ExprKind::Map { value, body, .. } => {
            expression(value, prefix, offset);
            expression(body, prefix, offset);
        }
        ExprKind::Local(_) | ExprKind::Literal(_) => {}
    }
}

// 预算计数包含结构类型和表达式，统计不创建临时副本。
pub(super) fn module_cost(module: &Module) -> usize {
    fn ty(t: &Type) -> usize {
        1 + match t {
            Type::Array(t) | Type::Optional(t) => ty(t),
            Type::Result(a, b) => ty(a) + ty(b),
            Type::Record(fields) => fields.values().map(ty).sum(),
            _ => 0,
        }
    }
    fn expr(e: &Expr) -> usize {
        ty(&e.ty)
            + 1
            + match &e.kind {
                ExprKind::Unary { value, .. } | ExprKind::Member { value, .. } => expr(value),
                ExprKind::Binary { left, right, .. } => expr(left) + expr(right),
                ExprKind::Conditional { condition, yes, no } => {
                    expr(condition) + expr(yes) + expr(no)
                }
                ExprKind::Array(values) => values.iter().map(expr).sum(),
                ExprKind::Record(values) => values.iter().map(|(_, value)| expr(value)).sum(),
                ExprKind::Index { value, index } => expr(value) + expr(index),
                ExprKind::Call { arguments, .. } => arguments.iter().map(expr).sum(),
                ExprKind::Method {
                    value, arguments, ..
                } => expr(value) + arguments.iter().map(expr).sum::<usize>(),
                ExprKind::Map { value, body, .. } => expr(value) + expr(body),
                _ => 0,
            }
    }
    fn block(body: &[Statement]) -> usize {
        body.iter()
            .map(|statement| {
                1 + match statement {
                    Statement::Local(_, e)
                    | Statement::Evaluate(e)
                    | Statement::Return(Some(e)) => expr(e),
                    Statement::State(values) => values.iter().map(|(_, e)| expr(e)).sum(),
                    Statement::If(e, yes, no) => expr(e) + block(yes) + block(no),
                    _ => 0,
                }
            })
            .sum()
    }
    fn signature(s: &Signature) -> usize {
        1 + ty(&s.returns) + s.parameters.iter().map(|(_, t)| ty(t)).sum::<usize>()
    }
    module.data_types.values().map(ty).sum::<usize>()
        + module.states.iter().map(|s| ty(&s.ty)).sum::<usize>()
        + module.ports.iter().map(signature).sum::<usize>()
        + module
            .functions
            .iter()
            .map(|f| {
                signature(&f.signature)
                    + match &f.body {
                        Body::Dynamic(body) => block(body),
                        _ => 0,
                    }
            })
            .sum::<usize>()
        + module
            .tasks
            .iter()
            .map(|t| {
                signature(&t.signature)
                    + t.stages
                        .iter()
                        .map(|stage| match &stage.body {
                            TaskBody::Dynamic { statements, exit } => {
                                block(statements)
                                    + match exit {
                                        TaskExit::Branch { condition, .. } => expr(condition),
                                        TaskExit::Await { arguments, .. }
                                        | TaskExit::TailCall { arguments, .. } => {
                                            arguments.iter().map(expr).sum()
                                        }
                                        _ => 0,
                                    }
                            }
                            _ => 0,
                        })
                        .sum::<usize>()
            })
            .sum::<usize>()
}

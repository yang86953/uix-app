// 引入确定性调用图、局部作用域与可变绑定集合。
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

// 引入 action 独立 AST、表达式 AST 与诊断。
use super::{
    ActionBlock, ActionBody, ActionStatement, Diagnostic, Expression, ExpressionKind, WidgetAction,
};

// 验证全部 action 的词法作用域、参数边界与静态无环调用图。
pub(super) fn validate_widget_actions(
    actions: &mut [WidgetAction],
    state_names: &HashSet<String>,
) -> Result<(), Diagnostic> {
    let action_names = actions
        .iter()
        .map(|action| action.name.clone())
        .collect::<BTreeSet<_>>();
    let mut graph = BTreeMap::<String, BTreeSet<String>>::new();
    // 即使 action 没有事件调用点，也必须完成全图语义验证。
    for action in actions.iter_mut() {
        let mut calls = BTreeSet::new();
        let mut scopes = Vec::<ScopeFrame>::new();
        let mut next_binding_id = 0_usize;
        let mut mutable_bindings = HashSet::new();
        match &mut action.body {
            ActionBody::Expression(expression) => {
                validate_expression(
                    expression,
                    &mut scopes,
                    &action_names,
                    &mut calls,
                    &action.name,
                )?;
            }
            ActionBody::Block(block) => {
                validate_block(
                    block,
                    &mut scopes,
                    &action_names,
                    state_names,
                    &mut calls,
                    &action.name,
                    &mut next_binding_id,
                    &mut mutable_bindings,
                )?;
                let mut marker_id = 0;
                mark_mutable_bindings(block, &mut marker_id, &mutable_bindings);
            }
        }
        graph.insert(action.name.clone(), calls);
    }
    reject_recursive_actions(actions, &graph)
}

// 保存一个块当前已经生效的局部名称到唯一声明编号映射。
#[derive(Default)]
struct ScopeFrame {
    bindings: HashMap<String, usize>,
}

#[allow(clippy::too_many_arguments)]
fn validate_block(
    block: &mut ActionBlock,
    scopes: &mut Vec<ScopeFrame>,
    action_names: &BTreeSet<String>,
    state_names: &HashSet<String>,
    calls: &mut BTreeSet<String>,
    action_name: &str,
    next_binding_id: &mut usize,
    mutable_bindings: &mut HashSet<usize>,
) -> Result<(), Diagnostic> {
    scopes.push(ScopeFrame::default());
    for statement in &mut block.statements {
        match statement {
            ActionStatement::Let {
                name,
                initializer,
                span,
                ..
            } => {
                // 初始化先在外层和本块此前绑定中求值。
                validate_expression(initializer, scopes, action_names, calls, action_name)?;
                let current = scopes.last_mut().expect("action 块作用域已压栈");
                if current.bindings.contains_key(name) {
                    let duplicate = Diagnostic::new(
                        *span,
                        format!("action {action_name} 的同一块重复声明局部 {name}"),
                        "删除重复 let，或在内层 if/else 块中使用遮蔽",
                    );
                    scopes.pop();
                    return Err(duplicate);
                }
                let binding_id = *next_binding_id;
                *next_binding_id += 1;
                current.bindings.insert(name.clone(), binding_id);
            }
            ActionStatement::Assign { name, value, span } => {
                validate_expression(value, scopes, action_names, calls, action_name)?;
                let binding = scopes
                    .iter()
                    .rev()
                    .find_map(|scope| scope.bindings.get(name).copied());
                let Some(binding) = binding else {
                    let diagnostic = if state_names.contains(name) {
                        Diagnostic::new(
                            *span,
                            format!("组件 state {name} 不能直接赋值"),
                            format!("使用 setState({name}: expression) 更新组件状态"),
                        )
                    } else {
                        Diagnostic::new(
                            *span,
                            format!("action 局部赋值目标 {name} 尚未声明"),
                            format!("先使用 let {name} = expression; 声明局部"),
                        )
                    };
                    scopes.pop();
                    return Err(diagnostic);
                };
                mutable_bindings.insert(binding);
            }
            ActionStatement::Expression { expression, .. } => {
                validate_expression(expression, scopes, action_names, calls, action_name)?;
            }
            ActionStatement::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                validate_expression(condition, scopes, action_names, calls, action_name)?;
                validate_block(
                    then_block,
                    scopes,
                    action_names,
                    state_names,
                    calls,
                    action_name,
                    next_binding_id,
                    mutable_bindings,
                )?;
                if let Some(else_block) = else_block {
                    validate_block(
                        else_block,
                        scopes,
                        action_names,
                        state_names,
                        calls,
                        action_name,
                        next_binding_id,
                        mutable_bindings,
                    )?;
                }
            }
            ActionStatement::Return { value, .. } => {
                if let Some(value) = value {
                    validate_expression(value, scopes, action_names, calls, action_name)?;
                }
            }
        }
    }
    scopes.pop();
    Ok(())
}

fn validate_expression(
    expression: &Expression,
    scopes: &mut Vec<ScopeFrame>,
    action_names: &BTreeSet<String>,
    calls: &mut BTreeSet<String>,
    action_name: &str,
) -> Result<(), Diagnostic> {
    if matches!(&expression.kind, ExpressionKind::Identifier(name) if name == "$event") {
        return Err(Diagnostic::new(
            expression.span,
            format!("action {action_name} 不能隐式引用 $event"),
            "在事件属性中直接处理 $event，或等待登记带参数 action",
        ));
    }
    if let ExpressionKind::Call { callee, arguments } = &expression.kind {
        if let ExpressionKind::Identifier(name) = &callee.kind {
            let shadowed = scopes
                .iter()
                .rev()
                .any(|scope| scope.bindings.contains_key(name));
            if !shadowed && action_names.contains(name) {
                if !arguments.is_empty() {
                    return Err(Diagnostic::new(
                        expression.span,
                        format!("action {name} 当前不接受参数"),
                        format!("使用 {name}()；带参数 action 将在后续阶段登记"),
                    ));
                }
                calls.insert(name.clone());
            }
        }
    }
    match &expression.kind {
        ExpressionKind::LoweredAction(_) => unreachable!("声明语义早于 action 内联"),
        ExpressionKind::Closure { parameter, body } => {
            let mut frame = ScopeFrame::default();
            frame.bindings.insert(parameter.clone(), usize::MAX);
            scopes.push(frame);
            let result = validate_expression(body, scopes, action_names, calls, action_name);
            scopes.pop();
            result?;
        }
        ExpressionKind::Unary { operand, .. } => {
            validate_expression(operand, scopes, action_names, calls, action_name)?;
        }
        ExpressionKind::Binary { left, right, .. } => {
            validate_expression(left, scopes, action_names, calls, action_name)?;
            validate_expression(right, scopes, action_names, calls, action_name)?;
        }
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            validate_expression(condition, scopes, action_names, calls, action_name)?;
            validate_expression(then_branch, scopes, action_names, calls, action_name)?;
            validate_expression(else_branch, scopes, action_names, calls, action_name)?;
        }
        ExpressionKind::Member { object, .. } => {
            validate_expression(object, scopes, action_names, calls, action_name)?;
        }
        ExpressionKind::Index { object, index } => {
            validate_expression(object, scopes, action_names, calls, action_name)?;
            validate_expression(index, scopes, action_names, calls, action_name)?;
        }
        ExpressionKind::Call { callee, arguments } => {
            validate_expression(callee, scopes, action_names, calls, action_name)?;
            for argument in arguments {
                validate_expression(&argument.value, scopes, action_names, calls, action_name)?;
            }
        }
        ExpressionKind::Object(fields) => {
            for field in fields {
                validate_expression(&field.value, scopes, action_names, calls, action_name)?;
            }
        }
        ExpressionKind::Array(items) => {
            for item in items {
                validate_expression(item, scopes, action_names, calls, action_name)?;
            }
        }
        ExpressionKind::Identifier(_)
        | ExpressionKind::Number(_)
        | ExpressionKind::String(_)
        | ExpressionKind::Boolean(_) => {}
    }
    Ok(())
}

fn mark_mutable_bindings(
    block: &mut ActionBlock,
    next_binding_id: &mut usize,
    mutable_bindings: &HashSet<usize>,
) {
    for statement in &mut block.statements {
        match statement {
            ActionStatement::Let { mutable, .. } => {
                *mutable = mutable_bindings.contains(next_binding_id);
                *next_binding_id += 1;
            }
            ActionStatement::If {
                then_block,
                else_block,
                ..
            } => {
                mark_mutable_bindings(then_block, next_binding_id, mutable_bindings);
                if let Some(else_block) = else_block {
                    mark_mutable_bindings(else_block, next_binding_id, mutable_bindings);
                }
            }
            ActionStatement::Assign { .. }
            | ActionStatement::Expression { .. }
            | ActionStatement::Return { .. } => {}
        }
    }
}

fn reject_recursive_actions(
    actions: &[WidgetAction],
    graph: &BTreeMap<String, BTreeSet<String>>,
) -> Result<(), Diagnostic> {
    for action in actions {
        let mut path = Vec::new();
        let mut active = BTreeSet::new();
        if let Some(cycle) = find_cycle(&action.name, graph, &mut path, &mut active) {
            return Err(Diagnostic::new(
                action.span,
                format!("action 递归调用不受支持：{}", cycle.join(" -> ")),
                "移除 action 自调用或循环调用，把重复数据处理改为受限数组操作",
            ));
        }
    }
    Ok(())
}

fn find_cycle(
    name: &str,
    graph: &BTreeMap<String, BTreeSet<String>>,
    path: &mut Vec<String>,
    active: &mut BTreeSet<String>,
) -> Option<Vec<String>> {
    if let Some(start) = path.iter().position(|entry| entry == name) {
        let mut cycle = path[start..].to_vec();
        cycle.push(name.to_string());
        return Some(cycle);
    }
    if !active.insert(name.to_string()) {
        return None;
    }
    path.push(name.to_string());
    for callee in graph.get(name).into_iter().flatten() {
        if let Some(cycle) = find_cycle(callee, graph, path, active) {
            return Some(cycle);
        }
    }
    path.pop();
    active.remove(name);
    None
}

// 判断 action 主体是否直接包含或经块表达式包含 setStyle。
pub(super) fn action_body_uses_set_style(body: &ActionBody) -> bool {
    let mut found = false;
    let _ = visit_action_body_expressions(body, &mut |expression| {
        if super::dynamic_style_lower::expression_uses_set_style(expression) {
            found = true;
        }
        Ok(())
    });
    found
}

// 只读访问 action 主体中的全部直接表达式根。
pub(super) fn visit_action_body_expressions(
    body: &ActionBody,
    visitor: &mut impl FnMut(&Expression) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    match body {
        ActionBody::Expression(expression) => visitor(expression),
        ActionBody::Block(block) => visit_action_block_expressions(block, visitor),
    }
}

pub(super) fn visit_action_block_expressions(
    block: &ActionBlock,
    visitor: &mut impl FnMut(&Expression) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    for statement in &block.statements {
        match statement {
            ActionStatement::Let { initializer, .. } => visitor(initializer)?,
            ActionStatement::Assign { value, .. } => visitor(value)?,
            ActionStatement::Expression { expression, .. } => visitor(expression)?,
            ActionStatement::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                visitor(condition)?;
                visit_action_block_expressions(then_block, visitor)?;
                if let Some(else_block) = else_block {
                    visit_action_block_expressions(else_block, visitor)?;
                }
            }
            ActionStatement::Return { value, .. } => {
                if let Some(value) = value {
                    visitor(value)?;
                }
            }
        }
    }
    Ok(())
}

// 可变访问已经内联到表达式位置的 action 块全部表达式根。
pub(super) fn visit_action_block_expressions_mut(
    block: &mut ActionBlock,
    visitor: &mut impl FnMut(&mut Expression) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    for statement in &mut block.statements {
        match statement {
            ActionStatement::Let { initializer, .. } => visitor(initializer)?,
            ActionStatement::Assign { value, .. } => visitor(value)?,
            ActionStatement::Expression { expression, .. } => visitor(expression)?,
            ActionStatement::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                visitor(condition)?;
                visit_action_block_expressions_mut(then_block, visitor)?;
                if let Some(else_block) = else_block {
                    visit_action_block_expressions_mut(else_block, visitor)?;
                }
            }
            ActionStatement::Return { value, .. } => {
                if let Some(value) = value {
                    visitor(value)?;
                }
            }
        }
    }
    Ok(())
}

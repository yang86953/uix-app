// 引入调用参数、表达式 AST、诊断与源码跨度。
use super::super::{CallArgument, Diagnostic, Expression, ExpressionKind, SourceSpan};

// 保存要求受限闭包的不可变数组操作名称。
const CLOSURE_ARRAY_OPERATIONS: &[&str] = &["removeBy", "filter", "map", "sortBy", "find"];

// 验证调用目标和内置操作参数规则。
pub(super) fn validate_call(
    // 接收已经解析的调用目标。
    callee: &Expression,
    // 接收源码顺序中的调用参数。
    arguments: &[CallArgument],
    // 接收调用闭合括号跨度。
    close_span: SourceSpan,
) -> Result<(), Diagnostic> {
    // 调用目标只能是标识符或成员路径。
    if !matches!(
        callee.kind,
        ExpressionKind::Identifier(_) | ExpressionKind::Member { .. }
    ) {
        // 返回非法调用目标诊断。
        return Err(Diagnostic::new(
            // 指向调用目标。
            callee.span,
            // 陈述失败原因。
            "调用目标必须是回调或内置操作路径",
            // 给出修复建议。
            "使用 onConfirm()、props.onConfirm() 或文档列出的内置操作",
        ));
    }
    // 只有直接标识符可能是框架内置操作。
    let direct_name = match &callee.kind {
        // 借用直接标识符名称。
        ExpressionKind::Identifier(value) => Some(value.as_str()),
        // 成员路径不属于直接内置操作。
        _ => None,
    };
    // setState 要求至少一个命名参数。
    if direct_name == Some("setState") {
        // 检查非空且全部命名。
        if arguments.is_empty() || arguments.iter().any(|argument| argument.name.is_none()) {
            // 返回 setState 参数诊断。
            return Err(Diagnostic::new(
                // 指向调用结束位置。
                close_span,
                // 陈述失败原因。
                "setState 只接受一个或多个命名参数",
                // 给出合法示例。
                "使用 setState(count: count + 1)",
            ));
        }
        // 命名参数符合约束。
        return Ok(());
    }
    // setTheme 与 setStyle 要求一个字符串位置参数。
    if matches!(direct_name, Some("setTheme" | "setStyle")) {
        // 检查唯一位置字符串参数。
        let valid = arguments.len() == 1
            // 取得唯一参数。
            && arguments[0].name.is_none()
            // 验证字符串 AST。
            && matches!(arguments[0].value.kind, ExpressionKind::String(_));
        // 参数非法时返回专用诊断。
        if !valid {
            // 取得内置操作名称。
            let name = direct_name.expect("已匹配内置操作名称");
            // 返回参数形状诊断。
            return Err(Diagnostic::new(
                // 指向调用结束位置。
                close_span,
                // 陈述失败原因。
                format!("{name} 只接受一个字符串位置参数"),
                // 给出合法示例。
                format!("使用 {name}('name')"),
            ));
        }
        // 内置调用符合约束。
        return Ok(());
    }
    // 成员调用可能属于不可变数组操作集合。
    if let ExpressionKind::Member { member, .. } = &callee.kind {
        // 两参数数组更新必须严格使用两个位置参数。
        if matches!(member.as_str(), "insertAt" | "updateAt") {
            // 验证两个普通位置参数且不把闭包当成数组元素。
            if arguments.len() != 2
                || arguments.iter().any(|argument| {
                    // 命名参数和闭包都不属于该操作形状。
                    argument.name.is_some()
                        || matches!(argument.value.kind, ExpressionKind::Closure { .. })
                })
            {
                // 返回精确参数形状诊断。
                return Err(Diagnostic::new(
                    // 指向调用结束位置。
                    close_span,
                    // 说明索引和值两个参数要求。
                    format!("{member} 必须接收索引和值两个位置参数"),
                    // 给出对应合法示例。
                    format!("使用 array.{member}(index, value)"),
                ));
            }
            // 两参数数组操作通过验证。
            return Ok(());
        }
        // 谓词或映射数组操作必须严格接收一个受限闭包。
        if CLOSURE_ARRAY_OPERATIONS.contains(&member.as_str()) {
            // 验证唯一无名称闭包参数。
            let valid = arguments.len() == 1
                // 唯一参数不能命名。
                && arguments[0].name.is_none()
                // 唯一参数必须是受限闭包 AST。
                && matches!(arguments[0].value.kind, ExpressionKind::Closure { .. });
            // 非闭包参数返回定向诊断。
            if !valid {
                // 返回数组闭包操作诊断。
                return Err(Diagnostic::new(
                    // 指向调用结束位置。
                    close_span,
                    // 说明唯一闭包参数要求。
                    format!("{member} 必须接收一个受限闭包"),
                    // 给出对应合法示例。
                    format!("使用 array.{member}(|item| expression)"),
                ));
            }
            // 闭包数组操作通过验证。
            return Ok(());
        }
    }
    // 普通回调不接受命名参数。
    if arguments.iter().any(|argument| argument.name.is_some()) {
        // 返回命名参数范围诊断。
        return Err(Diagnostic::new(
            // 指向首个命名参数。
            arguments
                // 查找命名参数。
                .iter()
                // 选择首个命名项。
                .find(|argument| argument.name.is_some())
                // 调用条件保证存在。
                .expect("已确认存在命名参数")
                // 使用参数跨度。
                .span,
            // 陈述失败原因。
            "命名参数只允许用于 setState",
            // 给出修复建议。
            "普通回调使用位置参数，或改用 setState(name: value)",
        ));
    }
    // 普通调用的直接闭包参数必须在解析期拒绝。
    if let Some(argument) = arguments
        // 遍历全部普通调用参数。
        .iter()
        // 查找直接闭包参数。
        .find(|argument| matches!(argument.value.kind, ExpressionKind::Closure { .. }))
    {
        // 返回闭包位置诊断。
        return Err(closure_position_error(argument.value.span));
    }
    // 普通回调通过验证。
    Ok(())
}

// 验证完整表达式中没有脱离数组操作参数位置的闭包。
pub(super) fn validate_closure_positions(expression: &Expression) -> Result<(), Diagnostic> {
    // 从根表达式开始且根位置不允许直接闭包。
    validate_closure_position(expression, false)
}

// 递归验证一个表达式位置是否允许受限闭包。
fn validate_closure_position(
    // 接收待检查表达式。
    expression: &Expression,
    // 标记当前位置是否为闭包数组操作的唯一参数。
    closure_allowed: bool,
) -> Result<(), Diagnostic> {
    // 按表达式结构递归检查闭包位置。
    match &expression.kind {
        // 普通表达式解析器不会构造 action 内联桥接节点。
        ExpressionKind::LoweredAction(_) => Ok(()),
        // 闭包只允许位于明确开放的位置。
        ExpressionKind::Closure { body, .. } => {
            // 非开放位置返回定向诊断。
            if !closure_allowed {
                // 返回闭包位置错误。
                return Err(closure_position_error(expression.span));
            }
            // 闭包体本身是普通表达式，但可包含新的数组操作调用。
            validate_closure_position(body, false)
        }
        // 调用需要分别验证目标与各参数位置。
        ExpressionKind::Call { callee, arguments } => {
            // 调用目标永远不是闭包开放位置。
            validate_closure_position(callee, false)?;
            // 判断当前调用是否开放唯一闭包参数。
            let allows_closure = matches!(
                &callee.kind,
                ExpressionKind::Member { member, .. }
                    if CLOSURE_ARRAY_OPERATIONS.contains(&member.as_str())
            );
            // 按源码顺序验证全部参数。
            for (index, argument) in arguments.iter().enumerate() {
                // 只有闭包数组操作的第一个参数允许直接闭包。
                validate_closure_position(&argument.value, allows_closure && index == 0)?;
            }
            // 完成调用子树验证。
            Ok(())
        }
        // 对象字段按普通表达式位置检查。
        ExpressionKind::Object(fields) => fields
            // 遍历字段值。
            .iter()
            // 递归验证每个字段。
            .try_for_each(|field| validate_closure_position(&field.value, false)),
        // 数组元素按普通表达式位置检查。
        ExpressionKind::Array(items) => items
            // 遍历数组元素。
            .iter()
            // 递归验证每个元素。
            .try_for_each(|item| validate_closure_position(item, false)),
        // 一元表达式递归检查操作数。
        ExpressionKind::Unary { operand, .. } => validate_closure_position(operand, false),
        // 二元表达式递归检查两侧。
        ExpressionKind::Binary { left, right, .. } => {
            // 先检查左侧。
            validate_closure_position(left, false)?;
            // 再检查右侧。
            validate_closure_position(right, false)
        }
        // 三元表达式递归检查三个分支。
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            // 检查条件。
            validate_closure_position(condition, false)?;
            // 检查真分支。
            validate_closure_position(then_branch, false)?;
            // 检查假分支。
            validate_closure_position(else_branch, false)
        }
        // 成员访问递归检查对象。
        ExpressionKind::Member { object, .. } => validate_closure_position(object, false),
        // 下标访问递归检查对象与索引。
        ExpressionKind::Index { object, index } => {
            // 检查对象。
            validate_closure_position(object, false)?;
            // 检查索引。
            validate_closure_position(index, false)
        }
        // 叶子表达式不包含闭包。
        ExpressionKind::Identifier(_)
        | ExpressionKind::Number(_)
        | ExpressionKind::String(_)
        | ExpressionKind::Boolean(_) => Ok(()),
    }
}

// 构造统一的闭包位置诊断。
fn closure_position_error(span: SourceSpan) -> Diagnostic {
    // 返回精确闭包跨度与合法位置说明。
    Diagnostic::new(
        // 指向完整闭包。
        span,
        // 说明闭包不属于通用表达式。
        "受限闭包只能用于数组操作方法参数",
        // 列出开放闭包的操作集合。
        "把 |item| expression 用于 removeBy、filter、map、sortBy 或 find",
    )
}

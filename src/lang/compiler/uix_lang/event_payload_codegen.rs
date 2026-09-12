// 引入过程宏卫生标识符与令牌流。
use proc_macro2::{Ident, TokenStream};

// 引入 Compiler System 唯一事件载荷登记表。
use crate::lang::compiler::projection_schema::UI_PROJECTION_SCHEMA;

// 引入表达式 AST、诊断与既有处理器生成入口。
use super::{Diagnostic, Expression, ExpressionKind, generate_handler_expression};

// 校验事件字段后复用既有受限表达式处理器生成。
pub(crate) fn generate_event_handler_expression(
    // 接收事件处理器表达式。
    expression: &Expression,
    // 接收实际运行时载荷局部变量。
    event: &Ident,
    // 接收 UIX 事件属性名。
    event_name: &str,
) -> Result<TokenStream, Diagnostic> {
    // 非键盘事件不提供修饰键载荷。
    generate_key_event_handler_expression(expression, event, event_name, None)
}

// 校验键盘事件字段后生成处理器；修饰键载荷提供时 $event.mods 改写为该局部。
pub(crate) fn generate_key_event_handler_expression(
    // 接收事件处理器表达式。
    expression: &Expression,
    // 接收实际运行时 KeyCode 载荷局部变量。
    event: &Ident,
    // 接收 UIX 事件属性名。
    event_name: &str,
    // 接收可选的键盘修饰键载荷局部；提供时 $event.mods 改写为该局部。
    modifier_payload: Option<&Ident>,
) -> Result<TokenStream, Diagnostic> {
    // 在生成 Rust 前验证全部 $event 顶层字段。
    validate_event_payload_fields(expression, event_name, registered_fields(event_name))?;
    // 键盘事件把 $event.mods 改写为修饰键载荷局部后按既有路径生成。
    if let Some(mods) = modifier_payload {
        // 克隆表达式以便在生成前完成限定改写。
        let mut lowered = expression.clone();
        // 递归替换全部 $event.mods 成员访问。
        rewrite_modifier_access(&mut lowered, mods);
        // 复用既有处理器调用与来源标记生成。
        return generate_handler_expression(&lowered, Some(event));
    }
    // 复用既有处理器调用与来源标记生成。
    generate_handler_expression(expression, Some(event))
}

// 递归把 $event.mods 成员访问改写为修饰键载荷局部引用。
fn rewrite_modifier_access(
    // 接收待改写表达式。
    expression: &mut Expression,
    // 接收修饰键载荷局部变量。
    mods: &Ident,
) {
    // 按表达式形状递归。
    match &mut expression.kind {
        // action 块由独立降低路径处理，内部不含 $event。
        ExpressionKind::LoweredAction(_) => {}
        // 成员访问优先识别直接的 $event.mods 形状。
        ExpressionKind::Member { object, member } => {
            // 命中 $event.mods 时整体替换为载荷局部引用。
            if matches!(&object.kind, ExpressionKind::Identifier(name) if name == "$event")
                && member == "mods"
            {
                // 沿用原成员访问跨度，避免诊断漂移。
                let span = expression.span;
                // 替换为卫生载荷局部标识符。
                expression.kind = ExpressionKind::Identifier(mods.to_string());
                // 同步刷新被替换节点的跨度。
                expression.span = span;
                // 该节点已完成改写，不再下钻。
                return;
            }
            // 普通成员对象继续递归。
            rewrite_modifier_access(object, mods);
        }
        // 一元表达式递归操作数。
        ExpressionKind::Unary { operand, .. } => rewrite_modifier_access(operand, mods),
        // 二元表达式递归两侧。
        ExpressionKind::Binary { left, right, .. } => {
            rewrite_modifier_access(left, mods);
            rewrite_modifier_access(right, mods);
        }
        // 三元表达式递归条件与两支。
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            rewrite_modifier_access(condition, mods);
            rewrite_modifier_access(then_branch, mods);
            rewrite_modifier_access(else_branch, mods);
        }
        // 索引访问递归对象与索引。
        ExpressionKind::Index { object, index } => {
            rewrite_modifier_access(object, mods);
            rewrite_modifier_access(index, mods);
        }
        // 调用递归目标与全部参数。
        ExpressionKind::Call { callee, arguments } => {
            rewrite_modifier_access(callee, mods);
            for argument in arguments {
                rewrite_modifier_access(&mut argument.value, mods);
            }
        }
        // 对象字面量递归全部字段值。
        ExpressionKind::Object(fields) => {
            for field in fields {
                rewrite_modifier_access(&mut field.value, mods);
            }
        }
        // 数组字面量递归全部元素。
        ExpressionKind::Array(items) => {
            for item in items {
                rewrite_modifier_access(item, mods);
            }
        }
        // 受限闭包递归表达式体。
        ExpressionKind::Closure { body, .. } => rewrite_modifier_access(body, mods),
        // 标识符与字面量没有成员访问。
        ExpressionKind::Identifier(_)
        | ExpressionKind::Number(_)
        | ExpressionKind::String(_)
        | ExpressionKind::Boolean(_) => {}
    }
}

// 递归验证表达式中的事件顶层成员。
pub(super) fn validate_event_payload_fields(
    // 接收待检查表达式。
    expression: &Expression,
    // 接收当前事件类型。
    event_name: &str,
    allowed_fields: &[&str],
) -> Result<(), Diagnostic> {
    // 按表达式形状递归。
    match &expression.kind {
        // action 语义禁止 $event，内部块无需事件字段映射。
        ExpressionKind::LoweredAction(_) => Ok(()),
        // 成员访问先校验直接 $event 字段，再递归普通对象。
        ExpressionKind::Member { object, member } => {
            // 直接 $event.字段 必须存在于当前事件登记表。
            if matches!(&object.kind, ExpressionKind::Identifier(name) if name == "$event") {
                // 查找当前事件已登记字段。
                let fields = allowed_fields;
                // 未登记字段返回编译期诊断。
                if !fields.contains(&member.as_str()) {
                    // 构造包含合法字段集合的诊断。
                    return Err(Diagnostic::new(
                        // 指向完整成员访问。
                        expression.span,
                        // 陈述未知字段与事件类型。
                        format!("事件 {event_name} 未登记 $event.{member} 字段"),
                        // 给出当前事件精确字段清单。
                        if fields.is_empty() {
                            // 当前事件没有结构化字段。
                            "该事件只支持直接使用 $event，不能访问成员".to_string()
                        } else {
                            // 展示已登记成员。
                            format!(
                                // 拼接所有合法成员。
                                "使用 {}",
                                // 把字段映射为语言成员写法。
                                fields
                                    // 遍历字段。
                                    .iter()
                                    // 增加 $event 前缀。
                                    .map(|field| format!("$event.{field}"))
                                    // 收集为有序字符串列表。
                                    .collect::<Vec<_>>()
                                    // 使用中文顿号连接。
                                    .join("、")
                            )
                        },
                    ));
                }
                // 直接事件成员已完成验证，不再把保留标识符当普通字段递归。
                return Ok(());
            }
            // 普通成员对象可能继续包含事件访问。
            validate_event_payload_fields(object, event_name, allowed_fields)
        }
        // 一元表达式递归验证操作数。
        ExpressionKind::Unary { operand, .. } => {
            // 验证唯一操作数。
            validate_event_payload_fields(operand, event_name, allowed_fields)
        }
        // 二元表达式递归验证左右两侧。
        ExpressionKind::Binary { left, right, .. } => {
            // 先验证左侧。
            validate_event_payload_fields(left, event_name, allowed_fields)?;
            // 再验证右侧。
            validate_event_payload_fields(right, event_name, allowed_fields)
        }
        // 三元表达式递归验证条件与两个分支。
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            // 验证条件。
            validate_event_payload_fields(condition, event_name, allowed_fields)?;
            // 验证真分支。
            validate_event_payload_fields(then_branch, event_name, allowed_fields)?;
            // 验证假分支。
            validate_event_payload_fields(else_branch, event_name, allowed_fields)
        }
        // 索引访问递归验证对象与索引。
        ExpressionKind::Index { object, index } => {
            // 验证被索引对象。
            validate_event_payload_fields(object, event_name, allowed_fields)?;
            // 验证索引表达式。
            validate_event_payload_fields(index, event_name, allowed_fields)
        }
        // 调用递归验证目标与全部参数。
        ExpressionKind::Call { callee, arguments } => {
            // 验证调用目标。
            validate_event_payload_fields(callee, event_name, allowed_fields)?;
            // 逐一验证参数值。
            for argument in arguments {
                // 验证当前参数。
                validate_event_payload_fields(&argument.value, event_name, allowed_fields)?;
            }
            // 全部调用子表达式有效。
            Ok(())
        }
        // 对象字面量递归验证全部字段值。
        ExpressionKind::Object(fields) => {
            // 逐一验证对象字段。
            for field in fields {
                // 验证当前字段值。
                validate_event_payload_fields(&field.value, event_name, allowed_fields)?;
            }
            // 全部对象字段有效。
            Ok(())
        }
        // 数组字面量递归验证全部项目。
        ExpressionKind::Array(items) => {
            // 逐一验证数组项目。
            for item in items {
                // 验证当前项目。
                validate_event_payload_fields(item, event_name, allowed_fields)?;
            }
            // 全部数组项目有效。
            Ok(())
        }
        // 受限闭包递归验证表达式体。
        ExpressionKind::Closure { body, .. } => {
            // 验证闭包体。
            validate_event_payload_fields(body, event_name, allowed_fields)
        }
        // 标识符与字面量没有事件成员。
        ExpressionKind::Identifier(_)
        | ExpressionKind::Number(_)
        | ExpressionKind::String(_)
        | ExpressionKind::Boolean(_) => {
            // 返回无需校验。
            Ok(())
        }
    }
}

// 查询指定事件的已登记顶层载荷字段。
fn registered_fields(event_name: &str) -> &'static [&'static str] {
    UI_PROJECTION_SCHEMA
        .event(event_name)
        .map(|entry| entry.fields)
        .unwrap_or(&[])
}

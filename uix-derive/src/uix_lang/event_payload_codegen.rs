// 引入过程宏卫生标识符与令牌流。
use proc_macro2::{Ident, TokenStream};

// 引入表达式 AST、诊断与既有处理器生成入口。
use super::{Diagnostic, Expression, ExpressionKind, generate_handler_expression};

// 保存事件名到允许顶层载荷字段的唯一登记表。
const EVENT_PAYLOAD_FIELDS: &[(&str, &[&str])] = &[
    // 点击载荷公开逻辑坐标。
    ("@click", &["x", "y"]),
    // 值变化事件统一公开当前变化事实。
    ("@change", &["value"]),
    // 选择事件统一公开当前选择事实。
    ("@select", &["value"]),
    // 表单提交事件统一公开提交模型。
    ("@submit", &["value"]),
    // 关闭事件统一公开关闭事实。
    ("@close", &["value"]),
    // 键盘按下公开逻辑键与稳定调试代码。
    ("@keyDown", &["key", "code"]),
    // 键盘抬起使用同一字段集合。
    ("@keyUp", &["key", "code"]),
];

// 校验事件字段后复用既有受限表达式处理器生成。
pub(crate) fn generate_event_handler_expression(
    // 接收事件处理器表达式。
    expression: &Expression,
    // 接收实际运行时载荷局部变量。
    event: &Ident,
    // 接收 UIX 事件属性名。
    event_name: &str,
) -> Result<TokenStream, Diagnostic> {
    // 在生成 Rust 前验证全部 $event 顶层字段。
    validate_event_payload_fields(expression, event_name)?;
    // 复用既有处理器调用与来源标记生成。
    generate_handler_expression(expression, Some(event))
}

// 递归验证表达式中的事件顶层成员。
fn validate_event_payload_fields(
    // 接收待检查表达式。
    expression: &Expression,
    // 接收当前事件类型。
    event_name: &str,
) -> Result<(), Diagnostic> {
    // 按表达式形状递归。
    match &expression.kind {
        // 成员访问先校验直接 $event 字段，再递归普通对象。
        ExpressionKind::Member { object, member } => {
            // 直接 $event.字段 必须存在于当前事件登记表。
            if matches!(&object.kind, ExpressionKind::Identifier(name) if name == "$event") {
                // 查找当前事件已登记字段。
                let fields = registered_fields(event_name);
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
            validate_event_payload_fields(object, event_name)
        }
        // 一元表达式递归验证操作数。
        ExpressionKind::Unary { operand, .. } => {
            // 验证唯一操作数。
            validate_event_payload_fields(operand, event_name)
        }
        // 二元表达式递归验证左右两侧。
        ExpressionKind::Binary { left, right, .. } => {
            // 先验证左侧。
            validate_event_payload_fields(left, event_name)?;
            // 再验证右侧。
            validate_event_payload_fields(right, event_name)
        }
        // 三元表达式递归验证条件与两个分支。
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            // 验证条件。
            validate_event_payload_fields(condition, event_name)?;
            // 验证真分支。
            validate_event_payload_fields(then_branch, event_name)?;
            // 验证假分支。
            validate_event_payload_fields(else_branch, event_name)
        }
        // 索引访问递归验证对象与索引。
        ExpressionKind::Index { object, index } => {
            // 验证被索引对象。
            validate_event_payload_fields(object, event_name)?;
            // 验证索引表达式。
            validate_event_payload_fields(index, event_name)
        }
        // 调用递归验证目标与全部参数。
        ExpressionKind::Call { callee, arguments } => {
            // 验证调用目标。
            validate_event_payload_fields(callee, event_name)?;
            // 逐一验证参数值。
            for argument in arguments {
                // 验证当前参数。
                validate_event_payload_fields(&argument.value, event_name)?;
            }
            // 全部调用子表达式有效。
            Ok(())
        }
        // 对象字面量递归验证全部字段值。
        ExpressionKind::Object(fields) => {
            // 逐一验证对象字段。
            for field in fields {
                // 验证当前字段值。
                validate_event_payload_fields(&field.value, event_name)?;
            }
            // 全部对象字段有效。
            Ok(())
        }
        // 数组字面量递归验证全部项目。
        ExpressionKind::Array(items) => {
            // 逐一验证数组项目。
            for item in items {
                // 验证当前项目。
                validate_event_payload_fields(item, event_name)?;
            }
            // 全部数组项目有效。
            Ok(())
        }
        // 受限闭包递归验证表达式体。
        ExpressionKind::Closure { body, .. } => {
            // 验证闭包体。
            validate_event_payload_fields(body, event_name)
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
    // 在线性小表中查找精确事件名。
    EVENT_PAYLOAD_FIELDS
        // 遍历静态登记条目。
        .iter()
        // 选择名称完全匹配的事件。
        .find_map(|(name, fields)| (*name == event_name).then_some(*fields))
        // 未登记事件没有结构化字段。
        .unwrap_or(&[])
}

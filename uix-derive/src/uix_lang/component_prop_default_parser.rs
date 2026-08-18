// 引入表达式 AST、诊断、源码跨度与解析辅助。
use super::{
    Diagnostic, Expression, ExpressionKind, SourceSpan, is_data_constructor_chain, parse_expression,
};

// 切分 prop 类型和可选默认表达式，并验证默认值仅为字面量或数据构造链。
pub(super) fn split_prop_type_default(
    // 接收冒号后的完整类型与默认值源码。
    source: &str,
    // 接收 props 属性跨度。
    span: SourceSpan,
) -> Result<(&str, Option<Expression>), Diagnostic> {
    // 查找不在字符串或嵌套结构中的首个独立等号。
    let separator = top_level_default_separator(source);
    // 没有等号时保持必填 prop。
    let Some(separator) = separator else {
        // 返回原始类型与空默认值。
        return Ok((source.trim(), None));
    };
    // 提取并清理类型源码。
    let type_source = source[..separator].trim();
    // 提取并清理默认表达式源码。
    let default_source = source[separator + 1..].trim();
    // 类型与默认值都不能为空。
    if type_source.is_empty() || default_source.is_empty() {
        // 返回不完整默认声明诊断。
        return Err(Diagnostic::new(
            // 指向完整 props 属性。
            span,
            // 陈述默认声明缺失部分。
            "prop 默认值声明必须同时包含类型和表达式",
            // 给出规范形状。
            "使用 name: Type = 默认表达式",
        ));
    }
    // 使用共享受限表达式解析器构造默认值 AST。
    let default = parse_expression(default_source, span)?;
    // 默认值只接受直接字面量或已登记数据构造调用链。
    let allowed = matches!(
        // 检查直接字面量形状。
        default.kind,
        // 字符串字面量。
        ExpressionKind::String(_)
            // 数字字面量。
            | ExpressionKind::Number(_)
            // 布尔字面量。
            | ExpressionKind::Boolean(_)
    ) || matches!(default.kind, ExpressionKind::Call { .. })
        // 调用链根必须是已登记数据类型。
        && is_data_constructor_chain(&default);
    // 其他表达式可能读取调用方或产生不稳定副作用，必须拒绝。
    if !allowed {
        // 返回默认值边界诊断。
        return Err(Diagnostic::new(
            // 指向完整 props 属性。
            span,
            // 陈述允许集合。
            "prop 默认值只接受字面量或已登记数据类型构造",
            // 给出两个合法示例。
            "使用 label: String = '确定' 或 date: Date = Date(2026, 8, 15)",
        ));
    }
    // 返回清理后的类型与已验证默认表达式。
    Ok((type_source, Some(default)))
}

// 查找顶层独立等号，忽略字符串和圆括号内的数据构造参数。
fn top_level_default_separator(source: &str) -> Option<usize> {
    // 保存当前字符串引号；UIX 表达式使用单引号或双引号。
    let mut quote = None;
    // 保存上一字符是否为转义符。
    let mut escaped = false;
    // 保存圆括号深度。
    let mut parentheses = 0usize;
    // 保存方括号深度。
    let mut brackets = 0usize;
    // 保存花括号深度。
    let mut braces = 0usize;
    // 按 UTF-8 字符边界扫描源码。
    for (index, character) in source.char_indices() {
        // 字符串内部只处理转义与闭合引号。
        if let Some(active_quote) = quote {
            // 转义字符后的当前字符不结束字符串。
            if escaped {
                // 消费一次转义状态。
                escaped = false;
            } else if character == '\\' {
                // 标记下一字符被转义。
                escaped = true;
            } else if character == active_quote {
                // 关闭当前字符串。
                quote = None;
            }
            // 字符串内容不参与结构扫描。
            continue;
        }
        // 按普通结构字符更新深度或识别分隔符。
        match character {
            // 进入单引号或双引号字符串。
            '\'' | '"' => quote = Some(character),
            // 进入圆括号。
            '(' => parentheses += 1,
            // 离开圆括号并防御下溢。
            ')' => parentheses = parentheses.saturating_sub(1),
            // 进入方括号。
            '[' => brackets += 1,
            // 离开方括号并防御下溢。
            ']' => brackets = brackets.saturating_sub(1),
            // 进入花括号。
            '{' => braces += 1,
            // 离开花括号并防御下溢。
            '}' => braces = braces.saturating_sub(1),
            // 仅最外层等号分隔默认表达式。
            '=' if parentheses == 0 && brackets == 0 && braces == 0 => return Some(index),
            // 其他字符不影响扫描状态。
            _ => {}
        }
    }
    // 没有发现顶层默认值分隔符。
    None
}

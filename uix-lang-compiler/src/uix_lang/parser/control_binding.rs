// 拆分自 parser.rs：If/For 控制绑定解析与校验。
// 引入控制绑定解析所需的 AST、词法游标和诊断。
use super::super::{ControlBinding, Cursor, Diagnostic, ExpressionKind, SourceSpan};
// 引入拆分后的表达式节点解析入口。
use super::element_parser::parse_braced_expression_node;

pub(super) fn parse_control_binding(
    cursor: &mut Cursor<'_>,
    element_name: &str,
) -> Result<ControlBinding, Diagnostic> {
    // If 与 ElseIf 都直接保存一个条件表达式。
    if matches!(element_name, "If" | "ElseIf") {
        // 解析并返回条件绑定。
        return Ok(ControlBinding::If(parse_braced_expression_node(cursor)?));
    }
    // 其他标签除 For 外不允许匿名花括号绑定。
    if element_name != "For" {
        // 返回控制绑定位置诊断。
        return Err(Diagnostic::new(
            // 指向花括号起点。
            cursor.point_span(),
            // 陈述失败原因。
            format!("元素 <{element_name}> 不支持控制绑定"),
            // 给出支持结构。
            "只在 <If {condition}>、<ElseIf {condition}> 或 <For {item} in {items}> 中使用",
        ));
    }
    // 解析 For 的单标识符绑定声明。
    let binding_expression = parse_braced_expression_node(cursor)?;
    // 绑定必须是没有成员或运算的单标识符。
    let binding = match &binding_expression.expression.kind {
        // 提取合法标识符。
        ExpressionKind::Identifier(value) => value.clone(),
        // 其他表达式不能声明循环项。
        _ => {
            // 返回绑定形状诊断。
            return Err(Diagnostic::new(
                // 指向绑定表达式。
                binding_expression.expression.span,
                // 陈述失败原因。
                "For 绑定必须是单个标识符",
                // 给出合法示例。
                "使用 <For {item} in {items}>",
            ));
        }
    };
    // 跳过绑定后的空白或注释。
    cursor.skip_trivia()?;
    // 可选解析第二个索引绑定。
    let (index_binding, index_span) = if cursor.starts_with("{") {
        // 解析索引绑定表达式。
        let index_expression = parse_braced_expression_node(cursor)?;
        // 索引绑定同样必须是单标识符。
        let index_name = match &index_expression.expression.kind {
            // 提取合法索引标识符。
            ExpressionKind::Identifier(value) => value.clone(),
            // 其他表达式不能声明索引。
            _ => {
                // 返回索引绑定形状诊断。
                return Err(Diagnostic::new(
                    // 指向索引绑定表达式。
                    index_expression.expression.span,
                    // 陈述失败原因。
                    "For 索引绑定必须是单个标识符",
                    // 给出合法示例。
                    "使用 <For {item} {index} in {items}>",
                ));
            }
        };
        // 循环项和索引不能声明为同名绑定。
        if index_name == binding {
            // 返回重复绑定诊断。
            return Err(Diagnostic::new(
                // 指向重复索引绑定。
                index_expression.expression.span,
                // 陈述失败原因。
                "For 的循环项与索引绑定不能同名",
                // 给出不同名称示例。
                "使用 <For {item} {index} in {items}>",
            ));
        }
        // 保存名称与声明跨度。
        (
            // 保存索引名称。
            Some(index_name),
            // 保存索引跨度。
            Some(index_expression.expression.span),
        )
    } else {
        // 没有第二绑定时保持空值。
        (None, None)
    };
    // 跳过可选索引绑定后的空白或注释。
    cursor.skip_trivia()?;
    // 要求小写 in 关键字。
    let Some((keyword, keyword_span)) = cursor.identifier() else {
        // 返回缺少 in 诊断。
        return Err(Diagnostic::new(
            // 指向当前位置。
            cursor.point_span(),
            // 陈述失败原因。
            "For 绑定缺少 in",
            // 给出合法示例。
            "使用 <For {item} in {items}>",
        ));
    };
    // 拒绝其他标识符充当 in。
    if keyword != "in" {
        // 返回错误关键字诊断。
        return Err(Diagnostic::new(
            // 指向错误关键字。
            keyword_span,
            // 陈述失败原因。
            format!("For 绑定期望 in，但得到 {keyword}"),
            // 给出合法示例。
            "使用 <For {item} in {items}>",
        ));
    }
    // 跳过 in 后空白或注释。
    cursor.skip_trivia()?;
    // 数据源必须使用花括号表达式。
    if !cursor.starts_with("{") {
        // 返回缺失数据源诊断。
        return Err(Diagnostic::new(
            // 指向数据源位置。
            cursor.point_span(),
            // 陈述失败原因。
            "For 的数据源必须是花括号表达式",
            // 给出合法示例。
            "使用 <For {item} in {items}>",
        ));
    }
    // 解析并验证数据源表达式。
    let iterable = parse_braced_expression_node(cursor)?;
    // 返回完整 For 绑定。
    Ok(ControlBinding::For {
        // 保存绑定名。
        binding,
        // 保存单标识符跨度。
        binding_span: binding_expression.expression.span,
        // 保存可选索引绑定。
        index_binding,
        // 保存可选索引跨度。
        index_span,
        // 保存数据源。
        iterable,
        // key 属性由开始标签后续解析。
        key: None,
    })
}

// 验证 If、ElseIf 与 For 元素没有遗漏规范要求的绑定。
pub(super) fn require_control_binding(
    element_name: &str,
    control: Option<&ControlBinding>,
    span: SourceSpan,
) -> Result<(), Diagnostic> {
    // 普通元素或已有绑定的控制元素直接通过。
    if !matches!(element_name, "If" | "ElseIf" | "For") || control.is_some() {
        // 报告结构有效。
        return Ok(());
    }
    // 返回缺失控制绑定诊断。
    Err(Diagnostic::new(
        // 指向开始标签结束位置。
        span,
        // 陈述失败原因。
        format!("<{element_name}> 缺少控制绑定"),
        // 给出对应合法结构。
        if matches!(element_name, "If" | "ElseIf") {
            // 返回条件分支修复示例。
            "使用 <If {condition}> 或 <ElseIf {condition}>"
        } else {
            // 返回 For 修复示例。
            "使用 <For {item} in {items}>"
        },
    ))
}

// 读取花括号内容并构造已验证表达式节点。

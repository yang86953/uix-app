// 引入过程宏卫生标识符与令牌流。
use proc_macro2::{Ident, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入控制绑定、诊断、元素、表达式节点与有序节点 AST。
use super::{
    ControlBinding, Diagnostic, Element, ExpressionNode, Node, SourceSpan, generate_expression,
};
// 引入分支子节点生成与组件作用域传播入口。
use super::codegen::generate_scoped_child_statements;

// 保存一个已经完成相邻配对的条件链分支。
struct ConditionalBranch<'a> {
    // 保存分支元素以生成内容和定位诊断。
    element: &'a Element,
    // If 与 ElseIf 保存条件，Else 使用 None。
    condition: Option<&'a ExpressionNode>,
}

// 从指定 If 开始生成单次短路条件链，并返回下一个未消费节点索引。
pub(super) fn generate_conditional_chain(
    // 接收父元素的有序子节点。
    children: &[Node],
    // 接收当前 If 所在索引。
    start: usize,
    // 接收最终追加 View 的目标向量。
    output: &Ident,
) -> Result<(TokenStream, usize), Diagnostic> {
    // 起始节点必须是带条件的 If。
    let Some(Node::Element(first)) = children.get(start) else {
        // 防御性返回内部路由诊断。
        return Err(invalid_chain_start(children, start));
    };
    // 提取起始 If 条件。
    let first_condition = branch_condition(first, "If")?;
    // 保存按源码顺序配对的分支。
    let mut branches = vec![ConditionalBranch {
        // 保存起始 If 元素。
        element: first,
        // 保存起始 If 条件。
        condition: Some(first_condition),
    }];
    // 从 If 后一个节点开始查找相邻分支。
    let mut cursor = start + 1;
    // 记录是否已经消费兜底 Else。
    let mut saw_else = false;
    // 持续消费空白与相邻分支。
    loop {
        // 空白文本不打断相邻关系。
        cursor = skip_whitespace(children, cursor);
        // 文件末尾结束条件链。
        let Some(Node::Element(element)) = children.get(cursor) else {
            // 非元素或末尾均结束配对。
            break;
        };
        // 根据相邻元素名称决定是否归入链。
        match element.name.as_str() {
            // ElseIf 在 Else 前继续扩展条件链。
            "ElseIf" if !saw_else => {
                // 提取 ElseIf 条件。
                let condition = branch_condition(element, "ElseIf")?;
                // 保存条件分支。
                branches.push(ConditionalBranch {
                    // 保存当前元素。
                    element,
                    // 保存当前条件。
                    condition: Some(condition),
                });
                // 消费当前分支。
                cursor += 1;
            }
            // 首个 Else 作为兜底分支。
            "Else" if !saw_else => {
                // Else 不得携带控制条件。
                if element.control.is_some() {
                    // 返回非法 Else 绑定诊断。
                    return Err(Diagnostic::new(
                        // 指向完整 Else。
                        element.span,
                        // 陈述 Else 不接受条件。
                        "Else 不能声明条件",
                        // 指向 ElseIf 替代结构。
                        "需要条件时改用 <ElseIf {condition}>",
                    ));
                }
                // 保存兜底分支。
                branches.push(ConditionalBranch {
                    // 保存当前元素。
                    element,
                    // None 表示无条件兜底。
                    condition: None,
                });
                // 标记条件链已经闭合。
                saw_else = true;
                // 消费当前分支。
                cursor += 1;
            }
            // Else 后再次出现 Else 或 ElseIf 是重复尾部分支。
            "Else" | "ElseIf" if saw_else => {
                // 返回明确的重复 Else 诊断。
                return Err(Diagnostic::new(
                    // 指向重复尾部分支。
                    element.span,
                    // 陈述条件链已存在 Else。
                    format!(
                        "<{}> 出现在 Else 之后，条件链只能有一个尾部分支",
                        element.name
                    ),
                    // 给出删除或前移建议。
                    "删除重复分支，或把有条件分支移动到 Else 之前",
                ));
            }
            // 其他元素会终止相邻链。
            _ => break,
        }
    }
    // 从尾部向前构造嵌套 if/else if/else，确保条件只短路求值一次。
    let mut generated_tail: Option<TokenStream> = None;
    // 逆序处理全部已配对分支。
    for branch in branches.into_iter().rev() {
        // 生成当前分支内有序子节点及组件作用域传播。
        let body = generate_scoped_child_statements(
            // 传递当前分支子节点。
            &branch.element.children,
            // 传递外层输出向量。
            output,
            // 传递控制元素继承的组件作用域标记。
            &branch.element.widget_scopes,
        )?;
        // 按条件分支或无条件兜底生成当前尾部。
        generated_tail = Some(if let Some(condition) = branch.condition {
            // 生成当前条件表达式。
            let condition = generate_expression(&condition.expression, None)?;
            // 存在后继分支时生成 else 嵌套。
            if let Some(tail) = generated_tail {
                // 生成短路条件与后继链。
                quote! { if #condition { #body } else #tail }
            } else {
                // 没有后继分支时只生成单个 If。
                quote! { if #condition { #body } }
            }
        } else {
            // Else 生成可直接接在 else 后的块表达式。
            quote! {{ #body }}
        });
    }
    // 起始 If 保证至少产生一个分支令牌。
    let generated = generated_tail.ok_or_else(|| invalid_chain_start(children, start))?;
    // 返回完整条件链与下一个未消费索引。
    Ok((generated, cursor))
}

// 提取 If 或 ElseIf 的已解析条件。
fn branch_condition<'a>(
    // 接收待检查分支元素。
    element: &'a Element,
    // 接收期望标签名。
    expected: &str,
) -> Result<&'a ExpressionNode, Diagnostic> {
    // 标签与绑定必须同时匹配。
    if element.name == expected {
        // 提取复用的 If 条件绑定变体。
        if let Some(ControlBinding::If(condition)) = element.control.as_ref() {
            // 返回条件节点。
            return Ok(condition);
        }
    }
    // 返回控制结构损坏诊断。
    Err(Diagnostic::new(
        // 指向完整分支。
        element.span,
        // 陈述缺少条件。
        format!("<{}> 缺少匹配的条件绑定", element.name),
        // 给出规范结构。
        format!("使用 <{expected} {{condition}}>...</{expected}>"),
    ))
}

// 跳过不参与渲染的空白文本节点。
fn skip_whitespace(children: &[Node], mut cursor: usize) -> usize {
    // 连续消费全部空白文本。
    while matches!(children.get(cursor), Some(Node::Text(text)) if text.value.trim().is_empty()) {
        // 前进到下一个候选节点。
        cursor += 1;
    }
    // 返回首个非空白节点索引。
    cursor
}

// 构造不可能的链起始位置诊断。
fn invalid_chain_start(children: &[Node], start: usize) -> Diagnostic {
    // 优先使用现有节点跨度，否则使用零长度回退跨度。
    let span = children
        // 读取候选节点。
        .get(start)
        // 从不同节点形状提取跨度。
        .map(|node| match node {
            // 元素使用完整元素跨度。
            Node::Element(element) => element.span,
            // 文本使用文本跨度。
            Node::Text(text) => text.span,
            // 插值使用表达式节点跨度。
            Node::Interpolation(expression) => expression.span,
            // 成员块保留完整声明跨度。
            Node::WidgetMember(block) => block.span,
        })
        // 越界只可能来自内部调用错误，使用文件起点作防御性定位。
        .unwrap_or(SourceSpan {
            // 文件起点字节偏移。
            start: 0,
            // 零长度防御跨度。
            end: 0,
            // 用户诊断使用一基行号。
            line: 1,
            // 用户诊断使用一基列号。
            column: 1,
        });
    // 返回内部路由诊断。
    Diagnostic::new(
        // 指向候选起点。
        span,
        // 陈述内部条件链路由不一致。
        "条件链生成入口未指向 If",
        // 给出重新解析建议。
        "使用规范 <If {condition}>...</If> 结构重新声明条件分支",
    )
}

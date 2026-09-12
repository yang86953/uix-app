// 引入确定性捕获名称映射与闭包局部集合。
use std::collections::{BTreeMap, BTreeSet};

// 引入卫生标识符、跨度与生成令牌。
use proc_macro2::{Ident, Span, TokenStream};
// 引入确定性 Rust 令牌拼接宏。
use quote::quote;

// 引入受限表达式、文本节点与诊断契约。
use super::{Diagnostic, Expression, ExpressionKind, Node, SourceSpan};
// 引入共享文本内容生成与数据构造名称判定。
use super::{codegen::generate_text_content, is_registered_data_type};

pub(super) fn generate_text_function(children: &[Node], span: SourceSpan) -> Result<TokenStream, Diagnostic> {
    // 克隆节点以便只改写生成期副本。
    let mut lowered = children.to_vec();
    // 保存原标识符到卫生捕获标识符的确定映射。
    let mut captures = BTreeMap::new();
    // 记录受限闭包参数，避免把词法局部误判为外部捕获。
    let mut locals = Vec::new();
    // 逐个改写插值表达式中的自由标识符。
    for child in &mut lowered {
        // 纯文本没有运行时捕获。
        let Node::Interpolation(interpolation) = child else {
            // 继续处理下一个内容节点。
            continue;
        };
        // 为当前插值建立拥有型捕获引用。
        rewrite_captures(&mut interpolation.expression, &mut captures, &mut locals);
    }
    // 使用共享逻辑保持文本拼接、顺序与诊断完全一致。
    let content = generate_text_content(&lowered, span)?;
    // 生成每个自由值的独立 Clone 所有权。
    let capture_statements = captures.iter().map(|(source, captured)| {
        // 恢复已经过表达式词法校验的原标识符。
        let source = Ident::new(source, Span::mixed_site());
        // 返回闭包外的拥有型克隆语句。
        quote! {
            // 防止动态文本闭包移走调用方或兄弟 View 仍需使用的值。
            let #captured = ::std::clone::Clone::clone(&(#source));
        }
    });
    // 返回公开 DynamicLabel 构造器与原公共 View 装饰兼容的表达式。
    Ok(quote! {{
        // 在闭包创建前取得全部自由值的独立所有权。
        #(#capture_statements)*
        // State::get 只会在该动态文本闭包执行时发生。
        move || ::std::convert::Into::<::std::string::String>::into(#content)
    }})
}

// 递归把插值中的自由标识符改写为卫生的拥有型捕获。
fn rewrite_captures(
    // 接收待改写表达式。
    expression: &mut Expression,
    // 接收跨全部插值共享的捕获表。
    captures: &mut BTreeMap<String, Ident>,
    // 接收当前受限闭包的词法局部栈。
    locals: &mut Vec<BTreeSet<String>>,
) {
    // 按表达式形状递归改写。
    match &mut expression.kind {
        // action 只允许事件位置，动态文本路径不会接收该内部形状。
        ExpressionKind::LoweredAction(_) => {}
        // 自由标识符取得独立 Clone 捕获。
        ExpressionKind::Identifier(name) => {
            // 保留事件专用标识符，让共享表达式生成器返回原有作用域诊断。
            if name == "$event" {
                // 非法事件参数不得在捕获标识符构造阶段触发 panic。
                return;
            }
            // 闭包参数由闭包自身拥有，不从外层克隆。
            if locals.iter().rev().any(|scope| scope.contains(name)) {
                // 保留原词法局部名称。
                return;
            }
            // 语言构造器由表达式生成器解析，不属于运行时自由值。
            if matches!(name.as_str(), "Some" | "None") || is_registered_data_type(name) {
                // 保留标准构造名称。
                return;
            }
            // 为首次出现的自由值分配稳定卫生名称。
            let next_index = captures.len();
            // 复用同一 Text 内相同自由值的单一捕获。
            let captured = captures
                // 按原名称查找或插入捕获。
                .entry(name.clone())
                // 只在首次出现时创建卫生标识符。
                .or_insert_with(|| {
                    // 使用混合卫生避免与调用方名称冲突。
                    Ident::new(
                        // 编号只取决于首次遍历顺序。
                        &format!("__uix_dynamic_text_capture_{next_index}"),
                        // 保持宏内部名称卫生。
                        Span::mixed_site(),
                    )
                })
                // 复制标识符供 AST 名称替换。
                .clone();
            // 表达式后续生成时引用闭包拥有的捕获；Fn 闭包可重复执行，
            // 按值使用处必须克隆捕获，不能把捕获移动出闭包。
            let span = expression.span;
            expression.kind = ExpressionKind::Call {
                // 构造捕获变量的公开 clone 成员调用。
                callee: Box::new(Expression {
                    kind: ExpressionKind::Member {
                        object: Box::new(Expression {
                            kind: ExpressionKind::Identifier(captured.to_string()),
                            span,
                        }),
                        member: "clone".to_string(),
                    },
                    span,
                }),
                // clone 不接收参数。
                arguments: Vec::new(),
            };
        }
        // 字面量没有自由标识符。
        ExpressionKind::Number(_) | ExpressionKind::String(_) | ExpressionKind::Boolean(_) => {}
        // 对象字段值按源码顺序递归。
        ExpressionKind::Object(fields) => {
            // 遍历全部字段值。
            for field in fields {
                // 改写当前字段表达式。
                rewrite_captures(&mut field.value, captures, locals);
            }
        }
        // 数组元素按源码顺序递归。
        ExpressionKind::Array(items) => {
            // 遍历全部数组元素。
            for item in items {
                // 改写当前元素。
                rewrite_captures(item, captures, locals);
            }
        }
        // 受限闭包参数只在闭包体内遮蔽同名自由值。
        ExpressionKind::Closure { parameter, body } => {
            // 压入当前唯一参数。
            locals.push(BTreeSet::from([parameter.clone()]));
            // 改写闭包体中的外部自由值。
            rewrite_captures(body, captures, locals);
            // 恢复外层词法作用域。
            locals.pop();
        }
        // 一元表达式只递归操作数。
        ExpressionKind::Unary { operand, .. } => {
            // 改写一元操作数。
            rewrite_captures(operand, captures, locals);
        }
        // 二元表达式递归两侧。
        ExpressionKind::Binary { left, right, .. } => {
            // 改写左侧。
            rewrite_captures(left, captures, locals);
            // 改写右侧。
            rewrite_captures(right, captures, locals);
        }
        // 三元表达式递归条件与两支。
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            // 改写条件。
            rewrite_captures(condition, captures, locals);
            // 改写真分支。
            rewrite_captures(then_branch, captures, locals);
            // 改写假分支。
            rewrite_captures(else_branch, captures, locals);
        }
        // 成员名不是自由值，只递归所属对象。
        ExpressionKind::Member { object, .. } => {
            // 改写成员所属对象。
            rewrite_captures(object, captures, locals);
        }
        // 下标访问递归对象与索引。
        ExpressionKind::Index { object, index } => {
            // 改写被索引对象。
            rewrite_captures(object, captures, locals);
            // 改写索引表达式。
            rewrite_captures(index, captures, locals);
        }
        // 调用递归目标与全部参数。
        ExpressionKind::Call { callee, arguments } => {
            // 改写调用目标中的自由值。
            rewrite_captures(callee, captures, locals);
            // 按源码顺序改写参数。
            for argument in arguments {
                // 改写当前参数值。
                rewrite_captures(&mut argument.value, captures, locals);
            }
        }
    }
}

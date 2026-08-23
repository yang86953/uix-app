// 引入线程局部的生成文件标记上下文与确定映射。
use std::{cell::RefCell, collections::BTreeMap};

// 引入过程宏令牌与标识符类型。
use proc_macro2::{Ident, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入 Compiler System 的稳定源码身份。
use crate::source_graph::SourceId;
// 引入独立数据构造器生成入口。
use super::data_constructor_codegen::generate_data_constructor;
// 引入表达式语法树、运算符与结构化诊断。
use super::{
    BinaryOperator, CallArgument, Diagnostic, Expression, ExpressionKind, SourceSpan,
    UnaryOperator, data_chain_root, step_status_path,
};

// 保存当前线程是否正在为生成文件输出来源标记。
thread_local! {
    // 纯 codegen 默认不携带仅供写盘消费的内部令牌。
    static SOURCE_MARKERS: RefCell<SourceMarkerContext> = RefCell::new(SourceMarkerContext::default());
}

#[derive(Default)]
struct SourceMarkerContext {
    enabled: bool,
    current_source: Option<SourceId>,
    widget_sources: BTreeMap<String, SourceId>,
    record_sources: BTreeMap<String, SourceId>,
}

// 在一次宏入口代码生成期间启用来源标记。
pub(crate) fn with_source_markers<T>(
    root_source: SourceId,
    widget_sources: BTreeMap<String, SourceId>,
    record_sources: BTreeMap<String, SourceId>,
    operation: impl FnOnce() -> Result<T, Diagnostic>,
) -> Result<T, Diagnostic> {
    // 在当前过程宏线程内切换标记状态。
    SOURCE_MARKERS.with(|context| {
        // 保存嵌套调用前的完整上下文。
        let previous = context.replace(SourceMarkerContext {
            enabled: true,
            current_source: Some(root_source),
            widget_sources,
            record_sources,
        });
        // 执行完整文档代码生成。
        let result = operation();
        // 恢复调用前状态，避免污染后续纯 codegen。
        context.replace(previous);
        // 返回原始生成结果。
        result.map_err(|diagnostic| diagnostic.at_source_if_missing(Some(root_source)))
    })
}

// 在元素或组件模板生成期间临时切换到精确源码身份。
pub(crate) fn with_source_marker_id<T>(
    source_id: Option<SourceId>,
    operation: impl FnOnce() -> T,
) -> T {
    SOURCE_MARKERS.with(|context| {
        let previous = {
            let mut context = context.borrow_mut();
            let previous = context.current_source;
            if context.enabled && source_id.is_some() {
                context.current_source = source_id;
            }
            previous
        };
        let result = operation();
        context.borrow_mut().current_source = previous;
        result
    })
}

// 按类型化声明登记切换到当前自定义组件模板来源。
pub(crate) fn with_widget_source_marker<T>(
    name: &str,
    operation: impl FnOnce() -> Result<T, Diagnostic>,
) -> Result<T, Diagnostic> {
    let source_id = SOURCE_MARKERS.with(|context| {
        let context = context.borrow();
        context.widget_sources.get(name).copied()
    });
    with_source_marker_id(source_id, operation)
        .map_err(|diagnostic| diagnostic.at_source_if_missing(source_id))
}

// 按类型化声明登记切换到当前 Record 的真实来源。
pub(crate) fn with_record_source_marker<T>(
    name: &str,
    operation: impl FnOnce() -> Result<T, Diagnostic>,
) -> Result<T, Diagnostic> {
    let source_id = SOURCE_MARKERS.with(|context| {
        let context = context.borrow();
        context.record_sources.get(name).copied()
    });
    with_source_marker_id(source_id, operation)
        .map_err(|diagnostic| diagnostic.at_source_if_missing(source_id))
}

// 把已验证表达式转换为 Rust 表达式令牌。
pub(crate) fn generate_expression(
    // 接收确定性表达式语法树。
    expression: &Expression,
    // 接收可选的事件载荷局部变量。
    event: Option<&Ident>,
) -> Result<TokenStream, Diagnostic> {
    // 先生成不含定位包装的完整表达式。
    let generated = generate_expression_inner(expression, event)?;
    // 仅在生成文件入口为最外层加入源码位置标记。
    Ok(mark_source_tokens(generated, expression.span))
}

// 生成不携带文件来源标记的规范表达式令牌。
pub(crate) fn generate_expression_without_source_marker(
    // 接收确定性表达式语法树。
    expression: &Expression,
    // 接收可选的事件载荷局部变量。
    event: Option<&Ident>,
) -> Result<TokenStream, Diagnostic> {
    // 直接调用内部递归生成器。
    generate_expression_inner(expression, event)
}

// 递归生成表达式内部令牌而不重复加入来源标记。
pub(super) fn generate_expression_inner(
    // 接收确定性表达式语法树。
    expression: &Expression,
    // 接收可选的事件载荷局部变量。
    event: Option<&Ident>,
) -> Result<TokenStream, Diagnostic> {
    // 按表达式结构生成等价 Rust 代码。
    match &expression.kind {
        // action 独立语句 AST 在事件位置生成为局部 Rust 标签块。
        ExpressionKind::LoweredAction(action) => {
            super::action_codegen::generate_lowered_action(action)
        }
        // 普通标识符直接映射为 Rust 标识符。
        ExpressionKind::Identifier(name) => generate_identifier(name, expression.span, event),
        // 数字保持经过验证的源码表示。
        ExpressionKind::Number(source) => generate_number(source, expression.span),
        // 单引号字符串转换为 Rust 字符串字面量。
        ExpressionKind::String(value) => Ok(quote! { #value }),
        // 布尔值转换为 Rust 布尔字面量。
        ExpressionKind::Boolean(value) => Ok(quote! { #value }),
        // 对象字面量必须由声明了字段契约的结构属性生成器消费。
        ExpressionKind::Object(_) => Err(Diagnostic::new(
            // 指向完整对象。
            expression.span,
            // 陈述普通生成路径没有目标类型。
            "对象字面量只能用于已登记的结构属性",
            // 给出可审计的使用边界。
            "把对象用于文档明确声明的结构属性，或改用普通绑定表达式",
        )),
        // 数组字面量生成 Rust vec 字面量，由属性消费方按值 clone。
        ExpressionKind::Array(items) => {
            // 生成全部元素表达式。
            let items = items
                // 按源码顺序生成。
                .iter()
                // 递归转换每个元素。
                .map(|item| generate_expression_inner(item, event))
                // 收集或返回首个诊断。
                .collect::<Result<Vec<_>, _>>()?;
            // 返回可推断元素类型的 vec 字面量。
            Ok(quote! { ::std::vec![#(#items),*] })
        }
        // 闭包必须由已登记数组操作消费，不能独立生成。
        ExpressionKind::Closure { .. } => Err(Diagnostic::new(
            // 指向完整闭包。
            expression.span,
            // 说明生成边界。
            "受限闭包只能用于数组操作方法参数",
            // 列出开放闭包的数组操作。
            "把闭包用于 removeBy、filter、map、sortBy 或 find",
        )),
        // 一元表达式递归生成操作数。
        ExpressionKind::Unary { operator, operand } => {
            // 生成一元操作数。
            let operand = generate_expression_inner(operand, event)?;
            // 按运算符拼接 Rust 一元表达式。
            Ok(match operator {
                // 逻辑非保持 Rust 语义。
                UnaryOperator::Not => quote! { !(#operand) },
                // 数值取负保持 Rust 语义。
                UnaryOperator::Negate => quote! { -(#operand) },
            })
        }
        // 二元表达式递归生成两侧操作数。
        ExpressionKind::Binary {
            left,
            operator,
            right,
        } => {
            // 生成左操作数。
            let left = generate_expression_inner(left, event)?;
            // 生成右操作数。
            let right = generate_expression_inner(right, event)?;
            // 按运算符拼接 Rust 二元表达式。
            Ok(generate_binary(&left, *operator, &right))
        }
        // 三元表达式翻译为 Rust if/else 表达式。
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            // 生成条件表达式。
            let condition = generate_expression_inner(condition, event)?;
            // 生成真分支表达式。
            let then_branch = generate_expression_inner(then_branch, event)?;
            // 生成假分支表达式。
            let else_branch = generate_expression_inner(else_branch, event)?;
            // 返回类型由 Rust 编译器统一检查的条件表达式。
            Ok(quote! { if #condition { #then_branch } else { #else_branch } })
        }
        // 成员访问处理 length 与事件坐标的语义映射。
        ExpressionKind::Member { object, member } => {
            // 委托成员生成器处理特殊成员。
            generate_member(object, member, expression.span, event)
        }
        // 下标访问保持 Rust 索引语义。
        ExpressionKind::Index { object, index } => {
            // 生成被索引对象。
            let object = generate_expression_inner(object, event)?;
            // 生成索引表达式。
            let index = generate_expression_inner(index, event)?;
            // 返回带括号的 Rust 索引表达式。
            Ok(quote! { (#object)[#index] })
        }
        // 调用表达式处理普通调用和不可变数组操作。
        ExpressionKind::Call { callee, arguments } => {
            // 委托调用生成器验证当前 Gate 的语义边界。
            generate_call(callee, arguments, expression.span, event)
        }
    }
}

// 用稳定内部令牌标记一段用户表达式的 UIX 源位置。
fn mark_source_expression(generated: TokenStream, span: SourceSpan) -> TokenStream {
    // 把一基行列编码为不会与用户标识符冲突的卫生名称。
    let source_id = SOURCE_MARKERS.with(|context| context.borrow().current_source);
    let marker_name = source_id.map_or_else(
        || format!("__uix_source_marker_{}_{}", span.line, span.column),
        |source_id| {
            format!(
                "__uix_source_marker_{:016x}_{}_{}",
                source_id.value(),
                span.line,
                span.column
            )
        },
    );
    let marker = Ident::new(
        // 生成文件写入器会把该令牌替换为真实映射注释。
        &marker_name,
        // 使用混合卫生避免参与调用方名称解析。
        proc_macro2::Span::mixed_site(),
    );
    // 把可移除标识符放在原表达式前，不增加会改变 place 语义的块。
    quote! {
        // 该令牌只存在于过程宏内部，写盘前会变成注释。
        #marker
        // 紧随完全不变的原始表达式令牌。
        #generated
    }
}

// 根据当前入口上下文选择原始或带来源标记的表达式。
pub(crate) fn mark_source_tokens(generated: TokenStream, span: SourceSpan) -> TokenStream {
    // 查询当前线程的生成文件模式。
    SOURCE_MARKERS.with(|context| {
        // 生成文件模式需要写入可替换标记。
        if context.borrow().enabled {
            // 返回带来源标记的表达式。
            mark_source_expression(generated, span)
        } else {
            // 纯 codegen 保持原有令牌形状。
            generated
        }
    })
}

// 生成事件处理器主体，并兼容无括号处理器名。
pub(crate) fn generate_handler_expression(
    // 接收事件属性中的表达式。
    expression: &Expression,
    // 接收可选事件载荷变量。
    event: Option<&Ident>,
) -> Result<TokenStream, Diagnostic> {
    // 裸标识符按文档约定视为零参数处理器调用。
    if let ExpressionKind::Identifier(name) = &expression.kind {
        // 保留事件保留参数自身的普通表达式含义。
        if name != "$event" {
            // 生成可调用标识符。
            let handler = generate_identifier(name, expression.span, event)?;
            // 返回带 UIX 源位置标记的零参数调用。
            return Ok(mark_source_tokens(
                // 生成原始零参数调用。
                quote! { (#handler)() },
                // 沿用事件表达式跨度。
                expression.span,
            ));
        }
    }
    // 其他结构按普通受限表达式生成。
    generate_expression(expression, event)
}

// 判断表达式是否读取事件保留参数。
pub(crate) fn expression_uses_event(expression: &Expression) -> bool {
    // 递归检查全部表达式结构。
    match &expression.kind {
        // action 不允许捕获事件，防御性检查块内全部表达式。
        ExpressionKind::LoweredAction(action) => {
            let mut uses_event = false;
            let _ = super::action_semantic::visit_action_block_expressions(
                &action.block,
                &mut |expression| {
                    uses_event |= expression_uses_event(expression);
                    Ok(())
                },
            );
            uses_event
        }
        // 只有事件保留标识符直接命中。
        ExpressionKind::Identifier(name) => name == "$event",
        // 一元表达式递归检查操作数。
        ExpressionKind::Unary { operand, .. } => expression_uses_event(operand),
        // 二元表达式递归检查两侧。
        ExpressionKind::Binary { left, right, .. } => {
            // 任一侧命中即需要事件载荷。
            expression_uses_event(left) || expression_uses_event(right)
        }
        // 三元表达式递归检查条件与两分支。
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            // 任一子表达式命中即需要事件载荷。
            expression_uses_event(condition)
                || expression_uses_event(then_branch)
                || expression_uses_event(else_branch)
        }
        // 成员访问递归检查对象。
        ExpressionKind::Member { object, .. } => expression_uses_event(object),
        // 索引访问递归检查对象与索引。
        ExpressionKind::Index { object, index } => {
            // 任一子表达式命中即需要事件载荷。
            expression_uses_event(object) || expression_uses_event(index)
        }
        // 调用递归检查目标与全部参数。
        ExpressionKind::Call { callee, arguments } => {
            // 检查调用目标或任一参数。
            expression_uses_event(callee)
                || arguments
                    // 遍历参数列表。
                    .iter()
                    // 检查每个参数值。
                    .any(|argument| expression_uses_event(&argument.value))
        }
        // 对象递归检查全部字段值。
        ExpressionKind::Object(fields) => fields
            // 遍历有序字段。
            .iter()
            // 任一字段值读取事件即命中。
            .any(|field| expression_uses_event(&field.value)),
        // 数组递归检查全部元素。
        ExpressionKind::Array(items) => items
            // 遍历有序元素。
            .iter()
            // 任一元素读取事件即命中。
            .any(expression_uses_event),
        // 受限闭包递归检查唯一表达式体。
        ExpressionKind::Closure { body, .. } => expression_uses_event(body),
        // 其余字面量不读取事件。
        ExpressionKind::Number(_) | ExpressionKind::String(_) | ExpressionKind::Boolean(_) => {
            // 返回未命中。
            false
        }
    }
}

// 生成普通或保留标识符。
fn generate_identifier(
    // 接收语言标识符文本。
    name: &str,
    // 接收诊断跨度。
    span: SourceSpan,
    // 接收可选事件载荷变量。
    event: Option<&Ident>,
) -> Result<TokenStream, Diagnostic> {
    // 事件保留标识符必须位于事件闭包内。
    if name == "$event" {
        // 返回已经绑定的事件载荷变量。
        return event.map(|event| quote! { #event }).ok_or_else(|| {
            // 构造越界使用诊断。
            Diagnostic::new(
                // 指向保留标识符。
                span,
                // 说明事件参数作用域。
                "$event 只能在事件处理器中使用",
                // 给出修复动作。
                "把 $event 移入 @click 处理器或改用普通 Rust 变量",
            )
        });
    }
    // 使用 syn 验证标识符也能作为 Rust 名称。
    let identifier = syn::parse_str::<Ident>(name).map_err(|_| {
        // 构造 Rust 名称不兼容诊断。
        Diagnostic::new(
            // 指向语言标识符。
            span,
            // 说明映射失败原因。
            format!("标识符 {name} 不是合法 Rust 标识符"),
            // 给出改名建议。
            "改用非 Rust 关键字的 ASCII 标识符",
        )
    })?;
    // 返回标识符令牌。
    Ok(quote! { #identifier })
}

// 生成保持源码精度的数字令牌。
fn generate_number(
    // 接收已经通过词法验证的数字。
    source: &str,
    // 接收诊断跨度。
    span: SourceSpan,
) -> Result<TokenStream, Diagnostic> {
    // 解析为 Rust 令牌流以保留整数或小数形状。
    source.parse::<TokenStream>().map_err(|_| {
        // 构造数字映射诊断。
        Diagnostic::new(
            // 指向数字字面量。
            span,
            // 说明 Rust 令牌映射失败。
            format!("数字 {source} 无法转换为 Rust 字面量"),
            // 给出规范数字格式。
            "使用十进制整数或小数",
        )
    })
}

// 生成二元运算表达式。
fn generate_binary(
    // 接收左侧令牌。
    left: &TokenStream,
    // 接收语言二元运算符。
    operator: BinaryOperator,
    // 接收右侧令牌。
    right: &TokenStream,
) -> TokenStream {
    // 对每个允许运算符生成明确 Rust 结构。
    match operator {
        // 生成加法。
        BinaryOperator::Add => quote! { (#left) + (#right) },
        // 生成减法。
        BinaryOperator::Subtract => quote! { (#left) - (#right) },
        // 生成乘法。
        BinaryOperator::Multiply => quote! { (#left) * (#right) },
        // 生成除法。
        BinaryOperator::Divide => quote! { (#left) / (#right) },
        // 生成取余。
        BinaryOperator::Remainder => quote! { (#left) % (#right) },
        // 生成相等比较。
        BinaryOperator::Equal => quote! { (#left) == (#right) },
        // 生成不等比较。
        BinaryOperator::NotEqual => quote! { (#left) != (#right) },
        // 生成小于比较。
        BinaryOperator::Less => quote! { (#left) < (#right) },
        // 生成小于等于比较。
        BinaryOperator::LessEqual => quote! { (#left) <= (#right) },
        // 生成大于比较。
        BinaryOperator::Greater => quote! { (#left) > (#right) },
        // 生成大于等于比较。
        BinaryOperator::GreaterEqual => quote! { (#left) >= (#right) },
        // 生成短路逻辑与。
        BinaryOperator::And => quote! { (#left) && (#right) },
        // 生成短路逻辑或。
        BinaryOperator::Or => quote! { (#left) || (#right) },
    }
}

// 生成普通成员、length 或事件坐标访问。
fn generate_member(
    // 接收成员所属对象。
    object: &Expression,
    // 接收成员名称。
    member: &str,
    // 接收成员表达式跨度。
    span: SourceSpan,
    // 接收可选事件载荷变量。
    event: Option<&Ident>,
) -> Result<TokenStream, Diagnostic> {
    // 识别 $event 的坐标简写。
    if matches!(&object.kind, ExpressionKind::Identifier(name) if name == "$event") {
        // 要求事件闭包已经绑定点击载荷。
        let event = event.ok_or_else(|| {
            // 构造越界事件访问诊断。
            Diagnostic::new(
                // 指向成员表达式。
                span,
                // 说明保留参数作用域。
                "$event 成员只能在事件处理器中使用",
                // 给出修复动作。
                "把事件成员访问移入 @click 处理器",
            )
        })?;
        // 按登记表支持的字段名称投影实际载荷。
        return match member {
            // 生成 x 坐标读取。
            "x" => Ok(quote! { (#event).pos.x }),
            // 生成 y 坐标读取。
            "y" => Ok(quote! { (#event).pos.y }),
            // change、select、submit 与 close 的 value 就是现有实际载荷。
            "value" => Ok(quote! { (#event) }),
            // 键盘 key 保留公开 KeyCode 值。
            "key" => Ok(quote! { (#event) }),
            // 运行时没有第二套物理码，code 使用稳定 KeyCode 调试文本。
            "code" => Ok(quote! { ::std::format!("{:?}", (#event)) }),
            // 其他成员按 ClickEvent 的公开字段映射。
            _ => {
                // 验证成员名可映射为 Rust 标识符。
                let member = rust_member(member, span)?;
                // 返回公开字段访问。
                Ok(quote! { (#event).#member })
            }
        };
    }
    // 生成普通对象表达式。
    let object = generate_expression_inner(object, event)?;
    // length 按语言契约映射为 Rust len 调用。
    if member == "length" {
        // 返回集合长度。
        return Ok(quote! { (#object).len() });
    }
    // 验证普通成员名。
    let member = rust_member(member, span)?;
    // 返回 Rust 字段访问。
    Ok(quote! { (#object).#member })
}

// 生成普通调用或不可变数组操作。
fn generate_call(
    // 接收调用目标。
    callee: &Expression,
    // 接收有序调用参数。
    arguments: &[CallArgument],
    // 接收完整调用跨度。
    span: SourceSpan,
    // 接收可选事件载荷变量。
    event: Option<&Ident>,
) -> Result<TokenStream, Diagnostic> {
    // setState 由下一 Widget Gate 统一生成。
    if matches!(&callee.kind, ExpressionKind::Identifier(name) if name == "setState") {
        // 返回明确的阶段边界诊断。
        return Err(Diagnostic::new(
            // 指向完整调用。
            span,
            // 说明当前尚无状态上下文。
            "setState 需要 Widget state 代码生成上下文",
            // 指向后续合法位置。
            "在 Widget 内使用 setState，并由组件代码生成阶段处理",
        ));
    }
    // setStyle 必须先由组件动态样式阶段降低为闭合 setter。
    if matches!(&callee.kind, ExpressionKind::Identifier(name) if name == "setStyle") {
        // 返回明确组件边界诊断，禁止回退到调用方同名函数。
        return Err(Diagnostic::new(
            span,
            "setStyle 需要 Widget 动态样式代码生成上下文",
            "在 Widget 当前 View 的事件中使用 setStyle('className')",
        ));
    }
    // setTheme 是框架内置操作：生成主题请求通道调用，不依赖调用方同名函数。
    if matches!(&callee.kind, ExpressionKind::Identifier(name) if name == "setTheme") {
        // 参数形状已在解析期验证为单个字符串位置参数。
        let argument = single_positional_argument(arguments, "setTheme", span)?;
        // 生成主题名称参数。
        let name = generate_expression_inner(&argument.value, event)?;
        // 生成框架主题切换入口调用。
        return Ok(quote! { ::uix::ui::__private::uix_set_theme(&(#name)) });
    }
    // 识别不可变数组操作。
    if let ExpressionKind::Member { object, member } = &callee.kind {
        // 已登记数组操作由唯一不可变更新生成器消费。
        if let Some(generated) = generate_array_operation(object, member, arguments, span, event) {
            // 返回数组操作生成结果或对应诊断。
            return generated;
        }
        // Step.status 的字符串语义值映射为公开枚举路径。
        if member == "status" && data_chain_root(object) == Some("Step") {
            // 生成对象表达式。
            let object_tokens = generate_expression_inner(object, event)?;
            // 状态语义值必须是单个字符串字面量。
            let argument = single_positional_argument(arguments, "status", span)?;
            // 提取语言面字符串值。
            let value = match &argument.value.kind {
                // 只有字面量可以映射枚举。
                ExpressionKind::String(value) => value.clone(),
                // 其他形状返回专用诊断。
                _ => {
                    // 返回状态值形状诊断。
                    return Err(Diagnostic::new(
                        // 指向状态参数。
                        argument.span,
                        // 说明映射需求。
                        "Step.status 只接受字符串语义值",
                        // 给出合法值集合。
                        "使用 'finish'、'process' 或 'wait'",
                    ));
                }
            };
            // 查找语义值对应的枚举路径。
            let status = step_status_path(&value).ok_or_else(|| {
                // 构造未知状态诊断。
                Diagnostic::new(
                    // 指向状态参数。
                    argument.span,
                    // 说明未知语义值。
                    format!("Step.status={value:?} 不在登记表"),
                    // 给出合法值集合。
                    "使用 'finish'、'process' 或 'wait'",
                )
            })?;
            // 生成枚举路径调用。
            return Ok(quote! { (#object_tokens).status(#status) });
        }
    }
    // 语言面数据类型由独立生成器映射公开构造 API。
    if let ExpressionKind::Identifier(name) = &callee.kind {
        // 已登记构造器直接返回生成结果。
        if let Some(generated) = generate_data_constructor(name, arguments, span, event) {
            // 传播构造结果或诊断。
            return generated;
        }
    }
    // 普通调用不允许命名参数。
    if arguments.iter().any(|argument| argument.name.is_some()) {
        // 返回命名参数边界诊断。
        return Err(Diagnostic::new(
            // 指向完整调用。
            span,
            // 说明命名参数限制。
            "普通 Rust 调用不支持命名参数",
            // 给出位置参数修复建议。
            "删除参数名并按 Rust 函数签名顺序传参",
        ));
    }
    // 成员调用直接生成方法调用形式，避免与方法同名字段产生解析歧义。
    if let ExpressionKind::Member { object, member } = &callee.kind {
        // 生成成员所属对象。
        let object = generate_expression_inner(object, event)?;
        // 验证成员名可映射为 Rust 方法名。
        let member = rust_member(member, span)?;
        // 生成全部位置参数。
        let arguments = arguments
            // 遍历有序参数。
            .iter()
            // 转换每个参数表达式。
            .map(|argument| generate_expression_inner(&argument.value, event))
            // 收集或返回首个诊断。
            .collect::<Result<Vec<_>, _>>()?;
        // 返回方法调用表达式。
        return Ok(quote! { (#object).#member(#(#arguments),*) });
    }
    // 生成普通调用目标。
    let callee = generate_expression_inner(callee, event)?;
    // 生成全部位置参数。
    let arguments = arguments
        // 遍历有序参数。
        .iter()
        // 转换每个参数表达式。
        .map(|argument| generate_expression_inner(&argument.value, event))
        // 收集或返回首个诊断。
        .collect::<Result<Vec<_>, _>>()?;
    // 返回 Rust 调用表达式。
    Ok(quote! { (#callee)(#(#arguments),*) })
}

// 生成一个已登记的不可变数组操作，普通成员调用返回 None。
fn generate_array_operation(
    // 接收数组对象表达式。
    object: &Expression,
    // 接收语言面操作名称。
    member: &str,
    // 接收操作参数。
    arguments: &[CallArgument],
    // 接收完整调用跨度。
    span: SourceSpan,
    // 接收可选事件载荷变量。
    event: Option<&Ident>,
) -> Option<Result<TokenStream, Diagnostic>> {
    // 只接管规范登记的数组操作名称。
    if !matches!(
        member,
        "push"
            | "removeAt"
            | "insertAt"
            | "updateAt"
            | "removeBy"
            | "filter"
            | "map"
            | "sortBy"
            | "find"
    ) {
        // 普通 Rust 成员调用留给后续路径。
        return None;
    }
    // 在内部闭包中生成完整数组操作。
    Some((|| {
        // 生成只求值一次的数组对象。
        let object = generate_expression_inner(object, event)?;
        // 为不可变更新局部数组选择卫生名称。
        let array = Ident::new("__uix_array_value", proc_macro2::Span::mixed_site());
        // 按操作名称生成确定性 Rust 语义。
        match member {
            // push 克隆后追加并返回新数组。
            "push" => {
                // 读取唯一追加值。
                let argument = single_positional_argument(arguments, member, span)?;
                // 生成追加值表达式。
                let value = generate_expression_inner(&argument.value, event)?;
                // 返回不可变追加块。
                Ok(quote! {{
                    // 克隆原数组以保持语言的不可变更新语义。
                    let mut #array = (#object).clone();
                    // 向新数组追加元素。
                    #array.push(#value);
                    // 返回新数组。
                    #array
                }})
            }
            // removeAt 克隆后按下标删除并返回新数组。
            "removeAt" => {
                // 读取唯一删除下标。
                let argument = single_positional_argument(arguments, member, span)?;
                // 生成下标表达式。
                let index = generate_expression_inner(&argument.value, event)?;
                // 返回不可变删除块。
                Ok(quote! {{
                    // 克隆原数组以保持语言的不可变更新语义。
                    let mut #array = (#object).clone();
                    // 从新数组移除指定位置。
                    #array.remove(#index);
                    // 返回新数组。
                    #array
                }})
            }
            // insertAt 克隆后在指定下标插入并返回新数组。
            "insertAt" => {
                // 读取索引和值两个位置参数。
                let (index, value) = two_positional_arguments(arguments, member, span)?;
                // 生成插入下标。
                let index = generate_expression_inner(&index.value, event)?;
                // 生成插入值。
                let value = generate_expression_inner(&value.value, event)?;
                // 返回不可变插入块。
                Ok(quote! {{
                    // 克隆原数组以保持调用方绑定不被移动。
                    let mut #array = (#object).clone();
                    // 在新数组指定位置插入元素。
                    #array.insert(#index, #value);
                    // 返回新数组。
                    #array
                }})
            }
            // updateAt 克隆后替换指定下标并返回新数组。
            "updateAt" => {
                // 读取索引和值两个位置参数。
                let (index, value) = two_positional_arguments(arguments, member, span)?;
                // 生成替换下标。
                let index = generate_expression_inner(&index.value, event)?;
                // 生成替换值。
                let value = generate_expression_inner(&value.value, event)?;
                // 返回不可变替换块。
                Ok(quote! {{
                    // 克隆原数组以保持调用方绑定不被移动。
                    let mut #array = (#object).clone();
                    // 替换新数组指定位置的元素。
                    #array[#index] = #value;
                    // 返回新数组。
                    #array
                }})
            }
            // removeBy 克隆后删除首个谓词匹配项。
            "removeBy" => {
                // 生成唯一受限闭包参数与表达式体。
                let (parameter, body) = generate_array_closure(arguments, member, span, event)?;
                // 为首个匹配位置选择卫生名称。
                let index = Ident::new("__uix_match_index", proc_macro2::Span::mixed_site());
                // 返回首匹配不可变删除块。
                Ok(quote! {{
                    // 克隆原数组以保持调用方绑定不被移动。
                    let mut #array = (#object).clone();
                    // 查找首个满足谓词的元素位置。
                    if let ::std::option::Option::Some(#index) = #array.iter().position(|#parameter| #body) {
                        // 只删除首个匹配元素。
                        #array.remove(#index);
                    }
                    // 返回新数组。
                    #array
                }})
            }
            // filter 对克隆数组按值迭代并收集匹配元素。
            "filter" => {
                // 生成唯一受限闭包参数与表达式体。
                let (parameter, body) = generate_array_closure(arguments, member, span, event)?;
                // 返回拥有型过滤结果。
                Ok(quote! {
                    (#object).clone().into_iter().filter(|#parameter| #body).collect::<::std::vec::Vec<_>>()
                })
            }
            // map 对克隆数组按值迭代并收集映射元素。
            "map" => {
                // 生成唯一受限闭包参数与表达式体。
                let (parameter, body) = generate_array_closure(arguments, member, span, event)?;
                // 返回拥有型映射结果。
                Ok(quote! {
                    (#object).clone().into_iter().map(|#parameter| #body).collect::<::std::vec::Vec<_>>()
                })
            }
            // sortBy 克隆后按闭包键执行稳定升序排序。
            "sortBy" => {
                // 生成唯一受限闭包参数与表达式体。
                let (parameter, body) = generate_array_closure(arguments, member, span, event)?;
                // 返回稳定排序后的新数组。
                Ok(quote! {{
                    // 克隆原数组以保持调用方绑定不被移动。
                    let mut #array = (#object).clone();
                    // 使用标准库稳定键排序。
                    #array.sort_by_key(|#parameter| #body);
                    // 返回新数组。
                    #array
                }})
            }
            // find 对克隆数组按值迭代并返回首个拥有型匹配项。
            "find" => {
                // 生成唯一受限闭包参数与表达式体。
                let (parameter, body) = generate_array_closure(arguments, member, span, event)?;
                // 返回首个匹配项或 None。
                Ok(quote! { (#object).clone().into_iter().find(|#parameter| #body) })
            }
            // 操作名称已由入口闭合集合保证。
            _ => unreachable!(),
        }
    })())
}

// 生成数组操作唯一受限闭包的参数与表达式体。
fn generate_array_closure(
    // 接收调用参数。
    arguments: &[CallArgument],
    // 接收操作名称。
    operation: &str,
    // 接收完整调用跨度。
    span: SourceSpan,
    // 接收可选事件载荷变量。
    event: Option<&Ident>,
) -> Result<(Ident, TokenStream), Diagnostic> {
    // 先验证唯一位置参数。
    let argument = single_positional_argument(arguments, operation, span)?;
    // 参数必须保持闭包 AST 形状。
    let ExpressionKind::Closure { parameter, body } = &argument.value.kind else {
        // 返回防御性闭包形状诊断。
        return Err(Diagnostic::new(
            // 指向实际参数。
            argument.span,
            // 说明操作参数要求。
            format!("{operation} 必须接收一个受限闭包"),
            // 给出规范示例。
            format!("使用 array.{operation}(|item| expression)"),
        ));
    };
    // 把闭包参数验证并转换为 Rust 标识符。
    let parameter = rust_member(parameter, argument.value.span)?;
    // 生成闭包唯一表达式体。
    let body = generate_expression_inner(body, event)?;
    // 返回闭包两部分供具体迭代器操作拼接。
    Ok((parameter, body))
}

// 验证并返回数组双参数更新操作的位置参数。
fn two_positional_arguments<'a>(
    // 接收调用参数。
    arguments: &'a [CallArgument],
    // 接收操作名称。
    operation: &str,
    // 接收完整调用跨度。
    span: SourceSpan,
) -> Result<(&'a CallArgument, &'a CallArgument), Diagnostic> {
    // 要求恰好两个无名称参数。
    if arguments.len() == 2 && arguments.iter().all(|argument| argument.name.is_none()) {
        // 返回索引和值参数。
        return Ok((&arguments[0], &arguments[1]));
    }
    // 返回双参数数组操作诊断。
    Err(Diagnostic::new(
        // 指向完整调用。
        span,
        // 说明索引和值要求。
        format!("{operation} 必须接收索引和值两个位置参数"),
        // 给出对应规范示例。
        format!("使用 array.{operation}(index, value)"),
    ))
}

// 验证数组操作只有一个位置参数。
fn single_positional_argument<'a>(
    // 接收调用参数。
    arguments: &'a [CallArgument],
    // 接收操作名称。
    operation: &str,
    // 接收完整调用跨度。
    span: SourceSpan,
) -> Result<&'a CallArgument, Diagnostic> {
    // 要求恰好一个无名称参数。
    if arguments.len() == 1 && arguments[0].name.is_none() {
        // 返回唯一参数。
        return Ok(&arguments[0]);
    }
    // 返回数组操作参数诊断。
    Err(Diagnostic::new(
        // 指向完整调用。
        span,
        // 说明参数数量要求。
        format!("{operation} 必须接收一个位置参数"),
        // 给出对应规范示例。
        format!("使用 array.{operation}(value)"),
    ))
}

// 把语言成员名验证并转换为 Rust 标识符。
fn rust_member(
    // 接收成员文本。
    member: &str,
    // 接收诊断跨度。
    span: SourceSpan,
) -> Result<Ident, Diagnostic> {
    // 使用 syn 校验 Rust 标识符规则。
    syn::parse_str::<Ident>(member).map_err(|_| {
        // 构造非法成员诊断。
        Diagnostic::new(
            // 指向成员表达式。
            span,
            // 说明成员映射失败。
            format!("成员 {member} 不是合法 Rust 字段名"),
            // 给出改名建议。
            "改用非 Rust 关键字的 ASCII 成员名",
        )
    })
}

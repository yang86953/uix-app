// 引入过程宏卫生标识符与令牌流。
use proc_macro2::{Ident, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入事件属性 AST、诊断与处理器生成入口。
use super::{
    Attribute, AttributeValue, Diagnostic, expression_uses_event,
    generate_event_handler_expression, generate_handler_expression,
    generate_key_event_handler_expression,
};

// 生成当前核心 Gate 支持的通用事件。
pub(super) fn apply_event(
    // 接收已经生成的 View。
    view: TokenStream,
    // 接收事件属性。
    attribute: &Attribute,
) -> Result<TokenStream, Diagnostic> {
    // 键盘按下与抬起复用公开 on_key 入口并继续向组件传播。
    if matches!(attribute.name.as_str(), "@keyDown" | "@keyUp") {
        // 事件解析器保证事件值是表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回内部形状保护诊断。
            return Err(Diagnostic::new(
                // 指向完整属性。
                attribute.span,
                // 陈述处理器形状要求。
                "键盘事件处理器必须是受限表达式",
                // 给出合法示例。
                "使用 @keyDown=\"handler($event.key)\"",
            ));
        };
        // 创建卫生的系统事件变量。
        let system_event = Ident::new("__uix_key_system_event", Span::mixed_site());
        // 创建卫生的 KeyCode 值载荷变量。
        let key_payload = Ident::new("__uix_key_payload", Span::mixed_site());
        // 创建卫生的修饰键载荷变量，供 $event.mods 引用。
        let mods_payload = Ident::new("__uix_mods_payload", Span::mixed_site());
        // 按当前事件登记表校验 key/code/mods 后生成处理器。
        let handler = generate_key_event_handler_expression(
            // 传递处理器表达式。
            &expression.expression,
            // 传递实际 KeyCode 载荷。
            &key_payload,
            // 传递当前键盘事件名。
            &attribute.name,
            // 传递修饰键载荷，处理器可经 $event.mods 引用。
            Some(&mods_payload),
        )?;
        // 选择对应系统事件变体。
        let event_variant = if attribute.name == "@keyDown" {
            // 按键按下变体。
            quote! { ::uix::prelude::SystemEvent::KeyDown }
        } else {
            // 按键抬起变体。
            quote! { ::uix::prelude::SystemEvent::KeyUp }
        };
        // 返回不截断组件自身键盘处理的观察器。
        return Ok(quote! {
            // 复用公开键盘事件注册入口。
            (#view).on_key(move |#system_event| {
                // 只在声明的键盘事件变体执行语言处理器。
                if let #event_variant { key: __uix_key, mods: __uix_mods } = #system_event {
                    // 复制公开 KeyCode 作为稳定载荷。
                    let #key_payload = *__uix_key;
                    // 复制公开 KeyMod 作为稳定修饰键载荷。
                    let #mods_payload = *__uix_mods;
                    // 丢弃处理器返回值并保留副作用。
                    let _ = { #handler };
                    // 观察事实仍允许组件处理与冒泡，但不能向 Agent 伪报无人接收。
                    return ::uix::prelude::EventResult::Bubbled;
                }
                // 继续交付组件自身处理器并向父节点冒泡。
                ::uix::prelude::EventResult::NotHandled
            })
        });
    }
    // 鼠标进入与离开使用原始指针事件且不吞掉组件处理器。
    if matches!(attribute.name.as_str(), "@mouseEnter" | "@mouseLeave") {
        // 事件解析器保证事件值是表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回内部形状保护诊断。
            return Err(Diagnostic::new(
                attribute.span,
                "鼠标事件处理器必须是受限表达式",
                "使用 @mouseEnter=\"handler()\" 或 @mouseLeave=\"handler()\"",
            ));
        };
        // 当前两个无载荷事件不伪装为 ClickEvent。
        if expression_uses_event(&expression.expression) {
            // 返回精确载荷边界诊断。
            return Err(Diagnostic::new(
                expression.span,
                "$event 不在 @mouseEnter/@mouseLeave 中提供载荷",
                "在 @mouseEnter/@mouseLeave 中调用无参数处理器",
            ));
        }
        // 生成无事件参数的处理器主体。
        let handler = generate_handler_expression(&expression.expression, None)?;
        // 选择对应系统指针事件分支。
        let event_variant = if attribute.name == "@mouseEnter" {
            // 鼠标进入映射到 PointerEnter。
            quote! { ::uix::prelude::SystemEvent::PointerEnter }
        } else {
            // 鼠标离开映射到 PointerLeave。
            quote! { ::uix::prelude::SystemEvent::PointerLeave }
        };
        // 返回不会截断组件自身 Enter/Leave 的指针监听器。
        return Ok(quote! {
            // 复用公开指针事件注册入口。
            (#view).on_pointer(move |__uix_pointer_event| {
                // 只在声明的进入或离开事件执行语言处理器。
                if matches!(__uix_pointer_event, &#event_variant) {
                    // 丢弃处理器返回值并保留副作用。
                    let _ = { #handler };
                }
                // 继续交付组件自身处理器并向父节点冒泡。
                ::uix::prelude::EventResult::NotHandled
            })
        });
    }
    // 其余核心事件当前只登记 click。
    if attribute.name != "@click" {
        // 返回未登记事件诊断。
        return Err(Diagnostic::new(
            // 指向完整事件属性。
            attribute.span,
            // 说明缺少事件映射。
            format!("事件 {} 尚无已登记的 Rust API 映射", attribute.name),
            // 给出当前支持集合。
            "当前核心 Gate 使用 @click、@keyDown、@keyUp、@mouseEnter 或 @mouseLeave；其他事件由内置组件映射矩阵登记",
        ));
    }
    // 事件解析器保证事件值是表达式。
    let AttributeValue::Expression(expression) = &attribute.value else {
        // 返回内部形状保护诊断。
        return Err(Diagnostic::new(
            // 指向完整事件属性。
            attribute.span,
            // 说明事件值形状错误。
            "事件处理器必须是受限表达式",
            // 给出规范写法。
            "使用 @click=\"handler()\"",
        ));
    };
    // 需要事件载荷时使用公开 on_click_event。
    if expression_uses_event(&expression.expression) {
        // 创建卫生的语义事件变量。
        let semantic_event = Ident::new("__uix_semantic_event", Span::mixed_site());
        // 创建卫生的点击载荷变量。
        let click_event = Ident::new("__uix_click_event", Span::mixed_site());
        // 生成把 $event 映射到 ClickEvent 的处理器主体。
        let handler =
            generate_event_handler_expression(&expression.expression, &click_event, "@click")?;
        // 返回带点击载荷筛选的事件链。
        return Ok(quote! {
            // 使用公开点击事件注册入口。
            (#view).on_click_event(move |#semantic_event| {
                // 只在语义事件带点击载荷时执行语言处理器。
                if let ::std::option::Option::Some(#click_event) = #semantic_event.click_payload() {
                    // 丢弃处理器返回值并保留副作用。
                    let _ = { #handler };
                }
            })
        });
    }
    // 不读取事件载荷时使用轻量点击闭包。
    let handler = generate_handler_expression(&expression.expression, None)?;
    // 返回无事件参数点击链。
    Ok(quote! {
        // 使用公开无状态点击入口。
        (#view).on_click_fn(move || {
            // 丢弃处理器返回值并保留副作用。
            let _ = { #handler };
        })
    })
}

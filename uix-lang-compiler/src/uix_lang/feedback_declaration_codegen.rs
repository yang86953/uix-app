// 引入卫生事件变量所需标识符和令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入属性值、表达式与诊断辅助。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, boolean_value,
    generate_event_handler_expression, literal_string, numeric_value, string_value,
};

// 生成 Message 的零布局 keyed 声明组件。
pub(crate) fn generate_message_declaration(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Message 声明不承载业务子树。
    reject_children(element, "Message")?;
    // key 是窗口内同类型声明的必需稳定身份。
    let key_attribute = required_attribute(element, "key")?;
    // 静态空 key 在宏展开期直接拒绝。
    reject_empty_key(key_attribute, "Message")?;
    // 生成字符串字面量或受限字符串表达式。
    let key = string_value(key_attribute)?;
    // content 是必需展示内容。
    let content = string_value(required_attribute(element, "content")?)?;
    // 从公开声明构造器开始。
    let mut widget = quote! {
        ::uix_app::prelude::MessageDeclaration::new(#key, #content)
    };
    // 应用有限状态关键字。
    if let Some(attribute) = find_attribute(element, "type") {
        // 静态映射为公开状态枚举。
        let status = status_value(attribute, "Message")?;
        // 保存声明状态等级。
        widget = quote! { (#widget).type_(#status) };
    }
    // duration 以秒为 UIX 单位。
    if let Some(attribute) = find_attribute(element, "duration") {
        // 静态负数字面量在宏展开期拒绝。
        reject_negative_duration(attribute, "Message")?;
        // 生成有限字面量或动态数值表达式。
        let duration = numeric_value(attribute)?;
        // 运行时再次校验动态有限非负边界并转换为毫秒。
        widget = quote! { (#widget).duration_seconds((#duration) as f64) };
    }
    // closable 接受布尔简写、字面量或表达式。
    if let Some(attribute) = find_attribute(element, "closable") {
        // 复用统一布尔诊断。
        let closable = boolean_value(attribute)?;
        // 显式保存最终能力，避免 builder 顺序歧义。
        widget = quote! { (#widget).closable(#closable) };
    }
    // @close 只观察 Host 已确认的类型化关闭事实。
    if let Some(attribute) = find_attribute(element, "@close") {
        // 事件必须是受限表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回事件形状诊断。
            return Err(Diagnostic::new(
                // 指向完整事件属性。
                attribute.span,
                // 说明处理器要求。
                "Message @close 必须是受限处理器表达式",
                // 给出类型化载荷示例。
                "使用 @close=\"on_message_close($event)\"",
            ));
        };
        // 创建卫生的关闭事实变量。
        let closed = Ident::new("__uix_message_closed", Span::mixed_site());
        // 生成裸处理器或显式载荷调用。
        let handler = generate_event_handler_expression(&expression.expression, &closed, "@close")?;
        // 注册需要 Send + Sync 的 owner 回调。
        widget = quote! {
            (#widget).on_close(move |#closed| {
                // 丢弃处理器返回值并保留副作用。
                let _ = { #handler };
            })
        };
    }
    // 物化为零布局叶 View。
    let view = quote! { ::uix_app::prelude::ViewNode::leaf(#widget) };
    // key 同时保留为 View 协调身份，其他专属属性不二次映射。
    apply_common_attributes(
        // 传入已经完成声明配置的 View。
        view,
        // 保留属性源码顺序。
        &element.attributes,
        // key 故意不排除，让协调层使用同一稳定身份。
        &["content", "type", "duration", "closable", "@close"],
    )
}

// 生成 Notification 的零布局 keyed 声明组件。
pub(crate) fn generate_notification_declaration(
    element: &Element,
) -> Result<TokenStream, Diagnostic> {
    // Notification 声明不承载业务子树。
    reject_children(element, "Notification")?;
    // key 是窗口内同类型声明的必需稳定身份。
    let key_attribute = required_attribute(element, "key")?;
    // 静态空 key 在宏展开期直接拒绝。
    reject_empty_key(key_attribute, "Notification")?;
    // 生成字符串字面量或受限字符串表达式。
    let key = string_value(key_attribute)?;
    // title 与 content 都是必需展示内容。
    let title = string_value(required_attribute(element, "title")?)?;
    // 正文映射到运行时 Notification description。
    let content = string_value(required_attribute(element, "content")?)?;
    // 从公开声明构造器开始。
    let mut widget = quote! {
        ::uix_app::prelude::NotificationDeclaration::new(#key, #title, #content)
    };
    // 应用有限状态关键字。
    if let Some(attribute) = find_attribute(element, "type") {
        // 静态映射为公开状态枚举。
        let status = status_value(attribute, "Notification")?;
        // 保存声明状态等级。
        widget = quote! { (#widget).type_(#status) };
    }
    // duration 以秒为 UIX 单位。
    if let Some(attribute) = find_attribute(element, "duration") {
        // 静态负数字面量在宏展开期拒绝。
        reject_negative_duration(attribute, "Notification")?;
        // 生成有限字面量或动态数值表达式。
        let duration = numeric_value(attribute)?;
        // 运行时再次校验动态有限非负边界并转换为毫秒。
        widget = quote! { (#widget).duration_seconds((#duration) as f64) };
    }
    // closable 接受布尔简写、字面量或表达式。
    if let Some(attribute) = find_attribute(element, "closable") {
        // 复用统一布尔诊断。
        let closable = boolean_value(attribute)?;
        // 显式保存最终能力。
        widget = quote! { (#widget).closable(#closable) };
    }
    // @close 只观察 Host 已确认的类型化关闭事实。
    if let Some(attribute) = find_attribute(element, "@close") {
        // 事件必须是受限表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回事件形状诊断。
            return Err(Diagnostic::new(
                // 指向完整事件属性。
                attribute.span,
                // 说明处理器要求。
                "Notification @close 必须是受限处理器表达式",
                // 给出类型化载荷示例。
                "使用 @close=\"on_notification_close($event)\"",
            ));
        };
        // 创建卫生的关闭事实变量。
        let closed = Ident::new("__uix_notification_closed", Span::mixed_site());
        // 生成裸处理器或显式载荷调用。
        let handler = generate_event_handler_expression(&expression.expression, &closed, "@close")?;
        // 注册需要 Send + Sync 的 owner 回调。
        widget = quote! {
            (#widget).on_close(move |#closed| {
                // 丢弃处理器返回值并保留副作用。
                let _ = { #handler };
            })
        };
    }
    // 物化为零布局叶 View。
    let view = quote! { ::uix_app::prelude::ViewNode::leaf(#widget) };
    // key 同时保留为 View 协调身份，其他专属属性不二次映射。
    apply_common_attributes(
        // 传入已经完成声明配置的 View。
        view,
        // 保留属性源码顺序。
        &element.attributes,
        // key 故意不排除，让协调层使用同一稳定身份。
        &["title", "content", "type", "duration", "closable", "@close"],
    )
}

// 拒绝声明节点的可渲染子树。
fn reject_children(element: &Element, tag: &str) -> Result<(), Diagnostic> {
    // 文本或元素子节点都会制造错误布局含义。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶节点形状诊断。
        return Err(Diagnostic::new(
            // 指向完整声明元素。
            element.span,
            // 点名不接受子节点的标签。
            format!("<{tag}> 不接受子节点"),
            // 给出自闭合声明建议。
            format!("使用 <{tag} ... />"),
        ));
    }
    // 空声明形状合法。
    Ok(())
}

// 查找元素上的具名属性。

// 查找声明组件的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        // 诊断标签跟随实际元素名。
        &element.name,
        &format!("为 <{}> 提供非空 {name}", element.name),
    )
}

// 拒绝静态空 key。
fn reject_empty_key(attribute: &Attribute, tag: &str) -> Result<(), Diagnostic> {
    // 只有静态字面量可在宏展开期判断空白。
    if matches!(&attribute.value, AttributeValue::Literal(value) if value.trim().is_empty()) {
        // 返回稳定身份诊断。
        return Err(Diagnostic::new(
            // 指向 key 属性。
            attribute.span,
            // 说明身份约束。
            format!("{tag} key 必须是非空字符串"),
            // 给出稳定 key 示例。
            "使用如 key=\"saved\" 的窗口内稳定身份",
        ));
    }
    // 动态表达式由运行时 owner 再次验证。
    Ok(())
}

// 生成四种反馈状态等级。
fn status_value(attribute: &Attribute, tag: &str) -> Result<TokenStream, Diagnostic> {
    // type 必须在编译期选择有限关键字。
    let status = literal_string(attribute, &format!("{tag} type"))?;
    // 映射到公开状态枚举。
    match status.as_str() {
        // 信息状态。
        "info" => Ok(quote! { ::uix_app::prelude::StatusLevel::Info }),
        // 成功状态。
        "success" => Ok(quote! { ::uix_app::prelude::StatusLevel::Success }),
        // 警告状态。
        "warning" => Ok(quote! { ::uix_app::prelude::StatusLevel::Warning }),
        // 错误状态。
        "error" => Ok(quote! { ::uix_app::prelude::StatusLevel::Error }),
        // 其余值拒绝静默回退。
        _ => Err(Diagnostic::new(
            // 指向非法 type。
            attribute.span,
            // 说明有限集合。
            format!("{tag} type 只支持 info、success、warning 或 error"),
            // 给出规范修复建议。
            "使用已登记的反馈状态关键字",
        )),
    }
}

// 拒绝静态负 duration。
fn reject_negative_duration(attribute: &Attribute, tag: &str) -> Result<(), Diagnostic> {
    // 只对可直接解析的静态字面量执行范围检查。
    if let AttributeValue::Literal(value) = &attribute.value
        && value.parse::<f64>().is_ok_and(|duration| duration < 0.0)
    {
        // 返回秒单位范围诊断。
        return Err(Diagnostic::new(
            // 指向非法 duration。
            attribute.span,
            // 说明非负有限约束。
            format!("{tag} duration 必须是有限非负秒数"),
            // 说明零的特殊含义。
            "使用 0 表示不自动关闭，或使用正秒数",
        ));
    }
    // 动态表达式由运行时声明组件归一化并报告。
    Ok(())
}

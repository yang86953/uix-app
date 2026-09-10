// 引入卫生事件变量所需的标识符、跨度与令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Alert 属性、表达式、值映射与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, boolean_value,
    generate_event_handler_expression, literal_string, string_value,
};

// 生成只接线声明配置与关闭处理器的 Alert 叶节点。
pub(crate) fn generate_alert(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Alert 自身绘制消息与关闭入口，不接受 UIX 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Alert 元素。
            element.span,
            // 说明警告提示不接受子节点。
            "<Alert> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <Alert message=\"磁盘空间不足\" />",
        ));
    }

    // message 是运行时内容来源，必须显式提供。
    let message_attribute = required_attribute(element, "message")?;
    // 生成字符串字面量或受限字符串表达式。
    let message = string_value(message_attribute)?;
    // 缺省状态遵循文档的 info 契约。
    let status = status_value(element)?;
    // 临时借用消息，让运行时组件复制并独占内容。
    let mut widget = quote! { ::uix_app::prelude::Alert::new(&*(#message)).type_(#status) };

    // 可关闭能力接受布尔简写、字面量或表达式。
    if let Some(attribute) = find_attribute(element, "closable") {
        // 复用统一布尔值诊断与动态表达式生成。
        let closable = boolean_value(attribute)?;
        // 静态 false 保留运行时默认值。
        if !matches!(&attribute.value, AttributeValue::Literal(value) if value == "false") {
            // 静态 true 直接启用无参数构建器。
            if matches!(&attribute.value, AttributeValue::Literal(value) if value == "true") {
                // 调用公开关闭能力入口。
                widget = quote! { (#widget).closable() };
            } else {
                // 动态布尔值用同类型分支选择初始能力。
                widget = quote! { if #closable { (#widget).closable() } else { #widget } };
            }
        }
    }

    // 先物化公开叶节点，关闭处理器与样式都由 View 契约拥有。
    let mut view = quote! { ::uix_app::prelude::ViewNode::leaf(#widget) };
    // 可选关闭事件只观察 Alert 已建立的 closed 变更事实。
    if let Some(attribute) = find_attribute(element, "@close") {
        // 事件解析器应始终提供受限表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回内部形状保护诊断。
            return Err(Diagnostic::new(
                // 指向完整关闭事件属性。
                attribute.span,
                // 说明关闭处理器形状。
                "Alert @close 必须是受限处理器表达式",
                // 给出带载荷的规范写法。
                "使用 @close=\"on_close($event)\"",
            ));
        };
        // 创建卫生的变更文本变量。
        let value = Ident::new("__uix_alert_change", Span::mixed_site());
        // 生成裸处理器或显式载荷调用。
        let handler = generate_event_handler_expression(&expression.expression, &value, "@change")?;
        // 使用公开 Change 注册入口，并过滤未来可能新增的其他变更事实。
        view = quote! {
            (#view).on_change_fn(move |#value| {
                // 只有运行时确认关闭后才执行语言处理器。
                if #value == "closed" {
                    // 丢弃处理器返回值并保留副作用。
                    let _ = { #handler };
                }
            })
        };
    }

    // 消费 Alert 专有属性后应用统一尺寸、样式、身份与其他公共事件。
    apply_common_attributes(
        // 传入已经配置关闭事件的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止 Alert 专有属性和关闭事件被二次映射。
        &["message", "type", "closable", "@close"],
    )
}

// 生成 Alert 的静态状态关键字。
fn status_value(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 未声明类型时显式生成公开信息状态。
    let Some(attribute) = find_attribute(element, "type") else {
        // 返回文档缺省状态。
        return Ok(quote! { ::uix_app::prelude::StatusLevel::Info });
    };
    // 状态必须在编译期映射为公开枚举。
    let status = literal_string(attribute, "Alert type")?;
    // 把文档关键字映射到公开运行时枚举。
    match status.as_str() {
        // 映射信息提示。
        "info" => Ok(quote! { ::uix_app::prelude::StatusLevel::Info }),
        // 映射成功提示。
        "success" => Ok(quote! { ::uix_app::prelude::StatusLevel::Success }),
        // 映射警告提示。
        "warning" => Ok(quote! { ::uix_app::prelude::StatusLevel::Warning }),
        // 映射错误提示。
        "error" => Ok(quote! { ::uix_app::prelude::StatusLevel::Error }),
        // 未登记关键字必须在编译期拒绝。
        _ => Err(Diagnostic::new(
            // 指向非法类型属性。
            attribute.span,
            // 说明未登记值。
            format!("Alert type={status:?} 不受支持"),
            // 给出完整合法集合。
            "使用 info、success、warning 或 error",
        )),
    }
}

// 查找元素上的具名属性。

// 查找 Alert 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "Alert",
        "使用 <Alert message=\"提示内容\" />",
    )
}

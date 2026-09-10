// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Mentions 属性、表达式与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, generate_expression, string_value};

// 生成绑定提及候选集合与完整输入文本状态的 Mentions 组件。
pub(crate) fn generate_mentions(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Mentions 是叶组件，不能静默丢弃子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Mentions 元素。
            element.span,
            // 说明提及输入组件不接受子节点。
            "<Mentions> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <Mentions value={message} suggestions={members} />",
        ));
    }

    // 查找文档要求的字符串提及候选表达式。
    let suggestions_attribute = required_attribute(element, "suggestions")?;
    // suggestions 必须保留调用侧 Vec<String> 类型检查。
    let AttributeValue::Expression(suggestions_expression) = &suggestions_attribute.value else {
        // 返回候选表达式形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 suggestions 属性。
            suggestions_attribute.span,
            // 说明公开运行时候选类型。
            "Mentions suggestions 必须是 Vec<String> 表达式",
            // 给出规范候选引用写法。
            "使用 suggestions={members}",
        ));
    };
    // 生成受限提及候选表达式。
    let suggestions = generate_expression(&suggestions_expression.expression, None)?;
    // 查找完整输入文本的双向绑定。
    let value_attribute = required_attribute(element, "value")?;
    // value 字面量不能提供双向状态所有权。
    let AttributeValue::Expression(value_expression) = &value_attribute.value else {
        // 返回绑定形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 value 属性。
            value_attribute.span,
            // 说明公开运行时绑定类型。
            "Mentions value 必须绑定 State<String> 表达式",
            // 给出规范状态引用写法。
            "使用 value={message}",
        ));
    };
    // 生成受限完整文本状态表达式。
    let state = generate_expression(&value_expression.expression, None)?;
    // 可选占位文本支持字符串字面量与受限表达式。
    let placeholder = find_attribute(element, "placeholder")
        // 将找到的属性统一转换为字符串令牌。
        .map(string_value)
        // 把可选结果转换为结果中的可选令牌。
        .transpose()?
        // 未声明占位文本时使用空字符串满足运行时构造契约。
        .unwrap_or_else(|| quote!(""));

    // 运行时先接收占位文本和候选，再绑定完整输入文本状态。
    let widget = quote! {
        ::uix_app::prelude::Mentions::new((#placeholder).to_string())
            .options((#suggestions).clone())
            .bind_value(&(#state))
    };
    // 物化为公开叶 View，再应用统一尺寸、样式与自动化属性。
    let view = quote! { ::uix_app::prelude::ViewNode::leaf(#widget) };
    // 消费 Mentions 专有属性并返回公共 View 表达式。
    apply_common_attributes(
        // 传入已经配置的提及输入 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["suggestions", "value", "placeholder"],
    )
}

// 查找 Mentions 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "Mentions",
        "使用 <Mentions value={message} suggestions={members} />",
    )
}

// 查找元素上的具名属性。

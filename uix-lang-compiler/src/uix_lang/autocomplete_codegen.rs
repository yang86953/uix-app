// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 AutoComplete 属性、表达式与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, generate_expression, string_value};

// 生成绑定候选集合与输入文本状态的自动完成组件。
pub(crate) fn generate_autocomplete(element: &Element) -> Result<TokenStream, Diagnostic> {
    // AutoComplete 是叶组件，不能静默丢弃子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 AutoComplete 元素。
            element.span,
            // 说明自动完成组件不接受子节点。
            "<AutoComplete> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <AutoComplete value={query} options={suggestions} />",
        ));
    }

    // 查找文档要求的字符串候选表达式。
    let options_attribute = required_attribute(element, "options")?;
    // options 必须保留调用侧 Vec<String> 类型检查。
    let AttributeValue::Expression(options_expression) = &options_attribute.value else {
        // 返回候选表达式形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 options 属性。
            options_attribute.span,
            // 说明公开运行时数据要求。
            "AutoComplete options 必须是 Vec<String> 表达式",
            // 给出规范候选引用写法。
            "使用 options={suggestions}",
        ));
    };
    // 生成受限候选表达式。
    let options = generate_expression(&options_expression.expression, None)?;
    // 查找输入文本与选中结果的双向绑定。
    let value_attribute = required_attribute(element, "value")?;
    // value 字面量不能提供双向状态所有权。
    let AttributeValue::Expression(value_expression) = &value_attribute.value else {
        // 返回绑定形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 value 属性。
            value_attribute.span,
            // 说明公开运行时绑定类型。
            "AutoComplete value 必须绑定 State<String> 表达式",
            // 给出规范状态引用写法。
            "使用 value={query}",
        ));
    };
    // 生成受限状态表达式。
    let state = generate_expression(&value_expression.expression, None)?;

    // 运行时先接收候选，再绑定输入文本状态。
    let mut widget = quote! {
        ::uix::prelude::AutoComplete::new()
            .options((#options).clone())
            .bind_value(&(#state))
    };
    // 可选占位文本支持字符串字面量与受限表达式。
    if let Some(attribute) = find_attribute(element, "placeholder") {
        // 生成统一字符串属性令牌。
        let placeholder = string_value(attribute)?;
        // 借用字符串并应用公开占位文本构建器。
        widget = quote! { (#widget).placeholder(&(#placeholder)) };
    }
    // 物化为公开叶 View，再应用统一尺寸、样式与自动化属性。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费 AutoComplete 专有属性并返回公共 View 表达式。
    apply_common_attributes(
        // 传入已经配置的自动完成 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["options", "value", "placeholder"],
    )
}

// 查找 AutoComplete 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "AutoComplete",
        "使用 <AutoComplete value={query} options={suggestions} />",
    )
}

// 查找元素上的具名属性。

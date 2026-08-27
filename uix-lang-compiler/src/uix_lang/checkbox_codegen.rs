// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Checkbox 属性、表达式、值转换与诊断契约。
use super::{
    AttributeValue, Diagnostic, Element, boolean_value, generate_expression,
    string_value,
};

// 生成保持 State<bool> 双向绑定的复选框节点。
pub(crate) fn generate_checkbox(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Checkbox 是叶组件，子树不能被静默忽略。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Checkbox 元素。
            element.span,
            // 说明复选框不接受子节点。
            "<Checkbox> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <Checkbox text=\"同意协议\" checked={accepted} />",
        ));
    }

    // 文本属性直接建立公开构造器标签。
    let label = if let Some(attribute) = find_attribute(element, "text") {
        // 生成字符串字面量或受限 String 表达式。
        string_value(attribute)?
    } else {
        // 缺省标签保持运行时空文本契约。
        quote! { "" }
    };
    // 从公开 Checkbox 构造器开始配置。
    let mut widget = quote! { ::uix::prelude::Checkbox::new(#label) };
    // 禁用状态接受静态或动态布尔值。
    if let Some(attribute) = find_attribute(element, "disabled") {
        // 生成统一布尔属性令牌。
        let disabled = boolean_value(attribute)?;
        // 应用公开禁用构建器。
        widget = quote! { (#widget).disabled(#disabled) };
    }
    // 可选 checked 必须保留 State<bool> 所有权句柄。
    if let Some(attribute) = find_attribute(element, "checked") {
        // 字面量不能提供双向状态所有权。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回绑定形状诊断。
            return Err(Diagnostic::new(
                // 指向非法 checked 属性。
                attribute.span,
                // 说明公开运行时绑定类型。
                "Checkbox checked 必须绑定 State<bool> 表达式",
                // 给出规范绑定写法。
                "使用 checked={accepted}",
            ));
        };
        // 生成受限状态表达式。
        let state = generate_expression(&expression.expression, None)?;
        // 借用状态句柄交给公开 Checkbox 双向绑定入口。
        widget = quote! { (#widget).checked(&(#state)) };
    }

    // 物化为公开叶 View，再应用统一尺寸、样式与自动化属性。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费 Checkbox 专有属性并返回公共 View 表达式。
    apply_common_attributes(
        // 传入已经配置的复选框 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["checked", "disabled", "text"],
    )
}

// 查找元素上的具名属性。

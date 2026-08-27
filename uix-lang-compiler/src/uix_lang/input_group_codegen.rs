// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 InputGroup 属性、表达式、字符串与诊断契约。
use super::{AttributeValue, Diagnostic, Element, generate_expression, string_value};

// 生成复用单一 Input 的前后文本组合。
pub(crate) fn generate_input_group(element: &Element) -> Result<TokenStream, Diagnostic> {
    // InputGroup 自身是叶组合，不能静默忽略子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组合形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 InputGroup 元素。
            element.span,
            // 说明复合输入不接受子节点。
            "<InputGroup> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <InputGroup addonBefore=\"¥\" value={amount} addonAfter=\"元\" />",
        ));
    }

    // 从公开复合输入构造器开始配置。
    let mut group = quote! { ::uix::prelude::InputGroup::new() };
    // 可选前置文本映射到公开 addon_before 构建器。
    if let Some(attribute) = find_attribute(element, "addonBefore") {
        // 生成字符串字面量或受限 String 表达式。
        let value = string_value(attribute)?;
        // 应用前置附加文本。
        group = quote! { (#group).addon_before(#value) };
    }
    // 可选 value 必须保留 State<String> 所有权句柄。
    if let Some(attribute) = find_attribute(element, "value") {
        // 字面量不能提供双向状态所有权。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回绑定形状诊断。
            return Err(Diagnostic::new(
                // 指向非法 value 属性。
                attribute.span,
                // 说明公开运行时绑定类型。
                "InputGroup value 必须绑定 State<String> 表达式",
                // 给出规范绑定写法。
                "使用 value={amount}",
            ));
        };
        // 生成受限状态表达式。
        let state = generate_expression(&expression.expression, None)?;
        // 借用状态句柄交给公开 InputGroup 双向绑定入口。
        group = quote! { (#group).value(&(#state)) };
    }
    // 可选后置文本映射到公开 addon_after 构建器。
    if let Some(attribute) = find_attribute(element, "addonAfter") {
        // 生成字符串字面量或受限 String 表达式。
        let value = string_value(attribute)?;
        // 应用后置附加文本。
        group = quote! { (#group).addon_after(#value) };
    }

    // 专有属性消费后应用统一尺寸、样式与自动化属性。
    apply_common_attributes(
        // 传入实现公开 Into<ViewNode> 的 InputGroup。
        group,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["addonBefore", "value", "addonAfter"],
    )
}

// 查找元素上的具名属性。

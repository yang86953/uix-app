// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与可见子节点判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入属性解析、表达式生成与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, generate_expression, numeric_value};

// 生成带状态回写能力的 BackTop 叶组件。
pub(crate) fn generate_back_top(element: &Element) -> Result<TokenStream, Diagnostic> {
    // BackTop 自己绘制按钮，不接受会被静默丢弃的内容子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 BackTop。
            element.span,
            // 说明不接受子节点。
            "<BackTop> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <BackTop scrollY={scroll_y} />",
        ));
    }
    // scrollY 是可见性与激活回写所需的应用状态。
    let scroll_attribute = find_attribute(element, "scrollY").ok_or_else(|| {
        // 返回缺失绑定诊断。
        Diagnostic::new(
            // 指向完整 BackTop。
            element.span,
            // 说明缺少必需状态。
            "<BackTop> 缺少必需的 scrollY 状态绑定",
            // 给出最小合法写法。
            "使用 scrollY={scroll_y}",
        )
    })?;
    // 状态句柄必须来自表达式而不是字面量。
    let AttributeValue::Expression(scroll_expression) = &scroll_attribute.value else {
        // 返回绑定形状诊断。
        return Err(Diagnostic::new(
            // 指向非法属性。
            scroll_attribute.span,
            // 说明公开运行时类型要求。
            "BackTop scrollY 必须绑定 State<f32> 表达式",
            // 给出合法绑定示例。
            "使用 scrollY={scroll_y}",
        ));
    };
    // 生成受限 Rust 状态表达式。
    let scroll_state = generate_expression(&scroll_expression.expression, None)?;
    // 从公开默认构造器开始配置。
    let mut widget = quote! { ::uix::prelude::BackTop::new() };
    // 可选阈值支持长度字面量与受限数值表达式。
    if let Some(attribute) = find_attribute(element, "threshold") {
        // 生成 f32 阈值表达式。
        let threshold = numeric_value(attribute)?;
        // 映射到现有公开 visibility_height 构建器。
        widget = quote! { (#widget).visibility_height(#threshold) };
    }
    // 最后绑定应用拥有的滚动状态句柄。
    widget = quote! { (#widget).scroll_state(&(#scroll_state)) };
    // 先物化为公开叶 View，再应用统一尺寸、样式与自动化属性。
    let base = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费专有属性并返回公共 View 表达式。
    apply_common_attributes(base, &element.attributes, &["threshold", "scrollY"])
}

// 查找元素上的具名属性。
fn find_attribute<'a>(element: &'a Element, name: &str) -> Option<&'a Attribute> {
    // 解析器已保证同名属性唯一。
    element
        .attributes
        .iter()
        .find(|attribute| attribute.name == name)
}

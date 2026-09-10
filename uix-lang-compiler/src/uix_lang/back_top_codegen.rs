// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与可见子节点判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入属性解析、表达式生成与诊断契约。
use super::{AttributeValue, Diagnostic, Element, generate_expression, numeric_value};

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
    // 根据属性形状生成状态绑定或只读数值快照调用。
    let scroll_configuration = match &scroll_attribute.value {
        // 表达式保留既有 State<f32> 双向状态所有权。
        AttributeValue::Expression(scroll_expression) => {
            // 生成受限 Rust 状态表达式。
            let scroll_state = generate_expression(&scroll_expression.expression, None)?;
            // 借用应用拥有的状态句柄。
            quote! { .scroll_state(&(#scroll_state)) }
        }
        // 数值字面量只建立本轮只读滚动快照。
        AttributeValue::Literal(_) => {
            // 复用有限数值与长度字面量诊断。
            let scroll_y = numeric_value(scroll_attribute)?;
            // 调用公开只读快照构建器。
            quote! { .scroll_y(#scroll_y) }
        }
        // 内联样式对象不能表达滚动位置。
        AttributeValue::InlineStyle(_) => {
            // 返回精确的属性形状诊断。
            return Err(Diagnostic::new(
                // 指向非法 scrollY 属性。
                scroll_attribute.span,
                // 说明允许的两种公开输入。
                "BackTop scrollY 必须绑定 State<f32> 或使用数值字面量",
                // 给出状态绑定和只读快照示例。
                "使用 scrollY={scroll_y} 或 scrollY=\"450\"",
            ));
        }
    };
    // 从公开默认构造器开始配置。
    let mut widget = quote! { ::uix_app::prelude::BackTop::new() };
    // 可选阈值支持长度字面量与受限数值表达式。
    if let Some(attribute) = find_attribute(element, "threshold") {
        // 生成 f32 阈值表达式。
        let threshold = numeric_value(attribute)?;
        // 映射到现有公开 visibility_height 构建器。
        widget = quote! { (#widget).visibility_height(#threshold) };
    }
    // 最后应用状态绑定或只读快照配置。
    widget = quote! { (#widget) #scroll_configuration };
    // 先通过公开 View 契约进入 BackTop 的 UIX 声明壳，再应用调用方公共属性。
    let base = quote! { ::uix_app::prelude::View::build(#widget) };
    // 消费专有属性并返回公共 View 表达式。
    apply_common_attributes(base, &element.attributes, &["threshold", "scrollY"])
}

// 查找元素上的具名属性。

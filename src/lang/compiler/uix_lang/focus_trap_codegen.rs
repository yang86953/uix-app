// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与有序子树生成入口。
use super::codegen::{apply_common_attributes, generate_children};
// 引入 FocusTrap 布尔属性、元素与诊断契约。
use super::{Diagnostic, Element, boolean_value};

// 生成保留完整有序焦点作用域子树的 FocusTrap 容器。
pub(crate) fn generate_focus_trap(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 从运行时默认启用的焦点陷阱开始构造。
    let mut widget = quote! { ::uix_app::prelude::FocusTrap::new() };
    // 可选 active 只声明当前作用域是否接管焦点循环。
    if let Some(attribute) = element
        // 借用有序属性列表。
        .attributes
        // 遍历全部属性。
        .iter()
        // 查找 FocusTrap 专有开关。
        .find(|attribute| attribute.name == "active")
    {
        // 接受布尔简写、字面量或受限表达式。
        let active = boolean_value(attribute)?;
        // 可聚焦后代解析仍由运行时 WidgetTree 负责。
        widget = quote! { (#widget).active(#active) };
    }
    // 按源码顺序生成普通节点与 If/For 控制流。
    let children = generate_children(&element.children)?;
    // 使用公开 ViewNode 子树声明稳定焦点作用域身份。
    let view = quote! {
        // 构造默认启用的焦点陷阱并保留全部后代。
        ::uix_app::prelude::ViewNode::new(#widget, #children)
    };
    // 消费 active 后应用公共样式、身份与事件。
    apply_common_attributes(view, &element.attributes, &["active"])
    // 结束 FocusTrap 生成函数。
}

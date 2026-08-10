// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与有序子树生成入口。
use super::codegen::{apply_common_attributes, generate_children};
// 引入 FocusTrap 元素与诊断契约。
use super::{Diagnostic, Element};

// 生成保留完整有序焦点作用域子树的 FocusTrap 容器。
pub(crate) fn generate_focus_trap(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 按源码顺序生成普通节点与 If/For 控制流。
    let children = generate_children(&element.children)?;
    // 使用公开 ViewNode 子树声明稳定焦点作用域身份。
    let view = quote! {
        // 构造默认启用的焦点陷阱并保留全部后代。
        ::uix::prelude::ViewNode::new(::uix::prelude::FocusTrap::new(), #children)
    };
    // FocusTrap 没有专有 UIX 属性，仅应用公共样式、身份与事件。
    apply_common_attributes(view, &element.attributes, &[])
    // 结束 FocusTrap 生成函数。
}

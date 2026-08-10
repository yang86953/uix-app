// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Calendar 诊断与元素语法树。
use super::{Diagnostic, Element};

// 生成使用运行时默认交互契约的 Calendar 叶节点。
pub(crate) fn generate_calendar(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Calendar 自身管理日期格、标题与交互，不接受 UIX 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Calendar 元素。
            element.span,
            // 说明日历不接受子节点。
            "<Calendar> 不接受子节点",
            // 给出当前文档化的自闭合写法。
            "使用 <Calendar />",
        ));
    }

    // 复用公开 Calendar 构造器及其运行时选择和导航所有权。
    let view = quote! {
        ::uix::prelude::ViewNode::leaf(::uix::prelude::Calendar::new())
    };
    // Calendar 当前没有专有 UIX 属性，只应用公共 View 属性。
    apply_common_attributes(
        // 传入公开日历叶 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 空集合确保未声明的专有属性被明确拒绝。
        &[],
    )
}

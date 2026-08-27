// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性、有序子树生成与可见节点判定入口。
use super::codegen::{apply_common_attributes, generate_children, is_renderable_node};
// 引入 Spin 属性、值映射、语法树与诊断契约。
use super::{Diagnostic, Element, boolean_value, string_value};

// 生成保留运行时动画所有权与可选遮罩子树的 Spin 加载指示器。
pub(crate) fn generate_spin(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 从公开默认构造器开始，保留缺省 spinning=true。
    let mut widget = quote! { ::uix::prelude::Spin::new() };

    // 可选 spinning 接受布尔简写、字面量或受限表达式。
    if let Some(attribute) = find_attribute(element, "spinning") {
        // 复用统一布尔值诊断并保留 Rust 类型检查。
        let spinning = boolean_value(attribute)?;
        // 把声明配置交给公开运行时构建器。
        widget = quote! { (#widget).spinning(#spinning) };
    }
    // 可选 text 接受字符串字面量或受限字符串表达式。
    if let Some(attribute) = find_attribute(element, "text") {
        // 生成调用方持有的提示文字表达式。
        let text = string_value(attribute)?;
        // 临时借用提示文字，让运行时 Spin 复制并独占内容。
        widget = quote! { (#widget).tip(&*(#text)) };
    }

    // 只要声明了可生成子树，就启用运行时遮罩布局模式。
    if element.children.iter().any(is_renderable_node) {
        // 动态 If/For 仍保持稳定的容器身份，不猜测运行时子项数量。
        widget = quote! { (#widget).wrapper_mode() };
    }
    // 按源码顺序生成普通节点与 If/For 控制流。
    let children = generate_children(&element.children)?;
    // 经公开桥接进入 Spin 自己的同目录 UIX 根声明，并原样移交拥有型子树。
    let view = quote! { (#widget).build_view_with_children(#children) };
    // 消费 Spin 专有属性并应用公共尺寸、样式、身份与事件。
    apply_common_attributes(view, &element.attributes, &["spinning", "text"])
    // 结束 Spin 生成函数。
}

// 查找元素上的具名属性。

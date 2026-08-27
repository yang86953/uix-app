// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入有序子树与公共属性生成入口。
use super::codegen::{apply_common_attributes, generate_children};
// 引入布局壳属性解析与诊断契约。
use super::{Diagnostic, Element, boolean_value, literal_string};

// 生成 Layout / Sider / Header / Content / Footer 应用布局壳。
pub(crate) fn generate_app_layout(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 先生成保持控制流与来源顺序的直接子树。
    let children = generate_children(&element.children)?;
    // 按标签选择公开运行时组件与专有属性集合。
    let (widget, consumed) = match element.name.as_str() {
        // Layout 拥有应用壳主轴方向。
        "Layout" => (generate_layout(element)?, &["direction"][..]),
        // Sider 拥有可折叠配置。
        "Sider" => (generate_sider(element)?, &["collapsible"][..]),
        // Header 使用文档锚点默认高度。
        "Header" => (quote! { ::uix::prelude::Header::default() }, &[][..]),
        // Content 默认占用纵向 Layout 的剩余空间。
        "Content" => (quote! { ::uix::prelude::Content::new() }, &[][..]),
        // Footer 使用与 Header 对称的默认高度。
        "Footer" => (quote! { ::uix::prelude::Footer::default() }, &[][..]),
        // 核心生成器只会把封闭的五类标签路由到此入口。
        _ => unreachable!("未知应用布局壳标签"),
    };
    // 用公开 ViewNode 建立区域组件对有序内容子树的组合所有权。
    let base = quote! {
        // 传入公开运行时组件与已经生成的子 View 向量。
        ::uix::prelude::ViewNode::new(#widget, #children)
    };
    // 专有属性消费后继续应用统一尺寸、样式、事件与自动化属性。
    apply_common_attributes(base, &element.attributes, consumed)
}

// 生成带文档化主轴方向的 Layout。
fn generate_layout(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 从公开默认纵向布局壳开始。
    let mut layout = quote! { ::uix::prelude::Layout::new() };
    // 可选方向必须在编译期决定布局主轴。
    if let Some(attribute) = find_attribute(element, "direction") {
        // 读取确定方向关键字。
        let direction = literal_string(attribute, "Layout direction")?;
        // 映射到公开共享布局方向。
        layout = match direction.as_str() {
            // row 建立侧栏与嵌套壳的横向结构。
            "row" => quote! {
                // 调用公开 Layout 方向构建器。
                (#layout).direction(::uix::prelude::FlexDirection::Row)
            },
            // column 显式保持 Header / Content / Footer 的纵向结构。
            "column" => quote! {
                // 调用公开 Layout 方向构建器。
                (#layout).direction(::uix::prelude::FlexDirection::Column)
            },
            // 未登记方向不得静默近似。
            _ => {
                // 返回来源精确的方向诊断。
                return Err(Diagnostic::new(
                    // 指向非法 direction 属性。
                    attribute.span,
                    // 说明封闭方向集合。
                    "Layout direction 只支持 row 或 column",
                    // 给出两种合法写法。
                    "使用 direction=\"row\" 或 direction=\"column\"",
                ));
            }
        };
    }
    // 返回已经验证的公开组件表达式。
    Ok(layout)
}

// 生成带可折叠配置的 Sider。
fn generate_sider(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 从二百逻辑像素文档默认侧栏开始。
    let mut sider = quote! { ::uix::prelude::Sider::default() };
    // 可选 collapsible 接受布尔简写、字面量或受限表达式。
    if let Some(attribute) = find_attribute(element, "collapsible") {
        // 生成布尔配置值。
        let collapsible = boolean_value(attribute)?;
        // 映射到公开 Sider 构建器。
        sider = quote! { (#sider).collapsible(#collapsible) };
    }
    // 返回已经配置的公开组件表达式。
    Ok(sider)
}

// 查找元素上的具名属性。

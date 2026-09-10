// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
// 引入共享元素属性查找。
use super::codegen::{apply_common_attributes, is_renderable_node};
use super::find_attribute;
// 引入 Empty 诊断与共享字符串生成契约。
use super::{Diagnostic, Element, string_value};

// 生成只投影静态内容配置的 Empty 叶节点。
pub(crate) fn generate_empty(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Empty 是叶组件，子树不能被静默忽略。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Empty 元素。
            element.span,
            // 说明空状态不接受子节点。
            "<Empty> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <Empty description=\"暂无数据\" icon=\"inbox\" />",
        ));
    }

    // 从公开默认构造器开始配置。
    let mut widget = quote! { ::uix_app::prelude::Empty::new() };
    // 可选描述接受字符串字面量或受限字符串表达式。
    if let Some(attribute) = find_attribute(element, "description") {
        // 生成描述字符串令牌。
        let description = string_value(attribute)?;
        // 调用公开描述构建器。
        widget = quote! { (#widget).description(#description) };
    }
    // 可选图标名接受字符串字面量或受限字符串表达式。
    if let Some(attribute) = find_attribute(element, "icon") {
        // 生成图标名字符串令牌。
        let icon = string_value(attribute)?;
        // 调用公开图标构建器。
        widget = quote! { (#widget).icon(#icon) };
    }

    // 经公开 View 契约进入组件自己的同目录 UIX 声明壳。
    let view = quote! { ::uix_app::prelude::View::build(#widget) };
    // 消费 Empty 专有内容属性并返回公共 View 表达式。
    apply_common_attributes(
        // 传入已经配置的空状态 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["description", "icon"],
    )
}

// 查找元素上的具名属性。

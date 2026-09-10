use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 RichText 属性、诊断与共享值生成契约。
use super::{Attribute, Diagnostic, Element, optional_boolean, string_value};

// 生成只投影 Markdown 内容与选择配置的 RichText 叶节点。
pub(crate) fn generate_rich_text(element: &Element) -> Result<TokenStream, Diagnostic> {
    // RichText 自身解析并绘制内容，不接受可渲染子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 RichText 元素。
            element.span,
            // 说明富文本不接受子节点。
            "<RichText> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <RichText content=\"支持 **富文本**\" />",
        ));
    }

    // content 是运行时解析器的必需输入。
    let content_attribute = required_attribute(element, "content")?;
    // 字面量或受限字符串表达式只在解析期间借用。
    let content = string_value(content_attribute)?;
    // 缺省选择能力显式固定为文档声明的 false。
    let selectable = optional_boolean(element, "selectable")?;
    // 生成层只调用公开解析与组件构建契约，不复制 Markdown 或选择状态。
    let widget = quote! {
        ::uix_app::prelude::RichText::new()
            .content(::uix_app::prelude::parse_rich_text(&*(#content)))
            .selectable(#selectable)
    };
    // RichText 是公开叶 View。
    let view = quote! { ::uix_app::prelude::ViewNode::leaf(#widget) };
    // 消费专有属性并继续应用统一样式与自动化属性。
    apply_common_attributes(
        // 传入已经配置的富文本 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["content", "selectable"],
    )
}

// 生成可选布尔属性，并显式保留 false 默认值。

// 查找 RichText 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "RichText",
        "使用 <RichText content=\"支持 **富文本**\" />",
    )
}

// 查找元素上的具名属性。

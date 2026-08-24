// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Watermark 属性、诊断与共享值生成契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, numeric_value, string_value};

// 生成只投影文字与透明度的 Watermark 叶节点。
pub(crate) fn generate_watermark(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Watermark 自身绘制平铺文字，不接受可渲染子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Watermark 元素。
            element.span,
            // 说明水印不接受子节点。
            "<Watermark> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <Watermark text=\"CONFIDENTIAL\" />",
        ));
    }

    // 水印文字是运行时构造器的必需输入。
    let text_attribute = required_attribute(element, "text")?;
    // 字面量或受限字符串表达式只在构造期间借用。
    let text = string_value(text_attribute)?;
    // 缺省透明度显式固定为运行时文档默认值。
    let opacity = match find_attribute(element, "opacity") {
        // 静态透明度额外执行区间与单位校验。
        Some(attribute) => watermark_opacity_value(attribute)?,
        // 保持 Watermark::default 的 0.15 契约。
        None => quote! { 0.15 },
    };
    // 借用调用方字符串，运行时 Watermark 立即取得文字所有权。
    let widget = quote! {
        ::uix::prelude::Watermark::new(&*(#text))
            .opacity(#opacity)
    };
    // 经公开 View 契约进入 Watermark 自己的同目录 UIX 声明根。
    let view = quote! { ::uix::prelude::View::build(#widget) };
    // 消费专有属性并继续应用统一样式与自动化属性。
    apply_common_attributes(
        // 传入已经配置的水印 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["text", "opacity"],
    )
}

// 生成无单位且位于闭区间内的透明度或受限 f32 表达式。
fn watermark_opacity_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 先复用统一有限 f32 与表达式生成契约。
    let value = numeric_value(attribute)?;
    // 动态表达式由运行时 Watermark::opacity 执行有限值与区间归一化。
    let AttributeValue::Literal(source) = &attribute.value else {
        // 返回已经生成的表达式。
        return Ok(value);
    };
    // opacity 是比例数值，不能借用长度属性的 px 后缀。
    if source.ends_with("px") {
        // 返回明确的无单位诊断。
        return Err(Diagnostic::new(
            // 指向带单位的 opacity 属性。
            attribute.span,
            // 说明属性类型边界。
            "Watermark opacity 必须是无单位数值",
            // 给出合法字面量与动态写法。
            "使用 0 到 1 的有限数值，例如 \"0.15\" 或 f32 表达式",
        ));
    }
    // 前置 numeric_value 已保证字面量可解析且有限。
    let parsed = source
        // 解析与运行时一致的 f32 透明度。
        .parse::<f32>()
        // 解析失败表示共享数值校验契约被破坏。
        .expect("numeric_value 已验证 Watermark opacity");
    // 闭区间内的值无需运行时静默截断。
    if (0.0..=1.0).contains(&parsed) {
        // 返回共享数值令牌。
        return Ok(value);
    }
    // 拒绝超出透明度范围的静态值。
    Err(Diagnostic::new(
        // 指向非法 opacity 属性。
        attribute.span,
        // 说明静态透明度约束。
        "Watermark opacity 必须位于 0 到 1 之间",
        // 给出合法静态与动态写法。
        "使用 0 到 1 的有限数值，例如 \"0.15\" 或 f32 表达式",
    ))
}

// 查找 Watermark 的必需属性。
fn required_attribute<'a>(
    // 借用待检查元素。
    element: &'a Element,
    // 指定必需属性名。
    name: &str,
) -> Result<&'a Attribute, Diagnostic> {
    // 返回现有属性或构造缺失诊断。
    find_attribute(element, name).ok_or_else(|| {
        // 创建指向完整元素的诊断。
        Diagnostic::new(
            // 指向完整 Watermark 元素。
            element.span,
            // 说明具体缺失属性。
            format!("<Watermark> 缺少必需的 {name} 属性"),
            // 给出最小合法写法。
            "使用 <Watermark text=\"CONFIDENTIAL\" />",
        )
    })
}

// 查找元素上的具名属性。
fn find_attribute<'a>(element: &'a Element, name: &str) -> Option<&'a Attribute> {
    // 解析器已保证同名属性唯一。
    element
        // 借用有序属性列表。
        .attributes
        // 遍历每个属性。
        .iter()
        // 返回首个名称匹配项。
        .find(|attribute| attribute.name == name)
}

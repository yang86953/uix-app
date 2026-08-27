// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 QRCode 属性、共享值生成与诊断契约。
use super::{Attribute, Diagnostic, Element, literal_string, numeric_value, string_value};

// 生成 qrcode capability 下的 QRCode 叶节点。
pub(crate) fn generate_qrcode(element: &Element) -> Result<TokenStream, Diagnostic> {
    // QRCode 自身绘制编码矩阵，不接收 View 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 QRCode 元素。
            element.span,
            // 说明二维码不接受子节点。
            "<QRCode> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <QRCode value=\"https://example.com\" />",
        ));
        // 结束叶节点形状检查。
    }

    // value 是二维码编码内容，必须显式提供。
    let value_attribute = required_attribute(element, "value")?;
    // 生成字符串字面量或受限字符串表达式。
    let value = string_value(value_attribute)?;
    // 未声明 size 时显式采用文档默认的 128px。
    let size = match find_attribute(element, "size") {
        // 显式尺寸复用统一 px 与 f32 表达式生成。
        Some(attribute) => numeric_value(attribute)?,
        // 缺省值不得泄漏运行时组件自身的 160px 默认值。
        None => quote! { 128.0 },
    };
    // 未声明 errorLevel 时显式采用文档默认的 M 级。
    let error_level = match find_attribute(element, "errorLevel") {
        // 显式纠错等级进入编译期关键字映射。
        Some(attribute) => error_level_value(attribute)?,
        // 运行时 M 级编码值为 1。
        None => quote! { 1u8 },
    };

    // 构造时临时借用字符串，运行时 QRCode 立即取得内容所有权。
    let widget = quote! {
        ::uix::prelude::QRCode::new(&(#value))
            .size(#size)
            .error_level(#error_level)
    };
    // 经公开 View 契约进入 QRCode 自己的同目录 UIX 声明根。
    let view = quote! { ::uix::prelude::View::build(#widget) };
    // 消费专有属性并应用公共尺寸、样式、事件与自动化属性。
    apply_common_attributes(
        // 传入已经配置的二维码 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["value", "size", "errorLevel"],
    )
    // 结束 QRCode 生成函数。
}

// 把文档纠错等级关键字映射为运行时 u8 编码。
fn error_level_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 纠错等级是封闭关键字集合，不接受运行时表达式。
    let level = literal_string(attribute, "QRCode errorLevel")?;
    // 返回与运行时 EcLevel 顺序一致的编码。
    match level.as_str() {
        // 映射低纠错等级。
        "L" => Ok(quote! { 0u8 }),
        // 映射中等纠错等级。
        "M" => Ok(quote! { 1u8 }),
        // 映射四分之一纠错等级。
        "Q" => Ok(quote! { 2u8 }),
        // 映射高纠错等级。
        "H" => Ok(quote! { 3u8 }),
        // 拒绝文档外的等级关键字。
        _ => Err(Diagnostic::new(
            // 指向非法 errorLevel 属性。
            attribute.span,
            // 说明未登记值。
            format!("QRCode errorLevel={level:?} 不受支持"),
            // 给出完整合法集合。
            "使用 L、M、Q 或 H",
        )),
    }
    // 结束纠错等级生成函数。
}

// 查找元素上的具名属性。

// 查找 QRCode 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "QRCode",
        "使用 <QRCode value=\"https://example.com\" />",
    )
}

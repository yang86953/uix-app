// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Avatar 属性、诊断与共享值生成契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, literal_string, numeric_value, string_value,
};

// 生成只投影头像来源、回退文字、形状与尺寸的 Avatar 叶节点。
pub(crate) fn generate_avatar(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Avatar 是叶组件，子树不能被静默忽略。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Avatar 元素。
            element.span,
            // 说明头像不接受子节点。
            "<Avatar> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <Avatar text=\"AL\" shape=\"circle\" />",
        ));
    }

    // 未声明回退文字时使用运行时默认空字符串。
    let text = match find_attribute(element, "text") {
        // 字面量或受限字符串表达式只在构造期间借用。
        Some(attribute) => string_value(attribute)?,
        // 空字符串保持 Avatar::default 的内容契约。
        None => quote! { "" },
    };
    // 未声明形状时显式采用文档默认圆形。
    let square = match find_attribute(element, "shape") {
        // 显式形状必须在编译期映射为公开布尔构建器。
        Some(attribute) => shape_value(attribute)?,
        // circle 对应运行时 square=false。
        None => quote! { false },
    };
    // 借用调用方字符串并让运行时 Avatar 取得自己的内容所有权。
    let mut widget = quote! { ::uix_app::prelude::Avatar::new(&*(#text)).square(#square) };
    // 图片来源只在显式声明时触发 image-codecs capability 下的公开 API。
    if let Some(attribute) = find_attribute(element, "src") {
        // 生成图片来源字面量或受限字符串表达式。
        let src = string_value(attribute)?;
        // 借用来源字符串，运行时组件复制路径并拥有缓存生命周期。
        widget = quote! { (#widget).src(&*(#src)) };
    }
    // 缺省尺寸保留运行时 Avatar 的 32px 默认值。
    if let Some(attribute) = find_attribute(element, "size") {
        // 静态尺寸在编译期拒绝非正值，动态值交给运行时归一化。
        let size = avatar_size_value(attribute)?;
        // 把边长交给 Avatar 自身固有尺寸契约。
        widget = quote! { (#widget).size(#size) };
    }

    // 经公开 View 契约进入 Avatar 自己的同目录 UIX 声明根。
    let view = quote! { ::uix_app::prelude::View::build(#widget) };
    // 消费 Avatar 专有属性并返回公共 View 表达式。
    apply_common_attributes(
        // 传入已经配置的头像 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["text", "src", "shape", "size"],
    )
}

// 把文档形状关键字映射为运行时 square 开关。
fn shape_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 形状是封闭关键字集合，不接受运行时表达式。
    let shape = literal_string(attribute, "Avatar shape")?;
    // 返回公开 square 构建器所需布尔值。
    match shape.as_str() {
        // 圆形头像关闭方形裁剪。
        "circle" => Ok(quote! { false }),
        // 方形头像开启圆角方形裁剪。
        "square" => Ok(quote! { true }),
        // 拒绝文档外的形状关键字。
        _ => Err(Diagnostic::new(
            // 指向非法 shape 属性。
            attribute.span,
            // 说明未登记值。
            format!("Avatar shape={shape:?} 不受支持"),
            // 给出完整合法集合。
            "使用 circle 或 square",
        )),
    }
}

// 生成正有限头像边长或受限 f32 表达式。
fn avatar_size_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 先复用统一有限 f32 与可选 px 解析。
    let value = numeric_value(attribute)?;
    // 动态表达式由运行时 Avatar::size 保持默认回退契约。
    let AttributeValue::Literal(source) = &attribute.value else {
        // 返回已经生成的表达式。
        return Ok(value);
    };
    // 去除文档允许的 px 后缀以检查静态正值。
    let number = source.strip_suffix("px").unwrap_or(source);
    // 前置 numeric_value 已保证字面量可解析且有限。
    let parsed = number
        // 解析与运行时一致的 f32 边长。
        .parse::<f32>()
        // 解析失败表示共享数值校验契约被破坏。
        .expect("numeric_value 已验证 Avatar size");
    // 正值不会被运行时静默改回默认尺寸。
    if parsed > 0.0 {
        // 返回共享数值令牌。
        return Ok(value);
    }
    // 拒绝零和负边长。
    Err(Diagnostic::new(
        // 指向非法 size 属性。
        attribute.span,
        // 说明头像边长约束。
        "Avatar size 必须大于 0",
        // 给出合法静态与动态写法。
        "使用正的有限边长，例如 \"32px\" 或 f32 表达式",
    ))
}

// 查找元素上的具名属性。

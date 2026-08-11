// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Image 属性、诊断与共享值生成契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, boolean_value, numeric_value, string_value,
};

// 生成只声明图片来源、固有尺寸与运行时配置的 Image 叶节点。
pub(crate) fn generate_image(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Image 自身拥有加载与预览生命周期，不能静默忽略 View 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Image 元素。
            element.span,
            // 说明图片组件不接受子节点。
            "<Image> 不接受子节点",
            // 给出最小合法自闭合写法。
            "使用 <Image src=\"assets/photo.png\" width=\"128px\" height=\"88px\" />",
        ));
    }

    // 图片路径是运行时资源身份，必须显式声明。
    let src_attribute = required_attribute(element, "src")?;
    // 生成字符串字面量或受限字符串表达式。
    let src = string_value(src_attribute)?;
    // 宽度缺省采用公开 Rust 使用示例的 128px 固有尺寸。
    let width = match find_attribute(element, "width") {
        // 显式宽度必须保持正且有限。
        Some(attribute) => positive_dimension_value(attribute, "width")?,
        // 缺省值不依赖图片解码后的像素尺寸。
        None => quote! { 128.0 },
    };
    // 高度缺省采用公开 Rust 使用示例的 88px 固有尺寸。
    let height = match find_attribute(element, "height") {
        // 显式高度必须保持正且有限。
        Some(attribute) => positive_dimension_value(attribute, "height")?,
        // 缺省值让加载前后布局保持稳定。
        None => quote! { 88.0 },
    };

    // 构造运行时 Image，并仅在构造期间借用调用方路径。
    let mut widget = quote! { ::uix::prelude::Image::new(#width, #height).src(&*(#src)) };
    // 可选替代文本继续由运行时持有并投影到语义快照。
    if let Some(attribute) = find_attribute(element, "alt") {
        // 生成字符串字面量或受限字符串表达式。
        let alt = string_value(attribute)?;
        // 只在公开构建器调用期间借用替代文本。
        widget = quote! { (#widget).alt(&*(#alt)) };
    }
    // 可选加载失败文字继续由运行时错误状态决定何时显示。
    if let Some(attribute) = find_attribute(element, "fallback") {
        // 生成字符串字面量或受限字符串表达式。
        let fallback = string_value(attribute)?;
        // 只在公开构建器调用期间借用失败文字。
        widget = quote! { (#widget).fallback(&*(#fallback)) };
    }
    // 可选圆角只声明静态外观，不接管绘制裁剪。
    if let Some(attribute) = find_attribute(element, "radius") {
        // 静态圆角必须非负，动态值交由运行时归一化。
        let radius = non_negative_radius_value(attribute)?;
        // 调用现有圆角构建器。
        widget = quote! { (#widget).radius(#radius) };
    }
    // 可选预览开关只配置运行时 Overlay 生命周期。
    if let Some(attribute) = find_attribute(element, "preview") {
        // 生成布尔字面量、简写或受限表达式。
        let preview = boolean_value(attribute)?;
        // 调用现有预览构建器。
        widget = quote! { (#widget).preview(#preview) };
    }
    // 可选 fit 开关只配置现有图片缩放策略。
    if let Some(attribute) = find_attribute(element, "fit") {
        // 生成布尔字面量、简写或受限表达式。
        let fit = boolean_value(attribute)?;
        // 调用现有缩放构建器。
        widget = quote! { (#widget).fit(#fit) };
    }
    // 可选 lazy 开关仍由运行时可见绘制路径触发加载。
    if let Some(attribute) = find_attribute(element, "lazy") {
        // 生成布尔字面量、简写或受限表达式。
        let lazy = boolean_value(attribute)?;
        // 调用现有延迟加载构建器。
        widget = quote! { (#widget).lazy(#lazy) };
    }

    // 物化为公开叶 View，再应用统一样式与自动化属性。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费 Image 专有属性，避免公共属性层重复解释固有尺寸。
    apply_common_attributes(
        // 传入已经配置的图片 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &[
            "src", "alt", "fallback", "width", "height", "radius", "preview", "fit", "lazy",
        ],
    )
}

// 生成正有限固有尺寸或受限 f32 表达式。
fn positive_dimension_value(
    // 接收待验证尺寸属性。
    attribute: &Attribute,
    // 接收用于诊断的尺寸名称。
    name: &str,
) -> Result<TokenStream, Diagnostic> {
    // 先复用统一有限 f32 与可选 px 解析。
    let value = numeric_value(attribute)?;
    // 动态表达式由 Rust 类型检查并交给运行时归一化。
    let AttributeValue::Literal(source) = &attribute.value else {
        // 返回已经生成的动态表达式。
        return Ok(value);
    };
    // 去除文档允许的 px 后缀以检查静态正值。
    let number = source.strip_suffix("px").unwrap_or(source);
    // 前置 numeric_value 已保证字面量可解析且有限。
    let parsed = number
        // 解析与运行时一致的 f32 尺寸。
        .parse::<f32>()
        // 解析失败表示共享数值校验契约被破坏。
        .expect("numeric_value 已验证 Image 尺寸");
    // 正值不会被运行时静默归一化为零。
    if parsed > 0.0 {
        // 返回共享数值令牌。
        return Ok(value);
    }
    // 拒绝零和负固有尺寸。
    Err(Diagnostic::new(
        // 指向非法尺寸属性。
        attribute.span,
        // 点名违反约束的属性。
        format!("Image {name} 必须大于 0"),
        // 给出合法静态与动态写法。
        "使用正的有限尺寸，例如 \"128px\" 或 f32 表达式",
    ))
}

// 生成非负有限圆角或受限 f32 表达式。
fn non_negative_radius_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 先复用统一有限 f32 与可选 px 解析。
    let value = numeric_value(attribute)?;
    // 动态表达式由 Rust 类型检查并交给运行时归一化。
    let AttributeValue::Literal(source) = &attribute.value else {
        // 返回已经生成的动态表达式。
        return Ok(value);
    };
    // 去除文档允许的 px 后缀以检查静态非负值。
    let number = source.strip_suffix("px").unwrap_or(source);
    // 前置 numeric_value 已保证字面量可解析且有限。
    let parsed = number
        // 解析与运行时一致的 f32 圆角。
        .parse::<f32>()
        // 解析失败表示共享数值校验契约被破坏。
        .expect("numeric_value 已验证 Image radius");
    // 零圆角与正圆角都能被运行时精确表达。
    if parsed >= 0.0 {
        // 返回共享数值令牌。
        return Ok(value);
    }
    // 拒绝会被运行时静默夹取的负圆角。
    Err(Diagnostic::new(
        // 指向非法 radius 属性。
        attribute.span,
        // 说明非负约束。
        "Image radius 不能为负数",
        // 给出合法静态与动态写法。
        "使用非负有限圆角，例如 \"8px\" 或 f32 表达式",
    ))
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

// 查找 Image 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 缺失属性时构造确定诊断。
    find_attribute(element, name).ok_or_else(|| {
        // 返回完整必需属性错误。
        Diagnostic::new(
            // 指向完整 Image 元素。
            element.span,
            // 点名缺失属性。
            format!("<Image> 缺少必需的 {name} 属性"),
            // 给出最小合法写法和 capability 边界。
            "启用 image-codecs，并使用 <Image src=\"assets/photo.png\" />",
        )
    })
}

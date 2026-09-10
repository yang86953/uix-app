// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与单节点生成入口。
use super::codegen::{apply_common_attributes, generate_node_view};
// 引入 Image 属性、节点、诊断与共享值生成契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, Node, boolean_value, numeric_value,
    string_value,
};

// 保存已经验证并生成的 Image 命名插槽。
struct ImageSlots {
    // 保存可选加载占位 View。
    placeholder: Option<TokenStream>,
    // 保存可选逐次错误 View 表达式。
    error: Option<TokenStream>,
}

// 生成图片来源、固有尺寸、运行时配置与命名状态 View。
pub(crate) fn generate_image(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 在配置运行时组件前验证并生成两个编译期命名插槽。
    let slots = generate_image_slots(element)?;

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
    let mut widget = quote! { ::uix_app::prelude::Image::new(#width, #height).src(&*(#src)) };
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
    // 可选占位插槽直接交给运行时 Image 保持加载生命周期所有权。
    if let Some(placeholder) = slots.placeholder {
        // 运行时只在尚未完成加载时挂载该稳定 View。
        widget = quote! { (#widget).placeholder(#placeholder) };
    }
    // 可选错误插槽通过工厂保证每次错误都构造新的 View。
    if let Some(error) = slots.error {
        // 第一版保留错误参数但不把它隐式注入 UIX 子树。
        widget = quote! { (#widget).on_error(move |_error| { #error }) };
    }

    // 经公开 View 契约进入 Image 自己的同目录 UIX 声明根。
    let view = quote! { ::uix_app::prelude::View::build(#widget) };
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

// 验证并生成 Image 的 placeholder 与 error 直接命名插槽。
fn generate_image_slots(element: &Element) -> Result<ImageSlots, Diagnostic> {
    // 初始化两个唯一插槽为空。
    let mut slots = ImageSlots {
        // 尚未发现加载占位。
        placeholder: None,
        // 尚未发现错误视图。
        error: None,
    };
    // 按源码顺序扫描 Image 的直接子节点。
    for node in &element.children {
        // 排版空白不构成默认插槽内容。
        if matches!(node, Node::Text(text) if text.value.trim().is_empty()) {
            // 忽略格式化空白。
            continue;
        }
        // 只有普通直接元素可以声明稳定的状态 View 身份。
        let Node::Element(child) = node else {
            // 拒绝裸文本和插值形成未声明默认插槽。
            return Err(Diagnostic::new(
                // 指向完整 Image 以覆盖直接内容。
                element.span,
                // 说明 Image 没有默认插槽。
                "<Image> 不接受裸文本、插值或默认插槽内容",
                // 给出显式状态 View 写法。
                "使用静态直接元素，并声明 slot=\"placeholder\" 或 slot=\"error\"",
            ));
        };
        // 直接控制流不能保证单一稳定状态 View 身份。
        if child.control.is_some()
            // 同时防御尚未附加控制绑定的控制标签。
            || matches!(child.name.as_str(), "If" | "ElseIf" | "Else" | "For")
        {
            // 返回动态直接插槽诊断。
            return Err(Diagnostic::new(
                // 指向非法控制元素。
                child.span,
                // 点明编译期静态直接元素约束。
                "<Image> 命名插槽必须是静态直接 View，不能是 If 或 For",
                // 允许在稳定容器内部继续使用控制流。
                "使用带 slot 属性的 Container 包裹 If 或 For",
            ));
        }
        // 克隆子元素以移除只负责插槽归位的属性。
        let mut child = child.clone();
        // 读取并移除必需的静态插槽名称。
        let slot = take_image_slot(&mut child)?;
        // 保留移除 slot 后仍对应同一源码元素的诊断跨度。
        let child_span = child.span;
        // 先通过目标元素的正常生成器验证完整子树。
        let view = generate_node_view(&Node::Element(child))?;
        // 按登记名称写入唯一目标。
        let target = match slot.as_str() {
            // 加载前内容进入 placeholder 构建器。
            "placeholder" => &mut slots.placeholder,
            // 加载失败内容进入 on_error 工厂。
            "error" => &mut slots.error,
            // 其他名称没有运行时归属，必须显式拒绝。
            _ => {
                // 返回未知命名插槽诊断。
                return Err(Diagnostic::new(
                    // 指向完整直接子元素。
                    child_span,
                    // 点名未知名称。
                    format!("<Image> 不支持名为 {slot} 的插槽"),
                    // 给出受支持的两个名称。
                    "使用 slot=\"placeholder\" 或 slot=\"error\"",
                ));
            }
        };
        // 每个状态只能有一个直接 View。
        if target.is_some() {
            // 返回重复插槽诊断。
            return Err(Diagnostic::new(
                // 指向后出现的重复子元素。
                child_span,
                // 点名重复目标。
                format!("<Image> 的 {slot} 插槽重复声明"),
                // 给出唯一性修复动作。
                "每个命名插槽只保留一个静态直接 View",
            ));
        }
        // 保存已经生成的状态 View。
        *target = Some(view);
    }
    // 返回两个可选命名插槽。
    Ok(slots)
}

// 读取并移除 Image 直接子元素的必需 slot 属性。
fn take_image_slot(element: &mut Element) -> Result<String, Diagnostic> {
    // 保存唯一 slot 属性的索引与字面量。
    let mut found = None;
    // 扫描直接子元素全部属性。
    for (index, attribute) in element.attributes.iter().enumerate() {
        // 其他属性交给子元素自己的生成器。
        if attribute.name != "slot" {
            // 继续扫描可能位于后方的 slot。
            continue;
        }
        // 同一子元素不能重复声明归位目标。
        if found.is_some() {
            // 返回重复属性诊断。
            return Err(Diagnostic::new(
                // 指向后出现的重复属性。
                attribute.span,
                // 说明归位目标必须唯一。
                "Image 子节点 slot 属性重复声明",
                // 给出唯一属性写法。
                "只保留一个 slot=\"placeholder\" 或 slot=\"error\" 属性",
            ));
        }
        // 插槽名称必须在编译期确定。
        let AttributeValue::Literal(value) = &attribute.value else {
            // 返回动态名称诊断。
            return Err(Diagnostic::new(
                // 指向非法 slot 属性。
                attribute.span,
                // 说明不接受表达式或简写。
                "Image 子节点 slot 属性必须是字符串字面量",
                // 给出合法静态名称。
                "使用 slot=\"placeholder\" 或 slot=\"error\"",
            ));
        };
        // 空名称不能伪装成默认插槽。
        if value.trim().is_empty() {
            // 返回空名称诊断。
            return Err(Diagnostic::new(
                // 指向空 slot 属性。
                attribute.span,
                // 说明 Image 不提供默认插槽。
                "Image 子节点 slot 名称不能为空",
                // 给出合法静态名称。
                "使用 slot=\"placeholder\" 或 slot=\"error\"",
            ));
        }
        // 保存待移除属性与名称。
        found = Some((index, value.clone()));
    }
    // Image 的每个非空直接子元素都必须显式归位。
    let Some((index, slot)) = found else {
        // 返回缺少命名插槽诊断。
        return Err(Diagnostic::new(
            // 指向未归位直接元素。
            element.span,
            // 说明不存在默认插槽。
            "<Image> 直接子 View 必须声明命名 slot",
            // 给出两个受支持目标。
            "添加 slot=\"placeholder\" 或 slot=\"error\"",
        ));
    };
    // 删除编译期归位属性，避免泄漏到子元素公共属性映射。
    element.attributes.remove(index);
    // 返回静态插槽名称。
    Ok(slot)
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

// 查找 Image 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "Image",
        "启用 image-codecs，并使用 <Image src=\"assets/photo.png\" />",
    )
}

// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与有序文本生成入口。
use super::codegen::{apply_common_attributes, generate_text_content};
// 引入 Tag 属性、表达式、布尔值与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, boolean_value, generate_expression,
    literal_string,
};

// 生成只投影初始外观与交互能力的 Tag 节点。
pub(crate) fn generate_tag(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 标签正文复用核心有序文本与插值契约，并拒绝嵌套元素。
    let content = generate_text_content(&element.children, element.span)?;
    // 从公开文本构造器开始配置。
    let mut widget = quote! { ::uix_app::prelude::Tag::new(#content) };
    // 可选颜色映射到公开预设枚举或自定义 Color。
    if let Some(attribute) = find_attribute(element, "color") {
        // 生成颜色配置后的 Tag。
        widget = apply_color(widget, attribute)?;
    }
    // 可选语义色绑定公开 TagColor 表达式；与 color 的具体色互斥。
    if let Some(attribute) = find_attribute(element, "semanticColor") {
        // 语义色必须是受限表达式，类型由 Rust 检查为公开 TagColor。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回语义色形状诊断。
            return Err(Diagnostic::new(
                // 指向非法 semanticColor 属性。
                attribute.span,
                // 说明公开运行时绑定类型。
                "Tag semanticColor 必须是 TagColor 表达式",
                // 给出规范绑定写法。
                "使用 semanticColor={status_color}",
            ));
        };
        if find_attribute(element, "color").is_some() {
            // 双色来源会造成未定义优先级。
            return Err(Diagnostic::new(
                // 指向 semanticColor 属性。
                attribute.span,
                // 说明互斥原因。
                "Tag semanticColor 与 color 不能同时声明",
                // 给出单一颜色来源建议。
                "使用语义色表达式或具体颜色之一",
            ));
        }
        // 生成受限语义色表达式。
        let semantic = generate_expression(&expression.expression, None)?;
        // 调用公开预设颜色构建器。
        widget = quote! { (#widget).color(#semantic) };
    }
    // 可关闭能力只改变运行时初始配置，不接管关闭生命周期。
    if let Some(attribute) = find_attribute(element, "closable") {
        // 复用统一布尔属性诊断与动态表达式生成。
        let closable = boolean_value(attribute)?;
        // 静态 false 保留运行时默认值。
        if !matches!(&attribute.value, AttributeValue::Literal(value) if value == "false") {
            // 静态 true 直接启用公开关闭能力。
            if matches!(&attribute.value, AttributeValue::Literal(value) if value == "true") {
                // 调用无参数关闭能力构建器。
                widget = quote! { (#widget).closable() };
            } else {
                // 动态布尔值用同类型分支选择是否启用关闭能力。
                widget = quote! { if #closable { (#widget).closable() } else { #widget } };
            }
        }
    }
    // 可勾选能力接受静态或动态布尔值。
    if let Some(attribute) = find_attribute(element, "checkable") {
        // 生成公开布尔配置令牌。
        let checkable = boolean_value(attribute)?;
        // 调用公开可勾选构建器。
        widget = quote! { (#widget).checkable(#checkable) };
    }

    // 经公开 View 契约进入组件自己的同目录 UIX 声明壳。
    let view = quote! { ::uix_app::prelude::View::build(#widget) };
    // 消费 Tag 专有属性并返回公共 View 表达式。
    apply_common_attributes(
        // 传入已经配置的标签 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["color", "semanticColor", "closable", "checkable"],
    )
}

// 把颜色属性应用到现有 Tag 构建表达式。
fn apply_color(widget: TokenStream, attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 动态颜色必须由 Rust 类型系统验证为公开 Color。
    if let AttributeValue::Expression(expression) = &attribute.value {
        // 生成受限颜色表达式。
        let color = generate_expression(&expression.expression, None)?;
        // 调用公开自定义颜色构建器。
        return Ok(quote! { (#widget).custom_color(#color) });
    }
    // 静态颜色必须是可确定映射的字符串字面量。
    let color = literal_string(attribute, "Tag color")?;
    // 优先映射公开预设颜色枚举。
    let preset = match color.as_str() {
        // 映射默认标签色。
        "default" => Some(quote! { ::uix_app::prelude::TagColor::Default }),
        // 映射成功标签色。
        "success" => Some(quote! { ::uix_app::prelude::TagColor::Success }),
        // 映射信息标签色。
        "info" => Some(quote! { ::uix_app::prelude::TagColor::Info }),
        // 映射警告标签色。
        "warning" => Some(quote! { ::uix_app::prelude::TagColor::Warning }),
        // 映射错误标签色。
        "error" => Some(quote! { ::uix_app::prelude::TagColor::Error }),
        // 映射蓝色标签。
        "blue" => Some(quote! { ::uix_app::prelude::TagColor::Blue }),
        // 映射青色标签。
        "cyan" => Some(quote! { ::uix_app::prelude::TagColor::Cyan }),
        // 映射极客蓝标签。
        "geekblue" => Some(quote! { ::uix_app::prelude::TagColor::Geekblue }),
        // 映射紫色标签。
        "purple" => Some(quote! { ::uix_app::prelude::TagColor::Purple }),
        // 映射洋红标签。
        "magenta" => Some(quote! { ::uix_app::prelude::TagColor::Magenta }),
        // 映射红色标签。
        "red" => Some(quote! { ::uix_app::prelude::TagColor::Red }),
        // 映射橙色标签。
        "orange" => Some(quote! { ::uix_app::prelude::TagColor::Orange }),
        // 映射金色标签。
        "gold" => Some(quote! { ::uix_app::prelude::TagColor::Gold }),
        // 映射青柠标签。
        "lime" => Some(quote! { ::uix_app::prelude::TagColor::Lime }),
        // 映射绿色标签。
        "green" => Some(quote! { ::uix_app::prelude::TagColor::Green }),
        // 其余字面量进入十六进制颜色校验。
        _ => None,
    };
    // 预设关键字直接调用公开枚举构建器。
    if let Some(preset) = preset {
        // 返回应用预设颜色后的 Tag。
        return Ok(quote! { (#widget).color(#preset) });
    }
    // 自定义颜色只接受三、四、六或八位十六进制字面量。
    let Some(hex) = color.strip_prefix('#') else {
        // 返回未知颜色诊断。
        return Err(color_diagnostic(attribute, &color));
    };
    // 校验十六进制位数与字符集合。
    if !matches!(hex.len(), 3 | 4 | 6 | 8) || !hex.chars().all(|value| value.is_ascii_hexdigit()) {
        // 返回非法十六进制颜色诊断。
        return Err(color_diagnostic(attribute, &color));
    }
    // 生成公开自定义颜色值。
    let custom = quote! { ::uix_app::prelude::Color::hex(#color) };
    // 返回应用自定义颜色后的 Tag。
    Ok(quote! { (#widget).custom_color(#custom) })
}

// 构造 Tag 静态颜色的统一诊断。
fn color_diagnostic(attribute: &Attribute, color: &str) -> Diagnostic {
    // 返回包含合法关键字与十六进制格式的诊断。
    Diagnostic::new(
        // 指向非法 color 属性。
        attribute.span,
        // 说明未登记颜色值。
        format!("Tag color={color:?} 不受支持"),
        // 给出关键字、十六进制与动态 Color 三类入口。
        "使用已登记 TagColor 关键字、#rgb/#rgba/#rrggbb/#rrggbbaa 或 Color 表达式",
    )
}

// 查找元素上的具名属性。

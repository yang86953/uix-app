// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入结构化诊断与样式属性。
use super::{Diagnostic, StyleProperty};

// 把 position 文档关键字映射到公开布局模式。
pub(super) fn position_value(
    // 接收已经完成语法解析的属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 去除属性值两端空白以匹配闭合关键字。
    let source = property.value.source.trim();
    // 只接受运行时完整实现的五种模式。
    match source {
        // static 保持正常流并忽略四边值。
        "static" => Ok(quote! { ::uix_app::prelude::PositionMode::Static }),
        // relative 保留槽位并移动视觉子树。
        "relative" => Ok(quote! { ::uix_app::prelude::PositionMode::Relative }),
        // absolute 相对最近定位祖先脱流。
        "absolute" => Ok(quote! { ::uix_app::prelude::PositionMode::Absolute }),
        // fixed 相对根视口脱流并提升合成路径。
        "fixed" => Ok(quote! { ::uix_app::prelude::PositionMode::Fixed }),
        // sticky 保留槽位并停靠最近滚动视口。
        "sticky" => Ok(quote! { ::uix_app::prelude::PositionMode::Sticky }),
        // 其他值不得静默回退 static。
        _ => Err(Diagnostic::new(
            // 指向完整 position 值。
            property.value.span,
            // 说明闭合的支持边界。
            "position 只支持 static、relative、absolute、fixed 或 sticky",
            // 给出最常用的脱流写法。
            "使用 position: absolute",
        )),
    }
}

// 把单一四边差异属性应用到既有 View 表达式。
pub(super) fn apply_position_inset(
    // 接收已经应用其他结构声明的 View 表达式。
    view: TokenStream,
    // 接收 top/right/bottom/left 属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 解析 auto 或有限逻辑像素值。
    let value = position_inset_value(property)?;
    // 按边名和取值形状选择只修改单边的公开入口。
    match (property.name.as_str(), value) {
        // 显式顶边像素值。
        ("top", Some(value)) => Ok(quote! { (#view).top(#value) }),
        // 顶边 auto 只清除顶边。
        ("top", None) => Ok(quote! { (#view).top_auto() }),
        // 显式右边像素值。
        ("right", Some(value)) => Ok(quote! { (#view).right(#value) }),
        // 右边 auto 只清除右边。
        ("right", None) => Ok(quote! { (#view).right_auto() }),
        // 显式底边像素值。
        ("bottom", Some(value)) => Ok(quote! { (#view).bottom(#value) }),
        // 底边 auto 只清除底边。
        ("bottom", None) => Ok(quote! { (#view).bottom_auto() }),
        // 显式左边像素值。
        ("left", Some(value)) => Ok(quote! { (#view).left(#value) }),
        // 左边 auto 只清除左边。
        ("left", None) => Ok(quote! { (#view).left_auto() }),
        // 调用方只应交付四个已登记属性。
        _ => Err(Diagnostic::new(
            // 指向完整属性作为内部边界保护。
            property.span,
            // 说明属性不属于定位四边。
            "定位 inset 只支持 top、right、bottom 或 left",
            // 引导调用方使用已登记名称。
            "核对 UIX 样式属性参考中的定位属性名称",
        )),
    }
}

// 解析四边属性的 auto 或有限 px 值。
fn position_inset_value(property: &StyleProperty) -> Result<Option<f32>, Diagnostic> {
    // 去除值两端空白。
    let source = property.value.source.trim();
    // auto 清除当前边的显式声明。
    if source == "auto" {
        // 用 None 表示运行时 auto。
        return Ok(None);
    }
    // 定位首批契约只接受显式 px 单位。
    let Some(number) = source.strip_suffix("px") else {
        // 百分比、calc 与无单位值都返回确定诊断。
        return Err(position_inset_diagnostic(property));
    };
    // 解析有符号浮点数并拒绝空值。
    let value = number
        // 去除数值与单位间可选空白。
        .trim()
        // 使用 Rust 有限浮点解析器。
        .parse::<f32>()
        // 非数值返回统一属性诊断。
        .map_err(|_| position_inset_diagnostic(property))?;
    // NaN 与无穷值无法形成稳定布局几何。
    if !value.is_finite() {
        // 返回与其他非法长度一致的诊断。
        return Err(position_inset_diagnostic(property));
    }
    // 保存显式有限逻辑像素值。
    Ok(Some(value))
}

// 构造四边长度的统一诊断。
fn position_inset_diagnostic(property: &StyleProperty) -> Diagnostic {
    // 返回指向精确属性值的错误。
    Diagnostic::new(
        // 指向完整 inset 值。
        property.value.span,
        // 说明首批稳定语法边界。
        format!("{} 只支持 auto 或有限 px 长度", property.name),
        // 给出正负像素都适用的规范写法。
        format!("使用 {}: 10px 或 {}: auto", property.name, property.name),
    )
}

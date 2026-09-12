// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入结构化诊断与样式属性。
use super::{Diagnostic, StyleProperty};
// 引入保留单位长度的共享解析入口。
use super::style_length_codegen::{LengthRule, style_length_value};

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
//
// 支持 auto、有符号 px、百分比与有限 calc(px ± %)；百分比与 calc 在运行时
// 按定位包含块解析。
pub(super) fn apply_position_inset(
    // 接收已经应用其他结构声明的 View 表达式。
    view: TokenStream,
    // 接收 top/right/bottom/left 属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 复用尺寸约束同一套长度语法，但 inset 允许负值且不接受 none。
    let value = style_length_value(property, LengthRule::INSET).map_err(|error| {
        Diagnostic::new(
            property.value.span,
            format!(
                "{} 只支持 auto、有限 px、百分比或 calc(px ± %)：{}",
                property.name, error.message
            ),
            format!(
                "使用 {}: 10px、{}: 50%、{}: calc(50% - 10px) 或 {}: auto",
                property.name, property.name, property.name, property.name
            ),
        )
    })?;
    // auto 只清除当前边并保留其他定位元数据。
    let is_auto = property.value.source.trim() == "auto";
    // 按边名和取值形状选择只修改单边的公开入口。
    match (property.name.as_str(), is_auto) {
        ("top", false) => Ok(quote! { (#view).top(#value) }),
        ("top", true) => Ok(quote! { (#view).top_auto() }),
        ("right", false) => Ok(quote! { (#view).right(#value) }),
        ("right", true) => Ok(quote! { (#view).right_auto() }),
        ("bottom", false) => Ok(quote! { (#view).bottom(#value) }),
        ("bottom", true) => Ok(quote! { (#view).bottom_auto() }),
        ("left", false) => Ok(quote! { (#view).left(#value) }),
        ("left", true) => Ok(quote! { (#view).left_auto() }),
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

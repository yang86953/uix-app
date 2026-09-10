// 引入卫生标识符、数值字面量与生成令牌流。
use proc_macro2::{Ident, Literal, TokenStream};
// 引入确定性 Rust 令牌拼接宏。
use quote::quote;

// 引入结构化诊断与样式属性。
use super::{Diagnostic, StyleProperty};

// 把 lineHeight 倍率或像素值映射为显式 UI 行高字段更新。
pub(super) fn line_height_field(
    // 接收 map_style 闭包中的卫生 Style 标识符。
    style: &Ident,
    // 接收保留源码跨度的 lineHeight 属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 去除属性值外围空白后判断单位。
    let source = property.value.source.trim();
    // 百分比不属于当前文档登记的行高单位。
    if source.ends_with('%') {
        // 返回指向完整值的单位诊断。
        return Err(line_height_diagnostic(
            // 传入当前属性。
            property,
            // 说明百分比能力边界。
            "lineHeight 不支持百分比",
        ));
    }
    // 区分固定像素与无单位倍率。
    let (number_source, pixels) = if let Some(number) = source.strip_suffix("px") {
        // px 后缀映射为固定逻辑像素。
        (number.trim(), true)
    } else {
        // 没有后缀时按最终字号倍率解释。
        (source, false)
    };
    // 解析十进制或科学计数法浮点值。
    let value = number_source.parse::<f32>().map_err(|_| {
        // 未知关键字或单位使用统一修复建议。
        line_height_diagnostic(property, "lineHeight 必须是无单位数值或 px 长度")
    })?;
    // 只允许正有限值进入运行时几何。
    if !value.is_finite() || value <= 0.0 {
        // 返回明确数值范围诊断。
        return Err(line_height_diagnostic(
            // 传入当前属性。
            property,
            // 说明零、负数与非有限值均不合法。
            "lineHeight 必须是正有限数值",
        ));
    }
    // 生成确定的 f32 字面量。
    let value = Literal::f32_unsuffixed(value);
    // 选择公开受控构造器，避免 UIX 生成私有单位类型。
    let line_height = if pixels {
        // 固定逻辑像素不随字号变化。
        quote! { ::uix_app::prelude::LineHeight::pixels(#value) }
    } else {
        // 无单位值在运行时按最终字号相乘。
        quote! { ::uix_app::prelude::LineHeight::factor(#value) }
    };
    // 宏期已经证明构造必定成功，生成显式字段声明。
    Ok(quote! {
        // 保存显式值以参与样式继承与布局失效。
        #style.line_height = ::std::option::Option::Some(
            // 运行时公开构造器再次守卫数值不变量。
            (#line_height).expect("UIX 已验证 lineHeight 为正有限数值")
        );
    })
}

// 构造 lineHeight 值级修复性诊断。
fn line_height_diagnostic(
    // 接收原始属性以定位值跨度。
    property: &StyleProperty,
    // 接收具体失败原因。
    message: impl Into<String>,
) -> Diagnostic {
    // 返回统一支持边界和可执行示例。
    Diagnostic::new(
        // 精确标记失败值。
        property.value.span,
        // 保留调用方具体原因。
        message,
        // 给出倍率与像素两种合法形态。
        "使用 lineHeight: 1.5 或 lineHeight: 24px",
    )
}

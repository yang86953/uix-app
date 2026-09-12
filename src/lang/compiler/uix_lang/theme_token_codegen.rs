// 引入过程宏标识符、字面量、跨度与令牌流。
use proc_macro2::{Ident, Literal, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入共享诊断、颜色解析与样式属性。
use super::{Diagnostic, StyleProperty, parse_color};

// 生成一个完整设计令牌字段的赋值语句。
pub(super) fn theme_token_update(
    // 接收已经完成样式语法解析的主题属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 按运行时 DesignTokens 的实际字段类型选择解析器。
    match property.name.as_str() {
        // 颜色令牌保持与运行时字段清单逐项对齐。
        "colorPrimary" => color_update(property, "color_primary"),
        "colorPrimaryHover" => color_update(property, "color_primary_hover"),
        "colorPrimaryActive" => color_update(property, "color_primary_active"),
        "colorPrimaryBg" => color_update(property, "color_primary_bg"),
        "colorPrimaryBorder" => color_update(property, "color_primary_border"),
        "colorBgContainer" => color_update(property, "color_bg_container"),
        "colorBgElevated" => color_update(property, "color_bg_elevated"),
        "colorBgRaised" => color_update(property, "color_bg_raised"),
        "colorBgOverlay" => color_update(property, "color_bg_overlay"),
        "colorBgLayout" => color_update(property, "color_bg_layout"),
        "colorBgSpotlight" => color_update(property, "color_bg_spotlight"),
        "colorBgMask" => color_update(property, "color_bg_mask"),
        "colorBorder" => color_update(property, "color_border"),
        "colorBorderSecondary" => color_update(property, "color_border_secondary"),
        "colorFill" => color_update(property, "color_fill"),
        "colorFillSecondary" => color_update(property, "color_fill_secondary"),
        "colorFillTertiary" => color_update(property, "color_fill_tertiary"),
        "colorFillQuaternary" => color_update(property, "color_fill_quaternary"),
        "colorText" => color_update(property, "color_text"),
        "colorTextSecondary" => color_update(property, "color_text_secondary"),
        "colorTextTertiary" => color_update(property, "color_text_tertiary"),
        "colorTextQuaternary" => color_update(property, "color_text_quaternary"),
        "colorWhite" => color_update(property, "color_white"),
        "colorBlack" => color_update(property, "color_black"),
        "colorShadow" => color_update(property, "color_shadow"),
        "colorShadowSecondary" => color_update(property, "color_shadow_secondary"),
        "colorSuccess" => color_update(property, "color_success"),
        "colorSuccessBg" => color_update(property, "color_success_bg"),
        "colorSuccessBorder" => color_update(property, "color_success_border"),
        "colorWarning" => color_update(property, "color_warning"),
        "colorWarningBg" => color_update(property, "color_warning_bg"),
        "colorWarningBorder" => color_update(property, "color_warning_border"),
        "colorError" => color_update(property, "color_error"),
        "colorErrorBg" => color_update(property, "color_error_bg"),
        "colorErrorBorder" => color_update(property, "color_error_border"),
        "colorInfo" => color_update(property, "color_info"),
        "colorInfoBg" => color_update(property, "color_info_bg"),
        "colorInfoBorder" => color_update(property, "color_info_border"),
        "colorLink" => color_update(property, "color_link"),
        "colorLinkHover" => color_update(property, "color_link_hover"),
        "colorLinkActive" => color_update(property, "color_link_active"),
        // 静态字符串令牌只接受拥有静态生命周期的字面文本。
        "fontFamily" => string_update(property, "font_family"),
        "motionEasingDefault" => string_update(property, "motion_easing_default"),
        "motionEasingIn" => string_update(property, "motion_easing_in"),
        "motionEasingOut" => string_update(property, "motion_easing_out"),
        "motionEasingInOut" => string_update(property, "motion_easing_in_out"),
        // 数值令牌接受有限 f32，并为尺寸类值兼容 px 后缀。
        "fontSizeSM" => number_update(property, "font_size_sm", true),
        "fontSize" => number_update(property, "font_size", true),
        "fontSizeLG" => number_update(property, "font_size_lg", true),
        "fontSizeXL" => number_update(property, "font_size_xl", true),
        "fontSizeHeading1" => number_update(property, "font_size_heading_1", true),
        "fontSizeHeading2" => number_update(property, "font_size_heading_2", true),
        "fontSizeHeading3" => number_update(property, "font_size_heading_3", true),
        "fontSizeHeading4" => number_update(property, "font_size_heading_4", true),
        "fontSizeHeading5" => number_update(property, "font_size_heading_5", true),
        "fontWeightRegular" => number_update(property, "font_weight_regular", false),
        "fontWeightMedium" => number_update(property, "font_weight_medium", false),
        "fontWeightSemibold" => number_update(property, "font_weight_semibold", false),
        "fontWeightBold" => number_update(property, "font_weight_bold", false),
        "lineHeight" => number_update(property, "line_height", false),
        "paddingXXS" => number_update(property, "padding_xss", true),
        "paddingXS" => number_update(property, "padding_xs", true),
        "paddingSM" => number_update(property, "padding_sm", true),
        "padding" => number_update(property, "padding", true),
        "paddingMD" => number_update(property, "padding_md", true),
        "paddingLG" => number_update(property, "padding_lg", true),
        "paddingXL" => number_update(property, "padding_xl", true),
        "borderRadius" => number_update(property, "border_radius", true),
        "borderRadiusSM" => number_update(property, "border_radius_sm", true),
        "borderRadiusLG" => number_update(property, "border_radius_lg", true),
        "borderRadiusXL" => number_update(property, "border_radius_xl", true),
        "borderRadiusRound" => number_update(property, "border_radius_round", true),
        "controlHeightSM" => number_update(property, "control_height_sm", true),
        "controlHeight" => number_update(property, "control_height", true),
        "controlHeightLG" => number_update(property, "control_height_lg", true),
        "backdropBlurRadius" => number_update(property, "backdrop_blur_radius", true),
        "motionDurationFast" => number_update(property, "motion_duration_fast", false),
        "motionDurationMid" => number_update(property, "motion_duration_mid", false),
        "motionDurationSlow" => number_update(property, "motion_duration_slow", false),
        "screenXS" => number_update(property, "screen_xs", true),
        "screenSM" => number_update(property, "screen_sm", true),
        "screenMD" => number_update(property, "screen_md", true),
        "screenLG" => number_update(property, "screen_lg", true),
        "screenXL" => number_update(property, "screen_xl", true),
        "screenXXL" => number_update(property, "screen_xxl", true),
        // 结构化阴影要求精确三层，从而完整构造既有 ShadowToken。
        "boxShadow" => shadow_update(property, "box_shadow"),
        "boxShadowSecondary" => shadow_update(property, "box_shadow_secondary"),
        // 明暗事实使用严格布尔值。
        "isDark" => boolean_update(property, "is_dark"),
        // 未登记名称必须在宏展开期失败。
        name => Err(theme_diagnostic(
            property,
            format!("主题 token {name} 尚未登记"),
            "使用 DesignTokens 已公开的 camelCase token 名",
        )),
    }
}

// 解析颜色 token 并生成字段赋值。
fn color_update(property: &StyleProperty, field: &str) -> Result<TokenStream, Diagnostic> {
    // 复用样式颜色解析以保持颜色语法一致。
    let (red, green, blue, alpha) = parse_color(&property.value.source, property)?;
    // 构造运行时字段标识符。
    let field = Ident::new(field, Span::call_site());
    // 返回精确颜色覆盖。
    Ok(quote! {
        __uix_tokens.#field = Some(::uix_app::prelude::Color::from_rgba(#red, #green, #blue, #alpha));
    })
}

// 解析有限数值 token 并生成字段赋值。
fn number_update(
    property: &StyleProperty,
    field: &str,
    allow_px: bool,
) -> Result<TokenStream, Diagnostic> {
    // 尺寸令牌允许省略或携带 px 单位。
    let source = if allow_px {
        property
            .value
            .source
            .strip_suffix("px")
            .unwrap_or(&property.value.source)
    } else {
        property.value.source.as_str()
    };
    // 解析有限 f32。
    let value = parse_f32(source, property)?;
    // 尺寸、时间和权重 token 均拒绝负值。
    if value < 0.0 {
        // 返回范围诊断。
        return Err(theme_diagnostic(
            property,
            "主题数值 token 不能为负数",
            "使用大于或等于 0 的有限数值",
        ));
    }
    // 构造运行时字段标识符。
    let field = Ident::new(field, Span::call_site());
    // 构造无后缀 f32 字面量。
    let value = Literal::f32_unsuffixed(value);
    // 返回精确数值覆盖。
    Ok(quote! { __uix_tokens.#field = Some(#value); })
}

// 解析静态字符串 token 并生成字段赋值。
fn string_update(property: &StyleProperty, field: &str) -> Result<TokenStream, Diagnostic> {
    // 去除可选的单引号或双引号外围。
    let source = unquote(&property.value.source).ok_or_else(|| {
        // 返回字符串形状诊断。
        theme_diagnostic(
            property,
            "主题字符串 token 必须使用引号包裹",
            "使用 fontFamily: 'Segoe UI'; 这类静态字符串",
        )
    })?;
    // 构造运行时字段标识符。
    let field = Ident::new(field, Span::call_site());
    // 返回静态字符串覆盖。
    Ok(quote! { __uix_tokens.#field = Some(#source); })
}

// 解析严格布尔 token 并生成字段赋值。
fn boolean_update(property: &StyleProperty, field: &str) -> Result<TokenStream, Diagnostic> {
    // 只接受两个规范布尔字面量。
    let value = match property.value.source.as_str() {
        // 映射真值。
        "true" => true,
        // 映射假值。
        "false" => false,
        // 拒绝其他文本。
        _ => {
            return Err(theme_diagnostic(
                property,
                "主题布尔 token 只接受 true 或 false",
                "使用 isDark: true; 或 isDark: false;",
            ));
        }
    };
    // 构造运行时字段标识符。
    let field = Ident::new(field, Span::call_site());
    // 返回精确布尔覆盖。
    Ok(quote! { __uix_tokens.#field = Some(#value); })
}

// 解析三层阴影 token 并生成字段赋值。
fn shadow_update(property: &StyleProperty, field: &str) -> Result<TokenStream, Diagnostic> {
    // 在函数括号外按逗号切分三层阴影。
    let layers = split_shadow_layers(&property.value.source);
    // 运行时 ShadowToken 固定拥有三层。
    if layers.len() != 3 {
        // 返回层数诊断。
        return Err(theme_diagnostic(
            property,
            "阴影 token 必须声明三层",
            "使用 x y blur color, x y blur color, x y blur color",
        ));
    }
    // 逐层解析结构化元组。
    let parsed = layers
        .iter()
        .map(|layer| parse_shadow_layer(layer, property))
        .collect::<Result<Vec<_>, _>>()?;
    // 构造运行时字段标识符。
    let field = Ident::new(field, Span::call_site());
    // 取得三层确定令牌。
    let layer_1 = &parsed[0];
    let layer_2 = &parsed[1];
    let layer_3 = &parsed[2];
    // 返回完整 ShadowToken 覆盖。
    Ok(quote! {
        __uix_tokens.#field = Some(::uix_app::prelude::ShadowToken {
            layer_1: #layer_1,
            layer_2: #layer_2,
            layer_3: #layer_3,
        });
    })
}

// 在圆括号外切分阴影层。
fn split_shadow_layers(source: &str) -> Vec<String> {
    // 保存当前圆括号深度。
    let mut depth = 0usize;
    // 保存当前层起点。
    let mut start = 0usize;
    // 保存有序层文本。
    let mut layers = Vec::new();
    // 遍历字符边界。
    for (index, value) in source.char_indices() {
        // 左括号进入颜色函数。
        if value == '(' {
            depth += 1;
        // 右括号退出颜色函数。
        } else if value == ')' {
            depth = depth.saturating_sub(1);
        // 顶层逗号结束当前阴影层。
        } else if value == ',' && depth == 0 {
            layers.push(source[start..index].trim().to_string());
            start = index + value.len_utf8();
        }
    }
    // 保存最后一层。
    layers.push(source[start..].trim().to_string());
    // 返回有序层列表。
    layers
}

// 解析一个 x y blur color 阴影层。
fn parse_shadow_layer(source: &str, property: &StyleProperty) -> Result<TokenStream, Diagnostic> {
    // 前三项为数值，其余文本共同构成颜色。
    let mut parts = source
        .splitn(4, char::is_whitespace)
        .filter(|part| !part.is_empty());
    // 读取水平偏移。
    let x = shadow_number(parts.next(), property, true)?;
    // 读取垂直偏移。
    let y = shadow_number(parts.next(), property, true)?;
    // 读取非负模糊半径。
    let blur = shadow_number(parts.next(), property, false)?;
    // 读取颜色文本。
    let color = parts.next().ok_or_else(|| {
        // 返回缺少颜色诊断。
        theme_diagnostic(property, "阴影层缺少颜色", "使用 0 2px 8px rgba(0,0,0,0.1)")
    })?;
    // 解析颜色通道。
    let (red, green, blue, alpha) = parse_color(color, property)?;
    // 返回运行时阴影层元组。
    Ok(quote! {
        (#x, #y, #blur, ::uix_app::prelude::Color::from_rgba(#red, #green, #blue, #alpha))
    })
}

// 解析阴影长度数值。
fn shadow_number(
    source: Option<&str>,
    property: &StyleProperty,
    signed: bool,
) -> Result<Literal, Diagnostic> {
    // 缺少分量时返回结构诊断。
    let source = source.ok_or_else(|| {
        // 给出完整阴影形状。
        theme_diagnostic(
            property,
            "阴影层缺少长度分量",
            "使用 0 2px 8px rgba(0,0,0,0.1)",
        )
    })?;
    // 去除可选 px 后缀。
    let source = source.strip_suffix("px").unwrap_or(source);
    // 解析有限数值。
    let value = parse_f32(source, property)?;
    // 模糊半径不能为负数。
    if !signed && value < 0.0 {
        // 返回模糊范围诊断。
        return Err(theme_diagnostic(
            property,
            "阴影模糊半径不能为负数",
            "使用非负 blur 值",
        ));
    }
    // 返回无后缀 f32 字面量。
    Ok(Literal::f32_unsuffixed(value))
}

// 解析有限 f32。
fn parse_f32(source: &str, property: &StyleProperty) -> Result<f32, Diagnostic> {
    // 把十进制文本解析为 f32。
    let value = source.parse::<f32>().map_err(|_| {
        // 返回数值语法诊断。
        theme_diagnostic(
            property,
            format!("主题数值 {source:?} 无法映射为 f32"),
            "使用有限十进制数",
        )
    })?;
    // 拒绝无穷和非数值。
    if !value.is_finite() {
        // 返回有限性诊断。
        return Err(theme_diagnostic(
            property,
            "主题数值必须有限",
            "使用有限十进制数",
        ));
    }
    // 返回已验证数值。
    Ok(value)
}

// 去除成对的单引号或双引号。
fn unquote(source: &str) -> Option<&str> {
    // 单引号字符串使用样式词法的原生形态。
    if source.len() >= 2 && source.starts_with('\'') && source.ends_with('\'') {
        // 返回内部文本。
        return Some(&source[1..source.len() - 1]);
    }
    // 双引号作为主题静态字符串兼容入口。
    if source.len() >= 2 && source.starts_with('"') && source.ends_with('"') {
        // 返回内部文本。
        return Some(&source[1..source.len() - 1]);
    }
    // 未加引号不是静态字符串。
    None
}

// 生成颜色样式值中的主题 token 引用。
pub(super) fn color_token_reference(
    source: &str,
    property: &StyleProperty,
) -> Result<Option<TokenStream>, Diagnostic> {
    // 非主题引用交回普通颜色解析。
    let Some(name) = theme_reference_name(source) else {
        // 报告未消费。
        return Ok(None);
    };
    // 已有 ColorValue 语义枚举优先保留每帧主题解析能力。
    if let Some(value) = semantic_color_reference(name) {
        // 返回既有运行时语义色引用。
        return Ok(Some(value));
    }
    // 颜色 token 通过现有主题提供者方法读取，不增加运行时枚举。
    let method =
        color_method(name).ok_or_else(|| incompatible_reference(property, name, "颜色"))?;
    // 构造主题方法标识符。
    let method = Ident::new(method, Span::call_site());
    // 返回构建期有效主题（局部 Provider 优先，其次窗口主题）中的色值。
    Ok(Some(quote! {{
        let __uix_tokens = ::uix_app::ui::__private::uix_effective_build_tokens();
        ::uix_app::prelude::ColorValue::custom(__uix_tokens.#method())
    }}))
}

// 把已有运行时色值角色覆盖的 token 映射为语义引用。
fn semantic_color_reference(name: &str) -> Option<TokenStream> {
    // 品牌与功能色使用既有 PaletteColor。
    let palette = match name {
        "primaryColor" | "colorPrimary" => Some("Primary"),
        "colorPrimaryHover" => Some("PrimaryHover"),
        "colorPrimaryActive" => Some("PrimaryActive"),
        "colorPrimaryBg" => Some("PrimaryBg"),
        "colorPrimaryBorder" => Some("PrimaryBorder"),
        "colorSuccess" => Some("Success"),
        "colorSuccessBg" => Some("SuccessBg"),
        "colorSuccessBorder" => Some("SuccessBorder"),
        "colorWarning" => Some("Warning"),
        "colorWarningBg" => Some("WarningBg"),
        "colorWarningBorder" => Some("WarningBorder"),
        "colorError" => Some("Error"),
        "colorErrorBg" => Some("ErrorBg"),
        "colorErrorBorder" => Some("ErrorBorder"),
        "colorInfo" => Some("Info"),
        "colorInfoBg" => Some("InfoBg"),
        "colorInfoBorder" => Some("InfoBorder"),
        "colorLink" => Some("Link"),
        "colorLinkHover" => Some("LinkHover"),
        "colorLinkActive" => Some("LinkActive"),
        "colorWhite" => Some("White"),
        "colorBlack" => Some("Black"),
        _ => None,
    };
    // 命中品牌或功能色时生成公开枚举。
    if let Some(palette) = palette {
        // 构造枚举变体标识符。
        let palette = Ident::new(palette, Span::call_site());
        // 返回语义色值。
        return Some(quote! {
            ::uix_app::prelude::ColorValue::palette(::uix_app::prelude::PaletteColor::#palette)
        });
    }
    // 中性色使用既有 NeutralRole。
    let neutral = match name {
        "colorText" => Some("Text"),
        "colorTextSecondary" => Some("TextSecondary"),
        "colorTextTertiary" => Some("TextTertiary"),
        "colorTextQuaternary" => Some("TextQuaternary"),
        "colorBorder" => Some("Border"),
        "colorBorderSecondary" => Some("BorderSecondary"),
        "colorFill" => Some("Fill"),
        "colorFillSecondary" => Some("FillSecondary"),
        "colorFillTertiary" => Some("FillTertiary"),
        "colorFillQuaternary" => Some("FillQuaternary"),
        "colorBgContainer" => Some("BgContainer"),
        "colorBgElevated" => Some("BgElevated"),
        "backgroundColor" | "colorBgLayout" => Some("BgLayout"),
        "colorBgMask" => Some("BgMask"),
        _ => None,
    }?;
    // 构造中性色角色标识符。
    let neutral = Ident::new(neutral, Span::call_site());
    // 返回中性语义色值。
    Some(quote! {
        ::uix_app::prelude::ColorValue::neutral(::uix_app::prelude::NeutralRole::#neutral)
    })
}

// 生成数值样式值中的主题 token 引用。
pub(super) fn number_token_reference(
    source: &str,
    property: &StyleProperty,
) -> Result<Option<TokenStream>, Diagnostic> {
    // 非主题引用交回普通数值解析。
    let Some(name) = theme_reference_name(source) else {
        // 报告未消费。
        return Ok(None);
    };
    // 数值 token 必须来自既有 f32 令牌方法。
    let method =
        number_method(name).ok_or_else(|| incompatible_reference(property, name, "数值"))?;
    // 构造主题方法标识符。
    let method = Ident::new(method, Span::call_site());
    // 返回构建期有效主题（局部 Provider 优先，其次窗口主题）中的数值。
    Ok(Some(quote! {{
        let __uix_tokens = ::uix_app::ui::__private::uix_effective_build_tokens();
        __uix_tokens.#method()
    }}))
}

// 识别以井号开头的主题名称。
fn theme_reference_name(source: &str) -> Option<&str> {
    // 去除井号前缀。
    let name = source.strip_prefix('#')?;
    // 十六进制颜色仍由普通颜色解析器负责。
    if name.chars().all(|value| value.is_ascii_hexdigit()) {
        // 报告非主题引用。
        return None;
    }
    // 返回语义 token 名称。
    Some(name)
}

// 把公开颜色 token 名映射到既有 ThemeTokens 方法。
fn color_method(name: &str) -> Option<&'static str> {
    // 颜色全集与两个兼容别名使用闭合映射。
    Some(match name {
        "primaryColor" | "colorPrimary" => "color_primary",
        "backgroundColor" | "colorBgLayout" => "color_bg_layout",
        "colorPrimaryHover" => "color_primary_hover",
        "colorPrimaryActive" => "color_primary_active",
        "colorPrimaryBg" => "color_primary_bg",
        "colorPrimaryBorder" => "color_primary_border",
        "colorBgContainer" => "color_bg_container",
        "colorBgElevated" => "color_bg_elevated",
        "colorBgRaised" => "color_bg_raised",
        "colorBgOverlay" => "color_bg_overlay",
        "colorBgSpotlight" => "color_bg_spotlight",
        "colorBgMask" => "color_bg_mask",
        "colorBorder" => "color_border",
        "colorBorderSecondary" => "color_border_secondary",
        "colorFill" => "color_fill",
        "colorFillSecondary" => "color_fill_secondary",
        "colorFillTertiary" => "color_fill_tertiary",
        "colorFillQuaternary" => "color_fill_quaternary",
        "colorText" => "color_text",
        "colorTextSecondary" => "color_text_secondary",
        "colorTextTertiary" => "color_text_tertiary",
        "colorTextQuaternary" => "color_text_quaternary",
        "colorWhite" => "color_white",
        "colorBlack" => "color_black",
        "colorShadow" => "color_shadow",
        "colorShadowSecondary" => "color_shadow_secondary",
        "colorSuccess" => "color_success",
        "colorSuccessBg" => "color_success_bg",
        "colorSuccessBorder" => "color_success_border",
        "colorWarning" => "color_warning",
        "colorWarningBg" => "color_warning_bg",
        "colorWarningBorder" => "color_warning_border",
        "colorError" => "color_error",
        "colorErrorBg" => "color_error_bg",
        "colorErrorBorder" => "color_error_border",
        "colorInfo" => "color_info",
        "colorInfoBg" => "color_info_bg",
        "colorInfoBorder" => "color_info_border",
        "colorLink" => "color_link",
        "colorLinkHover" => "color_link_hover",
        "colorLinkActive" => "color_link_active",
        _ => return None,
    })
}

// 把公开数值 token 名映射到既有 ThemeTokens 方法。
fn number_method(name: &str) -> Option<&'static str> {
    // 数值全集保持与运行时 trait 实际方法一致。
    Some(match name {
        "fontSizeSM" => "font_size_sm",
        "fontSize" => "font_size",
        "fontSizeLG" => "font_size_lg",
        "fontSizeXL" => "font_size_xl",
        "fontSizeHeading1" => "font_size_heading_1",
        "fontSizeHeading2" => "font_size_heading_2",
        "fontSizeHeading3" => "font_size_heading_3",
        "fontSizeHeading4" => "font_size_heading_4",
        "fontSizeHeading5" => "font_size_heading_5",
        "fontWeightRegular" => "font_weight_regular",
        "fontWeightMedium" => "font_weight_medium",
        "fontWeightSemibold" => "font_weight_semibold",
        "fontWeightBold" => "font_weight_bold",
        "lineHeight" => "line_height",
        "paddingXXS" => "padding_xss",
        "paddingXS" => "padding_xs",
        "paddingSM" => "padding_sm",
        "padding" => "padding",
        "paddingMD" => "padding_md",
        "paddingLG" => "padding_lg",
        "paddingXL" => "padding_xl",
        "borderRadius" => "border_radius",
        "borderRadiusSM" => "border_radius_sm",
        "borderRadiusLG" => "border_radius_lg",
        "borderRadiusXL" => "border_radius_xl",
        "borderRadiusRound" => "border_radius_round",
        "controlHeightSM" => "control_height_sm",
        "controlHeight" => "control_height",
        "controlHeightLG" => "control_height_lg",
        "backdropBlurRadius" => "backdrop_blur_radius",
        "motionDurationFast" => "motion_duration_fast",
        "motionDurationMid" => "motion_duration_mid",
        "motionDurationSlow" => "motion_duration_slow",
        "screenXS" => "screen_xs",
        "screenSM" => "screen_sm",
        "screenMD" => "screen_md",
        "screenLG" => "screen_lg",
        "screenXL" => "screen_xl",
        "screenXXL" => "screen_xxl",
        _ => return None,
    })
}

// 构造 token 类型不兼容或未知名称诊断。
fn incompatible_reference(property: &StyleProperty, name: &str, expected: &str) -> Diagnostic {
    // 统一说明闭合 token 契约与当前样式类型。
    theme_diagnostic(
        property,
        format!("主题 token {name} 不是可用于此处的{expected} token"),
        format!("使用已登记的{expected}设计 token 名"),
    )
}

// 构造主题 token 诊断。
fn theme_diagnostic(
    property: &StyleProperty,
    message: impl Into<String>,
    suggestion: impl Into<String>,
) -> Diagnostic {
    // 复用完整属性跨度以精确回映 UIX 源码。
    Diagnostic::new(property.span, message, suggestion)
}

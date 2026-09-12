//! Token syntax is checked against library-owned declarations.
use super::{Diagnostic, StyleProperty, parse_color};
use crate::lang::compiler::{
    components,
    projection_schema::{ThemeTokenKind, UI_PROJECTION_SCHEMA},
};
use proc_macro2::{Literal, TokenStream};
use quote::quote;
pub(super) fn theme_token_update(property: &StyleProperty) -> Result<TokenStream, Diagnostic> {
    let spec = UI_PROJECTION_SCHEMA
        .theme_token(&property.name)
        .ok_or_else(|| incompatible_reference(property, &property.name, "已登记"))?;
    components::use_unit(&spec.unit);
    let key = &spec.key;
    let value = match spec.kind {
        ThemeTokenKind::Color => {
            let (r, g, b, a) = parse_color(&property.value.source, property)?;
            quote! {::uix_app::ui::TokenValue::Color(::uix_app::prelude::Color::from_rgba(#r,#g,#b,#a))}
        }
        ThemeTokenKind::Number => {
            let source = if spec.allow_px {
                property
                    .value
                    .source
                    .strip_suffix("px")
                    .unwrap_or(&property.value.source)
            } else {
                &property.value.source
            };
            let value = parse_f32(source, property)?;
            if spec.minimum.is_some_and(|min| value < min) {
                return Err(theme_diagnostic(
                    property,
                    "token below declared minimum",
                    "Use a value in the library's declared range",
                ));
            }
            let value = Literal::f32_unsuffixed(value);
            quote! {::uix_app::ui::TokenValue::Number(#value)}
        }
        ThemeTokenKind::String => {
            let value = unquote(&property.value.source).ok_or_else(|| {
                theme_diagnostic(property, "Expected quoted token text", "Quote the string")
            })?;
            quote! {::uix_app::ui::TokenValue::Text(#value)}
        }
        ThemeTokenKind::Boolean => {
            let value = match property.value.source.as_str() {
                "true" => true,
                "false" => false,
                _ => {
                    return Err(theme_diagnostic(
                        property,
                        "Expected boolean token",
                        "Use true or false",
                    ));
                }
            };
            quote! {::uix_app::ui::TokenValue::Boolean(#value)}
        }
        ThemeTokenKind::Shadow => {
            let layers = split_shadow_layers(&property.value.source);
            if spec.shadow_layers.is_some_and(|n| n != layers.len()) {
                return Err(theme_diagnostic(
                    property,
                    "Incorrect number of shadow layers",
                    "Follow the library token declaration",
                ));
            }
            let layers = layers
                .iter()
                .map(|layer| parse_shadow_layer(layer, property))
                .collect::<Result<Vec<_>, _>>()?;
            quote! {::uix_app::ui::TokenValue::Shadows(vec![#(#layers),*])}
        }
    };
    Ok(quote! {__uix_tokens.insert(#key,#value);})
}
pub(super) fn color_token_reference(
    source: &str,
    property: &StyleProperty,
) -> Result<Option<TokenStream>, Diagnostic> {
    let Some(name) = theme_reference_name(source) else {
        return Ok(None);
    };
    let spec = UI_PROJECTION_SCHEMA
        .theme_token(name)
        .filter(|s| s.kind == ThemeTokenKind::Color)
        .ok_or_else(|| incompatible_reference(property, name, "颜色"))?;
    components::use_unit(&spec.unit);
    let key = spec.key;
    Ok(Some(
        quote! {::uix_app::ui::ColorValue::token(#key,::uix_app::prelude::Color::transparent())},
    ))
}
pub(super) fn number_token_reference(
    source: &str,
    property: &StyleProperty,
) -> Result<Option<TokenStream>, Diagnostic> {
    let Some(name) = theme_reference_name(source) else {
        return Ok(None);
    };
    let spec = UI_PROJECTION_SCHEMA
        .theme_token(name)
        .filter(|s| s.kind == ThemeTokenKind::Number)
        .ok_or_else(|| incompatible_reference(property, name, "数值"))?;
    components::use_unit(&spec.unit);
    let key = spec.key;
    let fallback = spec.fallback;
    Ok(Some(
        quote! {::uix_app::ui::__private::uix_effective_build_tokens().number(#key,#fallback)},
    ))
}
pub(super) fn typography_reference(name: &str) -> Option<TokenStream> {
    let spec = UI_PROJECTION_SCHEMA
        .theme_tokens()
        .into_iter()
        .find(|s| s.typography.iter().any(|n| n == name))?;
    components::use_unit(&spec.unit);
    let key = spec.key;
    let fallback = spec.fallback;
    Some(quote! {::uix_app::ui::TypographyToken::Token(#key,#fallback)})
}
fn theme_reference_name(source: &str) -> Option<&str> {
    let name = source.strip_prefix('#')?;
    if name.chars().all(|c| c.is_ascii_hexdigit()) {
        None
    } else {
        Some(name)
    }
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
        ::uix_app::prelude::BoxShadowDef::new(::uix_app::prelude::Color::from_rgba(#red, #green, #blue, #alpha), #blur, #x, #y)
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

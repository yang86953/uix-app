// 引入过程宏标识符与令牌流。
use proc_macro2::{Ident, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入结构化诊断与样式属性。
use super::{Diagnostic, StyleProperty};
// 引入样式值的共享长度、颜色与诊断解析。
use super::style_value_codegen::{
    parse_color, parse_length, parse_length_signed, value_diagnostic,
};

// 生成 BoxShadowDef 字段更新。
pub(super) fn box_shadow_field(
    // 接收待更新样式参数。
    style: &Ident,
    // 接收结构化阴影属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 生成完整可选阴影值。
    let value = box_shadow_value(property)?;
    // 把解析结果写入样式字段。
    Ok(quote! { #style.box_shadow = #value; })
}

// 生成允许既有颜色名称的通用盒阴影值。
pub(super) fn box_shadow_value(
    // 接收结构化阴影属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 保持内联样式既有颜色兼容面。
    box_shadow_value_with_policy(property, false)
}

// 生成只接受具体颜色字面量的 Container 盒阴影值。
pub(super) fn concrete_box_shadow_value(
    // 接收由 Container 字面量构造的阴影属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 禁止在复合组件属性中隐式解析颜色名称或主题引用。
    box_shadow_value_with_policy(property, true)
}

// 按颜色策略生成完整可选盒阴影值。
fn box_shadow_value_with_policy(
    // 接收结构化阴影属性。
    property: &StyleProperty,
    // 标记是否只接受十六进制与 rgb/rgba 颜色。
    concrete_color: bool,
) -> Result<TokenStream, Diagnostic> {
    // none 明确清除阴影。
    if property.value.source == "none" {
        // 生成 None 值。
        return Ok(quote! { ::std::option::Option::None });
    }
    // 阴影规范需要前三个长度和其余颜色文本。
    let parts = property
        // 读取未改写的阴影源码。
        .value
        // 按任意连续空白形成语义分量。
        .source
        // 使用标准空白切分避免空分量。
        .split_whitespace()
        // 收集后可把颜色内部空白重新拼接。
        .collect::<Vec<_>>();
    // 读取水平偏移。
    let horizontal = parts.first().copied().ok_or_else(|| {
        // 返回缺失水平偏移诊断。
        value_diagnostic(
            property,
            "boxShadow 缺少水平偏移",
            "使用 0 2px 4px rgba(0,0,0,0.1)",
        )
    })?;
    // 读取垂直偏移。
    let vertical = parts.get(1).copied().ok_or_else(|| {
        // 返回缺失垂直偏移诊断。
        value_diagnostic(
            property,
            "boxShadow 缺少垂直偏移",
            "使用 0 2px 4px rgba(0,0,0,0.1)",
        )
    })?;
    // 读取模糊半径。
    let blur = parts.get(2).copied().ok_or_else(|| {
        // 返回缺失模糊半径诊断。
        value_diagnostic(
            property,
            "boxShadow 缺少模糊半径",
            "使用 0 2px 4px rgba(0,0,0,0.1)",
        )
    })?;
    // 读取并清理颜色外围空白。
    let color = parts
        // 读取前三个长度之后的全部颜色分量。
        .get(3..)
        // 排除没有颜色分量的空切片。
        .filter(|parts| !parts.is_empty())
        // 恢复 rgba 通道之间允许的空格。
        .map(|parts| parts.join(" "))
        // 缺失颜色时返回精确诊断。
        .ok_or_else(|| {
            // 返回缺失颜色诊断。
            value_diagnostic(
                property,
                "boxShadow 缺少颜色",
                "使用 0 2px 4px rgba(0,0,0,0.1)",
            )
        })?;
    // Container 复合属性只接受明确的具体颜色语法。
    if concrete_color && !is_concrete_color(&color) {
        // 返回不允许主题或名称推断的诊断。
        return Err(value_diagnostic(
            property,
            format!("Container shadow 颜色 {color:?} 不是具体颜色字面量"),
            "使用十六进制、rgb(...) 或 rgba(...) 颜色",
        ));
    }
    // 解析水平偏移。
    let horizontal = parse_length_signed(horizontal, property)?;
    // 解析垂直偏移。
    let vertical = parse_length_signed(vertical, property)?;
    // 解析非负模糊半径。
    let blur = parse_length(blur, property)?;
    // 解析阴影颜色通道。
    let (red, green, blue, alpha) = parse_color(&color, property)?;
    // 生成保留完整偏移的阴影定义。
    Ok(quote! {
        ::std::option::Option::Some(::uix::prelude::BoxShadowDef::new(
            ::uix::prelude::Color::from_rgba(#red, #green, #blue, #alpha),
            #blur,
            #horizontal,
            #vertical,
        ))
    })
}

// 判断颜色是否属于 Container 复合属性登记的具体语法。
fn is_concrete_color(source: &str) -> bool {
    // 只允许十六进制、rgb 与 rgba 字面量。
    source.starts_with('#') || source.starts_with("rgb(") || source.starts_with("rgba(")
}

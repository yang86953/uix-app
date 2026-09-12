// 引入过程宏字面量与令牌流。
use proc_macro2::{Literal, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入结构化诊断与样式属性。
use super::{Diagnostic, StyleProperty};
// 引入闭合设计 token 的数值引用生成入口。
use super::theme_token_codegen::number_token_reference;
// 引入统一样式值诊断构造。
use super::style_value_codegen::value_diagnostic;

// 保留单位长度的接受规则。
#[derive(Clone, Copy)]
pub(super) struct LengthRule {
    // 是否接受 `none` 关键字（min/max 的不约束写法）。
    pub(super) allow_none: bool,
    // 单值 px/百分比是否必须非负；calc 内部各项允许为负，结果由运行时钳制。
    pub(super) non_negative: bool,
    // 是否接受无单位数值并按 px 解释。
    pub(super) allow_unitless: bool,
}

impl LengthRule {
    // min/max 尺寸约束：auto/none 不约束，单值非负，无单位按 px（与 width/height 一致）。
    pub(super) const SIZE_BOUND: Self = Self {
        allow_none: true,
        non_negative: true,
        allow_unitless: true,
    };
    // 定位 inset：auto 不约束，允许有符号值，沿用既有契约要求显式单位。
    pub(super) const INSET: Self = Self {
        allow_none: false,
        non_negative: false,
        allow_unitless: false,
    };
}

// 把属性值解析为运行时 `StyleLength` 表达式。
//
// 支持集合：`auto`（以及规则允许时的 `none`）、`<n>px`、无单位数值（按 px）、
// `<n>%`、主题数值 token（按 px），以及 `calc(<项> ± <项> ...)`，其中每一项
// 为上述 px/百分比/token 形态。不接受乘除、嵌套函数或其他单位。
pub(super) fn style_length_value(
    property: &StyleProperty,
    rule: LengthRule,
) -> Result<TokenStream, Diagnostic> {
    let source = property.value.source.trim();
    if source == "auto" || (rule.allow_none && source == "none") {
        return Ok(quote! { ::uix_app::prelude::StyleLength::Auto });
    }
    if let Some(inner) = source
        .strip_prefix("calc(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return calc_value(inner, property, rule.allow_unitless);
    }
    let term = length_term(source, property, rule.allow_unitless)?;
    if rule.non_negative && term.negative_literal {
        return Err(value_diagnostic(
            property,
            format!("{} 不能为负数", property.name),
            "使用大于或等于 0 的 px 或百分比，或改用 calc() 表达差值",
        ));
    }
    Ok(match term.kind {
        TermKind::Px(px) => quote! { ::uix_app::prelude::StyleLength::Px(#px) },
        TermKind::Percent(percent) => quote! { ::uix_app::prelude::StyleLength::Percent(#percent) },
    })
}

// 已解析的单个长度项。
struct LengthTerm {
    // 项的单位类别与值表达式。
    kind: TermKind,
    // 字面量是否为负数（token 引用视为非负）。
    negative_literal: bool,
}

// 单个长度项的单位类别。
enum TermKind {
    // 逻辑像素表达式（字面量或 token 读取）。
    Px(TokenStream),
    // 百分比字面量。
    Percent(TokenStream),
}

// 解析一个 px、无单位、百分比或 token 项。
fn length_term(
    source: &str,
    property: &StyleProperty,
    allow_unitless: bool,
) -> Result<LengthTerm, Diagnostic> {
    if let Some(value) = number_token_reference(source, property)? {
        return Ok(LengthTerm {
            kind: TermKind::Px(value),
            negative_literal: false,
        });
    }
    if let Some(number) = source.strip_suffix('%') {
        let value = finite_number(number, property)?;
        return Ok(LengthTerm {
            kind: TermKind::Percent(literal(value)),
            negative_literal: value < 0.0,
        });
    }
    let number = match source.strip_suffix("px") {
        Some(number) => number,
        None if allow_unitless => source,
        None => {
            return Err(value_diagnostic(
                property,
                format!("长度 {source:?} 缺少 px 或 % 单位"),
                "使用如 10px、50% 的带单位长度",
            ));
        }
    };
    let value = finite_number(number, property)?;
    Ok(LengthTerm {
        kind: TermKind::Px(literal(value)),
        negative_literal: value < 0.0,
    })
}

// 解析 calc 内部的加减序列并折叠为 px 与百分比两部分。
fn calc_value(
    inner: &str,
    property: &StyleProperty,
    allow_unitless: bool,
) -> Result<TokenStream, Diagnostic> {
    let inner = inner.trim();
    if inner.is_empty() {
        return Err(calc_diagnostic(property, "calc() 不能为空"));
    }
    if inner.contains(['*', '/', '(', ')']) {
        return Err(calc_diagnostic(
            property,
            "calc() 只支持 px 与百分比的加减，不支持乘除、嵌套函数或其他表达式",
        ));
    }
    // 运算符必须两侧留空白，避免与负数字面量混淆。
    let mut px_literal = 0.0f32;
    let mut px_tokens: Vec<TokenStream> = Vec::new();
    let mut percent = 0.0f32;
    let mut sign = 1.0f32;
    let mut expect_operand = true;
    for piece in inner.split_whitespace() {
        if expect_operand {
            let term = length_term(piece, property, allow_unitless)?;
            match term.kind {
                TermKind::Px(tokens) => {
                    if let Some(value) = literal_value(piece, property) {
                        px_literal += sign * value;
                    } else if sign < 0.0 {
                        px_tokens.push(quote! { - (#tokens) });
                    } else {
                        px_tokens.push(quote! { + (#tokens) });
                    }
                }
                TermKind::Percent(_) => {
                    let value = finite_number(piece.strip_suffix('%').unwrap_or(piece), property)?;
                    percent += sign * value;
                }
            }
            expect_operand = false;
        } else {
            sign = match piece {
                "+" => 1.0,
                "-" => -1.0,
                _ => {
                    return Err(calc_diagnostic(
                        property,
                        "calc() 的项之间只能使用两侧带空格的 + 或 -",
                    ));
                }
            };
            expect_operand = true;
        }
    }
    if expect_operand {
        return Err(calc_diagnostic(
            property,
            "calc() 以运算符结尾，缺少最后一项",
        ));
    }
    if !px_literal.is_finite() || !percent.is_finite() {
        return Err(calc_diagnostic(property, "calc() 折叠结果必须有限"));
    }
    let px = if px_tokens.is_empty() {
        literal(px_literal)
    } else {
        let base = literal(px_literal);
        quote! { (#base #(#px_tokens)*) }
    };
    let percent_literal = literal(percent);
    Ok(quote! {
        ::uix_app::prelude::StyleLength::Calc { px: #px, percent: #percent_literal }
    })
}

// 把 calc 中的 px 字面量项折叠为常量；token 项返回 None。
fn literal_value(piece: &str, property: &StyleProperty) -> Option<f32> {
    if number_token_reference(piece, property)
        .ok()
        .flatten()
        .is_some()
    {
        return None;
    }
    let number = piece.strip_suffix("px").unwrap_or(piece);
    number.parse::<f32>().ok().filter(|value| value.is_finite())
}

// 解析有限十进制数值。
fn finite_number(source: &str, property: &StyleProperty) -> Result<f32, Diagnostic> {
    let value = source.trim().parse::<f32>().map_err(|_| {
        value_diagnostic(
            property,
            format!("长度 {source:?} 无法映射为 f32"),
            "使用有限十进制数、px 长度或百分比",
        )
    })?;
    if !value.is_finite() {
        return Err(value_diagnostic(
            property,
            "长度数值必须有限",
            "使用有限十进制数",
        ));
    }
    Ok(value)
}

// 构造无后缀 f32 字面量令牌。
fn literal(value: f32) -> TokenStream {
    let literal = Literal::f32_unsuffixed(value);
    quote! { #literal }
}

// 构造 calc 语法诊断。
fn calc_diagnostic(property: &StyleProperty, message: &str) -> Diagnostic {
    value_diagnostic(
        property,
        message,
        "使用如 calc(100% - 32px)、calc(50% + 8px) 的有限加减表达式",
    )
}

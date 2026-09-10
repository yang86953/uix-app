// 引入过程宏数值字面量与令牌流。
use proc_macro2::{Ident, Literal, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入结构化诊断与样式属性。
use super::{AnimationPropertyKind, Diagnostic, StyleHashKind, StyleProperty};
// 引入闭合设计 token 的类型化引用生成入口。
use super::theme_token_codegen::{color_token_reference, number_token_reference};

// 定义数值范围规则。
#[derive(Clone, Copy)]
pub(super) enum NumberRule {
    // 接受任意有限数值。
    Any,
    // 只接受非负有限数值。
    NonNegative,
    // 只接受零到一之间的有限数值。
    UnitInterval,
}

// 生成首批 animation 数值字段的 f32 值令牌。
pub(super) fn animation_f32_value(
    // 接收闭合动画字段类型。
    kind: AnimationPropertyKind,
    // 接收基础值或关键帧值。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 按字段范围规则复用现有样式数值解析。
    match kind {
        // 宽度只接受非 auto 的非负 px 或数值。
        AnimationPropertyKind::Width => animation_dimension_value(property),
        // 高度只接受非 auto 的非负 px 或数值。
        AnimationPropertyKind::Height => animation_dimension_value(property),
        // 圆角只接受非负数值。
        AnimationPropertyKind::BorderRadius => number_value(property, NumberRule::NonNegative),
        // 透明度只接受零到一闭区间。
        AnimationPropertyKind::Opacity => number_value(property, NumberRule::UnitInterval),
        // 颜色字段由专用生成器处理。
        AnimationPropertyKind::Color | AnimationPropertyKind::BackgroundColor => {
            // 返回内部字段类型错配诊断。
            Err(value_diagnostic(
                // 指向实际属性。
                property,
                // 陈述失败原因。
                "颜色关键帧不能按 f32 生成",
                // 指向内部映射修复。
                "使用 animation_color_value 生成颜色字段",
            ))
        }
    }
}

// 生成 animation 颜色字段的具体 Color 值令牌。
pub(super) fn animation_color_value(
    // 接收基础值或关键帧值。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 语义主题引用在 View 构建期没有 WidgetTree 最终 token 快照，不能伪装插值。
    if property
        // 遍历值中的哈希片段。
        .value
        .hashes
        // 借用迭代器。
        .iter()
        // 查找主题引用。
        .any(|hash| matches!(hash.kind, StyleHashKind::ThemeReference(_)))
    {
        // 返回主题颜色动画诊断。
        return Err(value_diagnostic(
            // 指向完整颜色属性。
            property,
            // 陈述失败原因。
            "关键帧颜色暂不支持主题 token 引用",
            // 给出可插值写法。
            "在 @keyframes 与对应基础样式中使用十六进制、rgb 或 rgba 具体颜色",
        ));
    }
    // 复用完整颜色语法解析 RGBA 通道。
    let (red, green, blue, alpha) = parse_color(&property.value.source, property)?;
    // 生成可直接由 Animated<Color> 插值的具体颜色。
    Ok(quote! {
        ::uix_app::prelude::Color::from_rgba(#red, #green, #blue, #alpha)
    })
}

// 解析 animation 中不允许 auto 或百分比的固定尺寸。
fn animation_dimension_value(property: &StyleProperty) -> Result<TokenStream, Diagnostic> {
    // auto 没有可插值的确定数值端点。
    if property.value.source == "auto" {
        // 返回固定尺寸诊断。
        return Err(value_diagnostic(
            // 指向完整属性。
            property,
            // 陈述失败原因。
            "关键帧尺寸不支持 auto",
            // 给出数值写法。
            "使用有限非负 px 或无单位数值",
        ));
    }
    // 百分比需要父布局 used-value，当前 typed Animated 无法脱离布局解析。
    if property.value.source.ends_with('%') {
        // 返回百分比诊断。
        return Err(value_diagnostic(
            // 指向完整属性。
            property,
            // 陈述失败原因。
            "关键帧尺寸暂不支持百分比",
            // 给出数值写法。
            "使用有限非负 px 或无单位数值",
        ));
    }
    // 复用非负长度解析。
    parse_length(&property.value.source, property)
}

// 生成普通数值字段更新。
pub(super) fn numeric_field(
    // 接收 Style 标识符。
    style: &Ident,
    // 接收样式属性。
    property: &StyleProperty,
    // 接收 Rust 字段名。
    field: &str,
    // 接收范围规则。
    rule: NumberRule,
) -> Result<TokenStream, Diagnostic> {
    // 解析文档允许的像素或无单位数值。
    let value = number_value(property, rule)?;
    // 把字段名转换为卫生标识符。
    let field = Ident::new(field, Span::call_site());
    // 生成字段更新。
    Ok(quote! { #style.#field = #value; })
}

// 生成布尔字段更新。
pub(super) fn boolean_field(
    // 接收 Style 标识符。
    style: &Ident,
    // 接收样式属性。
    property: &StyleProperty,
    // 接收 Rust 字段名。
    field: &str,
) -> Result<TokenStream, Diagnostic> {
    // 解析严格布尔值。
    let value = match property.value.source.as_str() {
        // 映射 true。
        "true" => true,
        // 映射 false。
        "false" => false,
        // 拒绝其他字面量。
        value => {
            return Err(value_diagnostic(
                property,
                format!("{value:?} 不是布尔值"),
                "使用 true 或 false",
            ));
        }
    };
    // 把字段名转换为卫生标识符。
    let field = Ident::new(field, Span::call_site());
    // 生成字段更新。
    Ok(quote! { #style.#field = #value; })
}

// 生成可选固定尺寸字段更新。
pub(super) fn optional_dimension_field(
    // 接收 Style 标识符。
    style: &Ident,
    // 接收样式属性。
    property: &StyleProperty,
    // 接收 Rust 字段名。
    field: &str,
) -> Result<TokenStream, Diagnostic> {
    // 把字段名转换为卫生标识符。
    let field = Ident::new(field, Span::call_site());
    // auto 清除固定尺寸。
    if property.value.source == "auto" {
        // 生成 None 更新。
        return Ok(quote! { #style.#field = ::std::option::Option::None; });
    }
    // 百分比尚无 Style 等价表示。
    if property.value.source.ends_with('%') {
        // 返回明确的子能力差距诊断。
        return Err(value_diagnostic(
            // 传递属性。
            property,
            // 说明当前字段限制。
            "百分比尺寸尚无 Rust Style 等价表示",
            // 给出当前可用值。
            "使用 auto、像素值或无单位固定数值",
        ));
    }
    // 解析固定像素尺寸。
    let value = number_value(property, NumberRule::NonNegative)?;
    // 生成 Some 更新。
    Ok(quote! { #style.#field = ::std::option::Option::Some(#value); })
}

// 生成 EdgeInsets 简写字段更新。
pub(super) fn edge_insets_field(
    // 接收 Style 标识符。
    style: &Ident,
    // 接收样式属性。
    property: &StyleProperty,
    // 接收 Rust 字段名。
    field: &str,
) -> Result<TokenStream, Diagnostic> {
    // 解析 CSS 顺序的一到四个长度。
    let edges = edge_insets_value(property)?;
    // 把字段名转换为卫生标识符。
    let field = Ident::new(field, Span::call_site());
    // 生成完整 EdgeInsets 更新。
    Ok(quote! { #style.#field = #edges; })
}

// 生成单边 EdgeInsets 字段更新。
pub(super) fn edge_field(
    // 接收 Style 标识符。
    style: &Ident,
    // 接收样式属性。
    property: &StyleProperty,
    // 接收外层 Rust 字段名。
    outer: &str,
    // 接收边字段名。
    edge: &str,
) -> Result<TokenStream, Diagnostic> {
    // 解析单个非负长度。
    let value = number_value(property, NumberRule::NonNegative)?;
    // 把外层字段名转换为标识符。
    let outer = Ident::new(outer, Span::call_site());
    // 把边字段名转换为标识符。
    let edge = Ident::new(edge, Span::call_site());
    // 生成单边更新。
    Ok(quote! { #style.#outer.#edge = #value; })
}

// 生成边框颜色字段更新。
pub(super) fn border_color_field(
    style: &Ident,
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // none 明确清除边框颜色。
    if property.value.source == "none" {
        // 生成 None 更新。
        return Ok(quote! { #style.border_color = ::std::option::Option::None; });
    }
    // 解析颜色令牌。
    let value = color_value(property)?;
    // 生成可选颜色更新。
    Ok(quote! { #style.border_color = ::std::option::Option::Some(#value); })
}

// 生成普通或可选颜色字段更新。
pub(super) fn color_field(
    // 接收 Style 标识符。
    style: &Ident,
    // 接收样式属性。
    property: &StyleProperty,
    // 接收 Rust 字段名。
    field: &str,
    // 控制字段是否为 Option。
    optional: bool,
) -> Result<TokenStream, Diagnostic> {
    // 解析颜色令牌。
    let value = color_value(property)?;
    // 把字段名转换为卫生标识符。
    let field = Ident::new(field, Span::call_site());
    // 可选颜色包装为 Some。
    if optional {
        // 生成可选颜色更新。
        return Ok(quote! { #style.#field = ::std::option::Option::Some(#value); });
    }
    // 生成普通颜色更新。
    Ok(quote! { #style.#field = #value; })
}

// 生成字号字段更新。
pub(super) fn typography_field(
    style: &Ident,
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 按文档 token 或固定像素生成字号。
    let value = match property.value.source.as_str() {
        // 映射小号正文。
        "small" => quote! { ::uix_app::prelude::TypographyToken::Small },
        // 映射正文。
        "body" => quote! { ::uix_app::prelude::TypographyToken::Body },
        // 映射大号正文。
        "large" => quote! { ::uix_app::prelude::TypographyToken::Large },
        // 映射超大正文。
        "xlarge" | "xLarge" => quote! { ::uix_app::prelude::TypographyToken::XLarge },
        // 映射一级标题。
        "heading1" => quote! { ::uix_app::prelude::TypographyToken::Heading1 },
        // 映射二级标题。
        "heading2" => quote! { ::uix_app::prelude::TypographyToken::Heading2 },
        // 映射三级标题。
        "heading3" => quote! { ::uix_app::prelude::TypographyToken::Heading3 },
        // 映射四级标题。
        "heading4" => quote! { ::uix_app::prelude::TypographyToken::Heading4 },
        // 映射五级标题。
        "heading5" => quote! { ::uix_app::prelude::TypographyToken::Heading5 },
        // 其他值按固定字号处理。
        _ => {
            // 解析非负字号。
            let size = number_value(property, NumberRule::NonNegative)?;
            // 生成自定义字号。
            quote! { ::uix_app::prelude::TypographyToken::Custom(#size) }
        }
    };
    // 更新字号字段。
    Ok(quote! { #style.font_size = #value; })
}

// 生成显式子树裁剪字段更新。
pub(super) fn overflow_field(
    style: &Ident,
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 只映射 Style 当前可表达的两个裁剪值。
    let value = match property.value.source.as_str() {
        // visible 保留子树在节点边界外绘制与命中。
        "visible" => false,
        // hidden 把直接子树裁剪到当前节点边界。
        "hidden" => true,
        // scroll 与 auto 必须使用滚动组件。
        "scroll" | "auto" => {
            return Err(value_diagnostic(
                property,
                "overflow 的 scroll/auto 由滚动容器组件提供",
                "改用 ScrollView 或 VirtualScroll",
            ));
        }
        // 拒绝其他值。
        value => {
            return Err(value_diagnostic(
                property,
                format!("未知 overflow 值 {value:?}"),
                "使用 visible、hidden，或改用滚动容器",
            ));
        }
    };
    // 保存显式真假值以支持状态样式清除旧裁剪。
    Ok(quote! { #style.clip_content = ::std::option::Option::Some(#value); })
}

// 生成 Grid 轨道向量字段更新。
pub(super) fn grid_tracks_field(
    // 接收 Style 标识符。
    style: &Ident,
    // 接收样式属性。
    property: &StyleProperty,
    // 接收 Rust 字段名。
    field: &str,
) -> Result<TokenStream, Diagnostic> {
    // 保存有序轨道令牌。
    let mut tracks = Vec::new();
    // 按空白切分轨道列表。
    for source in property.value.source.split_whitespace() {
        // auto 映射到自动轨道。
        if source == "auto" {
            // 保存自动轨道。
            tracks.push(quote! { ::uix_app::prelude::GridTrack::Auto });
            // 继续下一轨道。
            continue;
        }
        // 识别弹性比例后缀。
        let (number, constructor) = if let Some(number) = source.strip_suffix("fr") {
            // 返回弹性轨道构造器。
            (number, "Fr")
        } else if let Some(number) = source.strip_suffix("px") {
            // 返回固定轨道构造器。
            (number, "Px")
        } else {
            // 拒绝未登记轨道单位。
            return Err(value_diagnostic(
                property,
                format!("Grid 轨道 {source:?} 缺少 px/fr 单位"),
                "使用 100px、1fr 或 auto",
            ));
        };
        // 解析正轨道数值。
        let value = parse_number(number, property, NumberRule::NonNegative)?;
        // 把构造器名转换为标识符。
        let constructor = Ident::new(constructor, Span::call_site());
        // 保存轨道构造表达式。
        tracks.push(quote! { ::uix_app::prelude::GridTrack::#constructor(#value) });
    }
    // 空轨道列表没有布局意义。
    if tracks.is_empty() {
        // 返回缺失轨道诊断。
        return Err(value_diagnostic(
            property,
            "Grid 轨道列表不能为空",
            "至少声明一个 100px、1fr 或 auto 轨道",
        ));
    }
    // 把字段名转换为卫生标识符。
    let field = Ident::new(field, Span::call_site());
    // 生成轨道向量更新。
    Ok(quote! { #style.#field = ::std::vec![#(#tracks),*]; })
}

// 生成 Grid 正整数跨度字段更新。
pub(super) fn span_field(
    style: &Ident,
    property: &StyleProperty,
    field: &str,
) -> Result<TokenStream, Diagnostic> {
    // 解析正整数跨度。
    let value =
        property.value.source.parse::<u32>().map_err(|_| {
            value_diagnostic(property, "Grid span 必须是正整数", "使用 1、2 等正整数")
        })?;
    // 零跨度非法。
    if value == 0 {
        // 返回范围诊断。
        return Err(value_diagnostic(
            property,
            "Grid span 不能为 0",
            "使用至少为 1 的正整数",
        ));
    }
    // 把字段名转换为卫生标识符。
    let field = Ident::new(field, Span::call_site());
    // 生成跨度更新。
    Ok(quote! { #style.#field = #value; })
}

// 生成 View 绘制与命中层级的有符号整数值。
pub(super) fn z_index_value(property: &StyleProperty) -> Result<TokenStream, Diagnostic> {
    // 按运行时公开契约解析完整 i32 范围。
    let value = property.value.source.parse::<i32>().map_err(|_| {
        // 返回同时覆盖小数、非数值与越界输入的确定诊断。
        value_diagnostic(
            property,
            "z-index 必须是 i32 范围内的整数",
            "使用如 -1、0 或 1000 的整数",
        )
    })?;
    // 生成可直接传给 View::z_index 的整数令牌。
    Ok(quote! { #value })
}

// 生成 display 枚举值。
pub(super) fn display_value(property: &StyleProperty) -> Result<TokenStream, Diagnostic> {
    // 映射文档登记值。
    match property.value.source.as_str() {
        // 映射 flex。
        "flex" => Ok(quote! { ::uix_app::prelude::DisplayMode::Flex }),
        // 映射 grid。
        "grid" => Ok(quote! { ::uix_app::prelude::DisplayMode::Grid }),
        // 映射 none。
        "none" => Ok(quote! { ::uix_app::prelude::DisplayMode::None }),
        // 拒绝其他显示模式。
        value => Err(value_diagnostic(
            property,
            format!("未知 display 值 {value:?}"),
            "使用 flex、grid 或 none",
        )),
    }
}

// 生成 FlexDirection 枚举值。
pub(super) fn flex_direction_value(property: &StyleProperty) -> Result<TokenStream, Diagnostic> {
    // 映射文档登记值。
    match property.value.source.as_str() {
        // 映射行方向。
        "row" => Ok(quote! { ::uix_app::prelude::FlexDirection::Row }),
        // 映射列方向。
        "column" => Ok(quote! { ::uix_app::prelude::FlexDirection::Column }),
        // 拒绝其他方向。
        value => Err(value_diagnostic(
            property,
            format!("未知 flexDirection 值 {value:?}"),
            "使用 row 或 column",
        )),
    }
}

// 生成 AlignItems 枚举值。
pub(super) fn align_value(property: &StyleProperty) -> Result<TokenStream, Diagnostic> {
    // 映射文档登记值。
    match property.value.source.as_str() {
        // 映射起始对齐。
        "flex-start" => Ok(quote! { ::uix_app::prelude::AlignItems::Start }),
        // 映射末端对齐。
        "flex-end" => Ok(quote! { ::uix_app::prelude::AlignItems::End }),
        // 映射居中对齐。
        "center" => Ok(quote! { ::uix_app::prelude::AlignItems::Center }),
        // 映射拉伸对齐。
        "stretch" => Ok(quote! { ::uix_app::prelude::AlignItems::Stretch }),
        // 拒绝其他对齐值。
        value => Err(value_diagnostic(
            property,
            format!("未知对齐值 {value:?}"),
            "使用 flex-start、flex-end、center 或 stretch",
        )),
    }
}

// 生成 JustifyContent 枚举值。
pub(super) fn justify_value(property: &StyleProperty) -> Result<TokenStream, Diagnostic> {
    // 映射文档登记值。
    match property.value.source.as_str() {
        // 映射起始对齐。
        "flex-start" => Ok(quote! { ::uix_app::prelude::JustifyContent::Start }),
        // 映射末端对齐。
        "flex-end" => Ok(quote! { ::uix_app::prelude::JustifyContent::End }),
        // 映射居中对齐。
        "center" => Ok(quote! { ::uix_app::prelude::JustifyContent::Center }),
        // 映射两端分布。
        "space-between" => Ok(quote! { ::uix_app::prelude::JustifyContent::SpaceBetween }),
        // 映射环绕分布。
        "space-around" => Ok(quote! { ::uix_app::prelude::JustifyContent::SpaceAround }),
        // 拒绝其他对齐值。
        value => Err(value_diagnostic(
            property,
            format!("未知 justifyContent 值 {value:?}"),
            "使用 flex-start、flex-end、center、space-between 或 space-around",
        )),
    }
}

// 把一到四个 CSS 长度转换为 EdgeInsets。
fn edge_insets_value(property: &StyleProperty) -> Result<TokenStream, Diagnostic> {
    // 保存解析后的有序值。
    let mut values = Vec::new();
    // 逐个解析简写分量。
    for source in property.value.source.split_whitespace() {
        // 百分比当前没有 EdgeInsets 等价表示。
        if source.ends_with('%') {
            // 返回明确子能力差距诊断。
            return Err(value_diagnostic(
                property,
                "百分比边距尚无 Rust EdgeInsets 等价表示",
                "使用像素值或无单位固定数值",
            ));
        }
        // 保存非负长度令牌。
        values.push(parse_length(source, property)?);
    }
    // CSS 简写只接受一到四项。
    let (top, right, bottom, left) = match values.as_slice() {
        // 一项应用到四边。
        [all] => (all, all, all, all),
        // 两项依次为垂直和水平。
        [vertical, horizontal] => (vertical, horizontal, vertical, horizontal),
        // 三项依次为上、水平、下。
        [top, horizontal, bottom] => (top, horizontal, bottom, horizontal),
        // 四项依次为上、右、下、左。
        [top, right, bottom, left] => (top, right, bottom, left),
        // 其他数量非法。
        _ => {
            return Err(value_diagnostic(
                property,
                "边距简写需要一到四个长度值",
                "使用如 8px、8px 12px 或 8px 12px 16px 20px",
            ));
        }
    };
    // 按 EdgeInsets 构造器的左上右下顺序生成。
    Ok(quote! { ::uix_app::prelude::EdgeInsets::new(#left, #top, #right, #bottom) })
}

// 解析一个非负长度分量。
pub(super) fn parse_length(
    // 接收待解析长度源码。
    source: &str,
    // 接收用于诊断的结构化属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 主题数值引用直接读取当前 Provider token。
    if let Some(value) = number_token_reference(source, property)? {
        // 返回已验证的主题数值表达式。
        return Ok(value);
    }
    // 去除可选 px 后缀。
    let number = source.strip_suffix("px").unwrap_or(source);
    // 解析非负有限数值。
    parse_number(number, property, NumberRule::NonNegative)
}

// 解析属性的像素或无单位数值。
fn number_value(property: &StyleProperty, rule: NumberRule) -> Result<TokenStream, Diagnostic> {
    // 主题数值引用直接读取当前 Provider token。
    if let Some(value) = number_token_reference(&property.value.source, property)? {
        // token 定义自身保证有限非负，返回运行期读取表达式。
        return Ok(value);
    }
    // 去除可选 px 后缀。
    let number = property
        .value
        .source
        .strip_suffix("px")
        .unwrap_or(&property.value.source);
    // 解析并校验范围。
    parse_number(number, property, rule)
}

// 解析有限 f32 并生成令牌。
fn parse_number(
    source: &str,
    property: &StyleProperty,
    rule: NumberRule,
) -> Result<TokenStream, Diagnostic> {
    // 解析十进制数值。
    let value = source.parse::<f32>().map_err(|_| {
        value_diagnostic(
            property,
            format!("数值 {source:?} 无法映射为 f32"),
            "使用有限十进制数或 px 长度",
        )
    })?;
    // 拒绝非有限值。
    if !value.is_finite() {
        // 返回有限值诊断。
        return Err(value_diagnostic(
            property,
            "样式数值必须有限",
            "使用有限十进制数",
        ));
    }
    // 校验非负规则。
    if matches!(rule, NumberRule::NonNegative) && value < 0.0 {
        // 返回非负范围诊断。
        return Err(value_diagnostic(
            property,
            "该样式数值不能为负数",
            "使用大于或等于 0 的数值",
        ));
    }
    // 校验单位区间规则。
    if matches!(rule, NumberRule::UnitInterval) && !(0.0..=1.0).contains(&value) {
        // 返回透明度范围诊断。
        return Err(value_diagnostic(
            property,
            "opacity 必须位于 0 到 1 之间",
            "使用 0、1 或两者之间的小数",
        ));
    }
    // 构造无后缀 f32 字面量。
    let value = Literal::f32_unsuffixed(value);
    // 返回数值令牌。
    Ok(quote! { #value })
}

// 生成 ColorValue 令牌。
pub(super) fn color_value(property: &StyleProperty) -> Result<TokenStream, Diagnostic> {
    // 闭合主题颜色引用从当前 Provider 上下文读取。
    if let Some(value) = color_token_reference(&property.value.source, property)? {
        // 返回已验证的主题颜色表达式。
        return Ok(value);
    }
    // 解析颜色的 RGBA 通道。
    let (red, green, blue, alpha) = parse_color(&property.value.source, property)?;
    // 生成公开自定义颜色值。
    Ok(quote! {
        ::uix_app::prelude::ColorValue::custom(
            ::uix_app::prelude::Color::from_rgba(#red, #green, #blue, #alpha)
        )
    })
}

// 解析允许负值的长度分量。
pub(super) fn parse_length_signed(
    // 接收待解析有符号长度源码。
    source: &str,
    // 接收用于诊断的结构化属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 去除可选 px 后缀。
    let number = source.strip_suffix("px").unwrap_or(source);
    // 解析任意有限数值。
    parse_number(number, property, NumberRule::Any)
}

// 解析文档允许的常用颜色形态。
pub(crate) fn parse_color(
    // 接收主题或样式中的颜色源码。
    source: &str,
    // 接收用于精确诊断的属性。
    property: &StyleProperty,
) -> Result<(u8, u8, u8, u8), Diagnostic> {
    // 十六进制颜色走精确位数解析。
    if let Some(hex) = source.strip_prefix('#') {
        // 主题引用不能伪装为十六进制颜色。
        if !hex.chars().all(|value| value.is_ascii_hexdigit()) {
            // 返回主题阶段差距诊断。
            return Err(value_diagnostic(
                property,
                format!("主题颜色引用 {source} 需要具名主题解析上下文"),
                "先使用十六进制、rgb/rgba 或已登记颜色名",
            ));
        }
        // 委托十六进制解析。
        return parse_hex_color(hex, property);
    }
    // rgb 函数映射为不透明颜色。
    if source.starts_with("rgb(") && source.ends_with(')') {
        // 解析三个整数通道。
        let channels = parse_rgb_channels(&source[4..source.len() - 1], property)?;
        // 返回不透明 RGBA。
        return Ok((channels[0], channels[1], channels[2], 255));
    }
    // rgba 函数映射为带透明度颜色。
    if source.starts_with("rgba(") && source.ends_with(')') {
        // 切分四个通道。
        let parts = source[5..source.len() - 1]
            .split(',')
            .map(str::trim)
            .collect::<Vec<_>>();
        // 通道数量必须为四。
        if parts.len() != 4 {
            // 返回通道数量诊断。
            return Err(value_diagnostic(
                property,
                "rgba() 需要四个参数",
                "使用 rgba(0,0,0,0.5)",
            ));
        }
        // 解析前三个整数通道。
        let channels = parse_rgb_channels(&parts[..3].join(","), property)?;
        // 解析零到一透明度。
        let alpha = parts[3].parse::<f32>().map_err(|_| {
            value_diagnostic(
                property,
                "rgba alpha 必须是 0 到 1 的小数",
                "使用 rgba(0,0,0,0.5)",
            )
        })?;
        // 校验透明度范围。
        if !(0.0..=1.0).contains(&alpha) {
            // 返回透明度范围诊断。
            return Err(value_diagnostic(
                property,
                "rgba alpha 必须位于 0 到 1 之间",
                "使用 rgba(0,0,0,0.5)",
            ));
        }
        // 转换为八位透明度。
        return Ok((
            channels[0],
            channels[1],
            channels[2],
            (alpha * 255.0).round() as u8,
        ));
    }
    // 映射文档示例与基础命名色。
    match source {
        // 映射透明色。
        "transparent" => Ok((0, 0, 0, 0)),
        // 映射黑色。
        "black" => Ok((0, 0, 0, 255)),
        // 映射白色。
        "white" => Ok((255, 255, 255, 255)),
        // 映射红色。
        "red" => Ok((255, 0, 0, 255)),
        // 映射绿色。
        "green" => Ok((0, 128, 0, 255)),
        // 映射蓝色。
        "blue" => Ok((0, 0, 255, 255)),
        // 拒绝未登记颜色名。
        value => Err(value_diagnostic(
            property,
            format!("颜色值 {value:?} 尚无确定映射"),
            "使用十六进制、rgb/rgba、transparent、black、white、red、green 或 blue",
        )),
    }
}

// 解析十六进制颜色。
fn parse_hex_color(hex: &str, property: &StyleProperty) -> Result<(u8, u8, u8, u8), Diagnostic> {
    // 展开三位与四位短格式。
    let expanded = match hex.len() {
        // 三位 RGB 各复制一次。
        3 => hex
            .chars()
            .flat_map(|value| [value, value])
            .collect::<String>(),
        // 四位 RGBA 各复制一次。
        4 => hex
            .chars()
            .flat_map(|value| [value, value])
            .collect::<String>(),
        // 六位与八位保持不变。
        6 | 8 => hex.to_string(),
        // 其他位数非法。
        _ => {
            return Err(value_diagnostic(
                property,
                "十六进制颜色需要 3、4、6 或 8 位",
                "使用 #fff、#ffff、#ffffff 或 #ffffffff",
            ));
        }
    };
    // 解析一个两位通道。
    let channel = |start: usize| {
        u8::from_str_radix(&expanded[start..start + 2], 16).expect("已验证十六进制字符")
    };
    // 读取 RGB 通道。
    let red = channel(0);
    // 读取绿色通道。
    let green = channel(2);
    // 读取蓝色通道。
    let blue = channel(4);
    // 八位格式读取透明度，否则不透明。
    let alpha = if expanded.len() == 8 { channel(6) } else { 255 };
    // 返回 RGBA。
    Ok((red, green, blue, alpha))
}

// 解析三个 RGB 整数通道。
fn parse_rgb_channels(source: &str, property: &StyleProperty) -> Result<[u8; 3], Diagnostic> {
    // 切分并去除空白。
    let parts = source.split(',').map(str::trim).collect::<Vec<_>>();
    // 必须恰有三个通道。
    if parts.len() != 3 {
        // 返回通道数量诊断。
        return Err(value_diagnostic(
            property,
            "rgb() 需要三个整数参数",
            "使用 rgb(255,255,255)",
        ));
    }
    // 保存已解析通道。
    let mut channels = [0u8; 3];
    // 逐个解析八位通道。
    for (index, part) in parts.iter().enumerate() {
        // u8 解析同时验证零到 255。
        channels[index] = part.parse::<u8>().map_err(|_| {
            value_diagnostic(
                property,
                "RGB 通道必须是 0 到 255 的整数",
                "使用 rgb(255,255,255)",
            )
        })?;
    }
    // 返回三个通道。
    Ok(channels)
}

// 判断属性是否仍由文档明确标记为规划中。
pub(super) fn is_planned_property(name: &str) -> bool {
    // 保留参数以维持统一未知属性查询签名。
    let _ = name;
    // animation 与 transition 均由独立降级阶段消费，当前没有样式规划占位。
    false
}

// 构造指向样式值的统一诊断。
pub(super) fn value_diagnostic(
    // 接收所属属性。
    property: &StyleProperty,
    // 接收失败原因。
    message: impl Into<String>,
    // 接收修复建议。
    suggestion: impl Into<String>,
) -> Diagnostic {
    // 返回包含精确值跨度的诊断。
    Diagnostic::new(property.value.span, message, suggestion)
}

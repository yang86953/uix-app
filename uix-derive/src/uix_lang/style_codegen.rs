// 引入过程宏数值字面量与令牌流。
use proc_macro2::{Ident, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入结构化属性、诊断与样式属性。
use super::{Attribute, AttributeValue, Diagnostic, StyleProperty};
// 引入 transform 函数列表到运行时矩阵的独立映射。
use super::style_transform_codegen::transform_value;
// 引入样式值与字段映射辅助。
use super::style_value_codegen::*;

// 把一个结构化内联 style 精确应用到既有 View 表达式。
pub(crate) fn apply_inline_style(
    // 接收已经生成的 View 表达式。
    view: TokenStream,
    // 接收名称为 style 的属性。
    attribute: &Attribute,
) -> Result<TokenStream, Diagnostic> {
    // 内联 style 必须已经由解析器结构化。
    let AttributeValue::InlineStyle(properties) = &attribute.value else {
        // 返回内部形状保护诊断。
        return Err(Diagnostic::new(
            // 指向完整 style 属性。
            attribute.span,
            // 说明值形状不正确。
            "style 必须使用双引号包裹的样式属性列表",
            // 给出规范写法。
            "使用 style=\"color: #fff; padding: 8px;\"",
        ));
    };
    // 委托属性列表入口保持静态与动态样式字段语义一致。
    apply_style_properties(view, properties)
}

// 把一组已经映射的样式属性精确应用到既有 View 表达式。
pub(crate) fn apply_style_properties(
    // 接收已经生成的 View 表达式。
    view: TokenStream,
    // 接收按优先级完成合并的样式属性。
    properties: &[StyleProperty],
) -> Result<TokenStream, Diagnostic> {
    // 创建不会与调用方绑定冲突的样式参数。
    let style = Ident::new("__uix_style", Span::mixed_site());
    // 保存源码顺序中的精确字段更新。
    let mut statements = Vec::new();
    // 保存由 View 运行时而非 Style 拥有的层级更新。
    let mut z_index = None;
    // 保存由 View 视觉变换运行时拥有的二维仿射更新。
    let mut transform = None;
    // 转换每一个已经解析的样式属性。
    for property in properties {
        // z-index 直接映射到 View 的绘制与命中顺序。
        if property.name == "z-index" {
            // 解析有符号整数层级并留待样式更新后应用。
            z_index = Some(z_index_value(property)?);
            // 结构属性不进入 Style 字段生成。
            continue;
        }
        // transform 直接映射到 View 的视觉与命中矩阵。
        if property.name == "transform" {
            // 解析完整函数列表并留待样式更新后应用。
            transform = Some(transform_value(property)?);
            // 结构变换不进入 Style 字段生成。
            continue;
        }
        // 把属性转换为单个字段更新语句。
        statements.push(generate_style_statement(&style, property)?);
    }
    // 先生成保留未声明字段的 Style 更新。
    let styled = quote! {
        // 只更新内联样式明确声明的字段。
        (#view).map_style(|#style| {
            // 按源码顺序执行确定的字段更新。
            #(#statements)*
        })
    };
    // 保存已经应用结构层级的节点表达式。
    let layered = if let Some(z_index) = z_index {
        // 同时携带视觉样式与绘制命中层级。
        quote! { (#styled).z_index(#z_index) }
    } else {
        // 没有层级声明时保持样式节点不变。
        styled
    };
    // 视觉变换存在时交给 View 运行时的唯一公开入口。
    if let Some(transform) = transform {
        // 返回同时携带样式、层级与仿射变换的节点。
        return Ok(quote! { (#layered).affine_transform(#transform) });
    }
    // 返回已经完成全部受控更新的节点。
    Ok(layered)
}

// 把单个已映射样式属性转换为 Style 字段更新。
fn generate_style_statement(
    // 接收卫生的 Style 借用标识符。
    style: &Ident,
    // 接收结构化样式属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 按文档中的映射矩阵选择公开字段。
    match property.name.as_str() {
        // 显示模式映射到 DisplayMode。
        "display" => {
            // 生成显示模式枚举。
            let value = display_value(property)?;
            // 更新显示模式字段。
            Ok(quote! { #style.display = #value; })
        }
        // 主轴方向映射到 FlexDirection。
        "flexDirection" => {
            // 生成方向枚举。
            let value = flex_direction_value(property)?;
            // 更新方向字段。
            Ok(quote! { #style.flex_direction = #value; })
        }
        // 主轴对齐映射到 JustifyContent。
        "justifyContent" => {
            // 生成主轴对齐枚举。
            let value = justify_value(property)?;
            // 更新主轴对齐字段。
            Ok(quote! { #style.justify_content = #value; })
        }
        // 交叉轴对齐映射到 AlignItems。
        "alignItems" => {
            // 生成交叉轴对齐枚举。
            let value = align_value(property)?;
            // 更新交叉轴对齐字段。
            Ok(quote! { #style.align_items = #value; })
        }
        // 子项自身对齐映射到可选 AlignItems。
        "alignSelf" => {
            // 生成交叉轴对齐枚举。
            let value = align_value(property)?;
            // 更新自身对齐字段。
            Ok(quote! { #style.align_self = ::std::option::Option::Some(#value); })
        }
        // 间距映射到有限非负数值。
        "gap" => numeric_field(style, property, "gap", NumberRule::NonNegative),
        // 扩张因子映射到有限非负数值。
        "flexGrow" => numeric_field(style, property, "flex_grow", NumberRule::NonNegative),
        // 收缩因子映射到有限非负数值。
        "flexShrink" => numeric_field(style, property, "flex_shrink", NumberRule::NonNegative),
        // 换行开关映射到布尔值。
        "flexWrap" => boolean_field(style, property, "flex_wrap"),
        // Grid 列模板映射到轨道向量。
        "gridTemplateColumns" => grid_tracks_field(style, property, "grid_template_columns"),
        // Grid 行模板映射到轨道向量。
        "gridTemplateRows" => grid_tracks_field(style, property, "grid_template_rows"),
        // Grid 列间距映射到有限非负数值。
        "gridColumnGap" => {
            numeric_field(style, property, "grid_column_gap", NumberRule::NonNegative)
        }
        // Grid 行间距映射到有限非负数值。
        "gridRowGap" => numeric_field(style, property, "grid_row_gap", NumberRule::NonNegative),
        // Grid 列跨度映射到正整数。
        "gridColumnSpan" => span_field(style, property, "grid_column_span"),
        // Grid 行跨度映射到正整数。
        "gridRowSpan" => span_field(style, property, "grid_row_span"),
        // 宽度映射到可选固定像素。
        "width" => optional_dimension_field(style, property, "width"),
        // 高度映射到可选固定像素。
        "height" => optional_dimension_field(style, property, "height"),
        // 外边距简写映射到 EdgeInsets。
        "margin" => edge_insets_field(style, property, "margin"),
        // 内边距简写映射到 EdgeInsets。
        "padding" => edge_insets_field(style, property, "padding"),
        // 单边外边距映射到对应字段。
        "marginTop" => edge_field(style, property, "margin", "top"),
        // 单边外边距映射到对应字段。
        "marginRight" => edge_field(style, property, "margin", "right"),
        // 单边外边距映射到对应字段。
        "marginBottom" => edge_field(style, property, "margin", "bottom"),
        // 单边外边距映射到对应字段。
        "marginLeft" => edge_field(style, property, "margin", "left"),
        // 单边内边距映射到对应字段。
        "paddingTop" => edge_field(style, property, "padding", "top"),
        // 单边内边距映射到对应字段。
        "paddingRight" => edge_field(style, property, "padding", "right"),
        // 单边内边距映射到对应字段。
        "paddingBottom" => edge_field(style, property, "padding", "bottom"),
        // 单边内边距映射到对应字段。
        "paddingLeft" => edge_field(style, property, "padding", "left"),
        // 边框颜色映射到可选 ColorValue。
        "borderColor" => border_color_field(style, property),
        // 四边边框宽度映射到 EdgeInsets。
        "borderWidth" => edge_insets_field(style, property, "border_width"),
        // 单边边框宽度映射到对应字段。
        "borderTopWidth" => edge_field(style, property, "border_width", "top"),
        // 单边边框宽度映射到对应字段。
        "borderRightWidth" => edge_field(style, property, "border_width", "right"),
        // 单边边框宽度映射到对应字段。
        "borderBottomWidth" => edge_field(style, property, "border_width", "bottom"),
        // 单边边框宽度映射到对应字段。
        "borderLeftWidth" => edge_field(style, property, "border_width", "left"),
        // 圆角映射到有限非负数值。
        "borderRadius" => numeric_field(style, property, "border_radius", NumberRule::NonNegative),
        // 文本颜色映射到 ColorValue。
        "color" => color_field(style, property, "color", false),
        // 字号映射到 TypographyToken。
        "fontSize" => typography_field(style, property),
        // 普通背景映射到可选 ColorValue。
        "backgroundColor" => color_field(style, property, "background", true),
        // 悬停背景映射到状态字段。
        "backgroundColor:hover" => color_field(style, property, "background_hover", true),
        // 焦点背景映射到状态字段。
        "backgroundColor:focus" => color_field(style, property, "background_focus", true),
        // 激活背景映射到状态字段。
        "backgroundColor:active" => color_field(style, property, "background_active", true),
        // 透明度映射到零到一数值。
        "opacity" => numeric_field(style, property, "opacity", NumberRule::UnitInterval),
        // overflow 映射到内容溢出开关。
        "overflow" => overflow_field(style, property),
        // 阴影映射到 BoxShadowDef。
        "boxShadow" => box_shadow_field(style, property),
        // 可见性映射到布尔值。
        "visible" => boolean_field(style, property, "visible"),
        // 文档标注规划中的属性必须明确拒绝。
        name if is_planned_property(name) => Err(Diagnostic::new(
            // 指向完整规划中属性。
            property.span,
            // 说明文档状态而非伪装支持。
            format!("样式属性 {name} 在文档中标记为规划中，当前没有 Rust Style 等价字段"),
            // 给出可执行修复方向。
            "删除该属性，或改用样式参考中标记为已映射的属性",
        )),
        // 其他属性禁止静默丢弃。
        name => Err(Diagnostic::new(
            // 指向完整未知属性。
            property.span,
            // 说明矩阵中没有登记。
            format!("样式属性 {name} 尚无已登记的 Rust Style 映射"),
            // 给出同步矩阵的修复动作。
            "核对 UIX 样式属性参考，并先登记映射状态后再使用",
        )),
    }
}

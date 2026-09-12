// 引入过程宏数值字面量与令牌流。
use proc_macro2::{Ident, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入 Compiler System 唯一样式属性登记。
use crate::lang::compiler::projection_schema::UI_PROJECTION_SCHEMA;

// 引入结构化属性、诊断与样式属性。
use super::{Attribute, AttributeValue, Diagnostic, StyleProperty};
// 引入三个背景图层属性到 UI 运行时值的独立映射。
use super::style_background_codegen::{
    background_image_field, background_position_field, background_repeat_field,
};
// 引入 borderStyle 与 borderRadius 到 UI 运行时边框契约的独立映射。
use super::style_border_codegen::{border_radius_field, border_style_field};
// 引入 backgroundSize 到 UI 运行时背景尺寸契约的独立映射。
use super::style_background_codegen::background_size_field;
// 引入 fontFamily 到 UI 运行时有序字体族列表的独立映射。
use super::style_font_family_codegen::font_family_field;
// 引入 fontWeight 到 UI 运行时字体粗细的独立映射。
use super::style_font_weight_codegen::font_weight_field;
// 引入 float/clear 非目标布局决策的专用诊断。
use super::style_float_codegen::reject_float_or_clear;
// 引入 lineHeight 到 UI 运行时行高的独立映射。
use super::style_line_height_codegen::line_height_field;
// 引入保留单位长度到 UI 运行时 StyleLength 的独立映射。
use super::style_length_codegen::{LengthRule, style_length_value};
// 引入 @media 条件到运行时窗口宽度判定的独立映射。
use super::media_query_codegen::media_condition;
// 引入 position 与四边差异值到运行时结构契约的独立映射。
use super::style_position_codegen::{apply_position_inset, position_value};
// 引入 textAlign 到 UI 运行时文本水平对齐的独立映射。
use super::style_text_align_codegen::text_align_field;
// 引入 textDecoration 到 UI 运行时文本装饰的独立映射。
use super::style_text_decoration_codegen::text_decoration_field;
// 引入 transform 与原点到运行时公共契约的独立映射。
use super::style_transform_codegen::{transform_origin_value, transform_value};
// 引入 userSelect 到 UI 运行时选择策略的独立映射。
use super::style_user_select_codegen::user_select_value;
// 引入 cursor 文档值到公开平台无关枚举的独立映射。
use super::style_cursor_codegen::cursor_value;
// 引入样式值与字段映射辅助。
use super::style_value_codegen::*;
// 引入完整盒阴影字段的独立生成器。
use super::style_shadow_codegen::box_shadow_field;

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
    // 创建承接显式声明字段的差异样式参数。
    let decl = Ident::new("__uix_style_decl", Span::mixed_site());
    // 保存源码顺序中的精确字段更新。
    let mut statements = Vec::new();
    // 保存显式声明过的差异字段名（与语句同序，允许重复快照）。
    let mut declared_fields: Vec<&'static str> = Vec::new();
    // 保存由 View 运行时而非 Style 拥有的层级更新。
    let mut z_index = None;
    // 保存由 View 视觉变换运行时拥有的二维仿射更新。
    let mut transform = None;
    // 保存由 View 运行时在布局帧确定后解析的变换原点更新。
    let mut transform_origin = None;
    // 保存由运行时命中树继承并交给平台的指针光标更新。
    let mut cursor = None;
    // 保存由运行时组件树解析声明值与实际值的文字选择策略。
    let mut user_select = None;
    // 保存由组件树布局 Module 解析的定位模式。
    let mut position = None;
    // 保存按源码顺序只修改单边的定位差异属性。
    let mut position_insets = Vec::new();
    // 转换每一个已经解析的样式属性。
    for property in properties {
        // 支持面先由 schema 裁决；float/clear 保留专用非目标诊断。
        if UI_PROJECTION_SCHEMA
            .style_property(&property.name)
            .is_none()
            && !matches!(property.name.as_str(), "float" | "clear")
        {
            // 未登记名称不能进入任何生成分支。
            return Err(Diagnostic::new(
                property.span,
                format!("样式属性 {} 尚无已登记的 Rust Style 映射", property.name),
                "核对 UIX 样式属性参考，并先在 UiProjectionSchema 登记后再使用",
            ));
        }
        // 媒体条件层只覆盖进入 Style 的字段；结构属性没有条件差异表示。
        if property.media.is_some() && style_diff_field(&property.name).is_none() {
            return Err(Diagnostic::new(
                property.span,
                format!("@media 块内暂不支持 {}", property.name),
                "本期媒体条件只覆盖进入 Style 的视觉与布局字段",
            ));
        }
        // z-index 直接映射到 View 的绘制与命中顺序。
        if property.name == "z-index" {
            // 解析有符号整数层级并留待样式更新后应用。
            z_index = Some(z_index_value(property)?);
            // 结构属性不进入 Style 字段生成。
            continue;
        }
        // transformOrigin 映射到 View 的公开原点值契约。
        if property.name == "transformOrigin" {
            // 解析关键字、百分比、像素与可选零 Z 值。
            transform_origin = Some(transform_origin_value(property)?);
            // 结构原点不进入 Style 字段生成。
            continue;
        }
        // transform 直接映射到 View 的视觉与命中矩阵。
        if property.name == "transform" {
            // 解析完整函数列表并留待样式更新后应用。
            transform = Some(transform_value(property)?);
            // 结构变换不进入 Style 字段生成。
            continue;
        }
        // cursor 映射到公开平台无关光标枚举。
        if property.name == "cursor" {
            // 解析文档登记的五种光标值。
            cursor = Some(cursor_value(property)?);
            // 结构光标不进入 Style 字段生成。
            continue;
        }
        // userSelect 映射到公开的树级选择策略枚举。
        if property.name == "userSelect" {
            // 解析文档登记的四种文字选择值。
            user_select = Some(user_select_value(property)?);
            // 结构选择策略不进入 Style 字段生成。
            continue;
        }
        // position 映射到公开的五模式布局定位枚举。
        if property.name == "position" {
            // 解析文档登记的五种定位模式。
            position = Some(position_value(property)?);
            // 结构定位不进入视觉 Style 字段生成。
            continue;
        }
        // 四边值逐边保存，确保状态差异不会清除未声明的其他边。
        if matches!(property.name.as_str(), "top" | "right" | "bottom" | "left") {
            // 保存借用供视觉 Style 与其他结构属性应用后链式生成。
            position_insets.push(property);
            // 定位四边不进入视觉 Style 字段生成。
            continue;
        }
        // 把属性转换为字段更新语句（含圆角等需要同步登记声明的属性）。
        let statement = generate_style_statement(&style, &decl, property)?;
        match (&property.media, style_diff_field(&property.name)) {
            // 条件层：构建期按窗口宽度评估，命中时才更新字段并登记声明。
            (Some(query), Some(field)) => {
                let condition = media_condition(query);
                let field = Ident::new(field, Span::mixed_site());
                statements.push(quote! {
                    if #condition {
                        #statement
                        #decl.#field = ::std::option::Option::Some(#style.#field.clone());
                    }
                });
            }
            // 无条件声明按源码顺序更新，并在末尾统一登记声明存在性。
            (_, field) => {
                statements.push(statement);
                if let Some(field) = field {
                    declared_fields.push(field);
                }
            }
        }
    }
    // 把显式写过的字段从值样式快照进差异样式。
    let declared = declared_fields
        .into_iter()
        .map(|field| {
            let field = Ident::new(field, Span::mixed_site());
            quote! {
                // 显式默认值（0/1/none/auto）也是声明，必须在内核恢复。
                #decl.#field = ::std::option::Option::Some(#style.#field.clone());
            }
        });
    // 先生成保留未声明字段的 Style 更新。
    let styled = quote! {
        // 更新内联样式明确声明的字段，并登记声明存在性。
        (#view).map_style_declared(|#style, #decl| {
            // 按源码顺序执行确定的字段更新。
            #(#statements)*
            // 显式写过的字段进入声明差异。
            #(#declared)*
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
    // 保存已经应用仿射矩阵的节点表达式。
    let transformed = if let Some(transform) = transform {
        // 同时携带样式、层级与仿射变换。
        quote! { (#layered).affine_transform(#transform) }
    } else {
        // 没有矩阵声明时保持当前节点不变。
        layered
    };
    // 保存已经应用变换原点的节点表达式。
    let with_origin = if let Some(transform_origin) = transform_origin {
        // 同时携带矩阵与原点语义。
        quote! { (#transformed).transform_origin(#transform_origin) }
    } else {
        // 没有原点声明时保持当前节点不变。
        transformed
    };
    // 保存已经应用命中光标策略的节点表达式。
    let with_cursor = if let Some(cursor) = cursor {
        // 同时携带全部视觉结构与光标语义。
        quote! { (#with_origin).cursor(#cursor) }
    } else {
        // 没有光标声明时保持当前节点不变。
        with_origin
    };
    // 保存已经应用定位模式的节点表达式。
    let with_position = if let Some(position) = position {
        // 同时携带结构定位模式与既有视觉交互声明。
        quote! { (#with_cursor).position(#position) }
    } else {
        // 未声明 position 时保留基础类或较低状态层的模式。
        with_cursor
    };
    // 逐边叠加 px 或 auto，保持差异样式未声明边不变。
    let mut positioned = with_position;
    // 按最终级联属性顺序应用每个显式四边值。
    for property in position_insets {
        // 只修改当前边并保留其他定位元数据。
        positioned = apply_position_inset(positioned, property)?;
    }
    // 文字选择策略存在时交给 WidgetTree 解析继承与组件默认值。
    if let Some(user_select) = user_select {
        // 返回同时携带视觉、光标与文字选择语义的节点。
        return Ok(quote! { (#positioned).user_select(#user_select) });
    }
    // 返回已经完成全部受控更新的节点。
    Ok(positioned)
}

// 把已登记样式属性映射到 StyleDiff 的同名字段。
//
// 全部样式属性都拥有与 Style 字段同名的逐字段差异表示；单边差值
// 属性（marginTop 等）映射到完整的 EdgeInsets 字段快照。
fn style_diff_field(property: &str) -> Option<&'static str> {
    let field = match property {
        "display" => "display",
        "flexDirection" => "flex_direction",
        "justifyContent" => "justify_content",
        "alignItems" => "align_items",
        "alignSelf" => "align_self",
        "gap" => "gap",
        "flexGrow" => "flex_grow",
        "flexShrink" => "flex_shrink",
        "flexWrap" => "flex_wrap",
        "gridTemplateColumns" => "grid_template_columns",
        "gridTemplateRows" => "grid_template_rows",
        "gridColumnGap" => "grid_column_gap",
        "gridRowGap" => "grid_row_gap",
        "gridColumnSpan" => "grid_column_span",
        "gridRowSpan" => "grid_row_span",
        "width" => "width",
        "height" => "height",
        "minWidth" => "min_width",
        "maxWidth" => "max_width",
        "minHeight" => "min_height",
        "maxHeight" => "max_height",
        "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" => "margin",
        "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" => "padding",
        "borderColor" => "border_color",
        "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth"
        | "borderLeftWidth" => "border_width",
        "borderStyle" => "border_style",
        "borderRadius" => "border_radius",
        "color" => "color",
        "fontSize" => "font_size",
        "fontFamily" => "font_family",
        "fontWeight" => "font_weight",
        "lineHeight" => "line_height",
        "textAlign" => "text_align",
        "textDecoration" => "text_decoration",
        "backgroundColor" => "background",
        "backgroundColor:hover" => "background_hover",
        "backgroundColor:focus" => "background_focus",
        "backgroundColor:active" => "background_active",
        "backgroundImage" => "background_image",
        "backgroundPosition" => "background_position",
        "backgroundRepeat" => "background_repeat",
        "backgroundSize" => "background_size",
        "opacity" => "opacity",
        "overflow" => "overflow_content",
        "boxShadow" => "box_shadow",
        "visible" => "visible",
        // 定位与结构属性不进入视觉 Style，也就没有差异字段。
        _ => return None,
    };
    Some(field)
}

// 生成保留单位的尺寸约束字段更新。
fn size_bound_field(
    style: &Ident,
    property: &StyleProperty,
    field: &str,
) -> Result<TokenStream, Diagnostic> {
    // auto/none 不约束；px、百分比与有限 calc 保留单位到运行时解析。
    let value = style_length_value(property, LengthRule::SIZE_BOUND)?;
    let field = Ident::new(field, Span::call_site());
    Ok(quote! { #style.#field = #value; })
}

// 把单个已映射样式属性转换为 Style 字段更新。
fn generate_style_statement(
    // 接收卫生的 Style 借用标识符。
    style: &Ident,
    // 接收卫生的 StyleDiff 借用标识符，供需要显式登记声明的属性使用。
    decl: &Ident,
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
        // 四个尺寸约束保留 px/百分比/calc 单位到运行时 StyleLength。
        "minWidth" => size_bound_field(style, property, "min_width"),
        "maxWidth" => size_bound_field(style, property, "max_width"),
        "minHeight" => size_bound_field(style, property, "min_height"),
        "maxHeight" => size_bound_field(style, property, "max_height"),
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
        // 边框线型映射到显式可选 BorderStyle。
        "borderStyle" => border_style_field(style, property),
        // 单边边框宽度映射到对应字段。
        "borderTopWidth" => edge_field(style, property, "border_width", "top"),
        // 单边边框宽度映射到对应字段。
        "borderRightWidth" => edge_field(style, property, "border_width", "right"),
        // 单边边框宽度映射到对应字段。
        "borderBottomWidth" => edge_field(style, property, "border_width", "bottom"),
        // 单边边框宽度映射到对应字段。
        "borderLeftWidth" => edge_field(style, property, "border_width", "left"),
        // 圆角映射到单值或四角形式，多值同步登记声明差异。
        "borderRadius" => border_radius_field(style, decl, property),
        // 文本颜色映射到 ColorValue。
        "color" => color_field(style, property, "color", false),
        // 字号映射到 TypographyToken。
        "fontSize" => typography_field(style, property),
        // 字体族映射到显式有序回退列表契约。
        "fontFamily" => font_family_field(style, property),
        // 字体粗细映射到显式精确数值契约。
        "fontWeight" => font_weight_field(style, property),
        // 行高映射到显式倍率或逻辑像素值。
        "lineHeight" => line_height_field(style, property),
        // 文本水平对齐映射到显式闭合枚举。
        "textAlign" => text_align_field(style, property),
        // 文本装饰映射到显式闭合枚举。
        "textDecoration" => text_decoration_field(style, property),
        // 普通背景映射到可选 ColorValue。
        "backgroundColor" => color_field(style, property, "background", true),
        // 单层背景来源映射到显式 BackgroundImage。
        "backgroundImage" => background_image_field(style, property),
        // 背景二维定位映射到显式 BackgroundPosition。
        "backgroundPosition" => background_position_field(style, property),
        // 背景重复方式映射到显式 BackgroundRepeat。
        "backgroundRepeat" => background_repeat_field(style, property),
        // 背景图尺寸策略映射到显式 BackgroundSize。
        "backgroundSize" => background_size_field(style, property),
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
        "boxShadow" => box_shadow_field(style, property, decl),
        // 可见性映射到布尔值。
        "visible" => boolean_field(style, property, "visible"),
        // UIX 不建立第二套 CSS 浮动格式上下文。
        "float" | "clear" => reject_float_or_clear(property),
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

// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入属性值、布尔值、表达式与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, boolean_value, generate_expression, literal_string,
};

// 判断属性是否属于十二类图表共享的声明契约。
pub(super) fn is_common_chart_attribute(name: &str) -> bool {
    // 返回共享属性白名单判定。
    matches!(
        name,
        "title"
            | "subtitle"
            | "responsive"
            | "legend"
            | "animation"
            | "interactive"
            | "brush"
            | "tooltip"
            | "referenceLines"
    )
}

// 把一个已登记的共同属性映射到现有 ChartPlaceholder builder。
pub(super) fn apply_common_chart_attribute(
    // 接收前序构建器链。
    widget: TokenStream,
    // 接收当前共享属性。
    attribute: &Attribute,
) -> Result<TokenStream, Diagnostic> {
    // 标题与副标题保持静态文本契约。
    if matches!(attribute.name.as_str(), "title" | "subtitle") {
        // 读取已经登记的静态文本。
        let value = literal_string(attribute, "图表文本属性")?;
        // 调用对应公开构建器。
        return Ok(if attribute.name == "title" {
            // 设置图表标题。
            quote! { (#widget).title(#value) }
        } else {
            // 设置图表副标题。
            quote! { (#widget).subtitle(#value) }
        });
    }
    // 响应式测量复用统一布尔简写与表达式规则。
    if attribute.name == "responsive" {
        // 生成布尔值。
        let value = boolean_value(attribute)?;
        // 调用现有响应式 builder。
        return Ok(quote! { (#widget).responsive(#value) });
    }
    // 图例允许静态语义值或类型化 LegendPosition 表达式。
    if attribute.name == "legend" {
        // 生成公开图例枚举值。
        let value = legend_value(attribute)?;
        // 调用现有图例 builder。
        return Ok(quote! { (#widget).legend(#value) });
    }
    // 坐标图参考线使用精确集合并依次调用现有 builder。
    if attribute.name == "referenceLines" {
        // 生成参考线集合映射。
        return reference_lines(widget, attribute);
    }
    // 其余四项高级配置只接受类型化 Rust 表达式。
    let value = typed_config_expression(attribute)?;
    // 按属性名调用精确公开 builder，并克隆声明快照以支持重复构建。
    Ok(match attribute.name.as_str() {
        // 设置入场动画配置。
        "animation" => quote! { (#widget).animation((#value).clone()) },
        // 设置点击、缩放、平移与十字线配置。
        "interactive" => quote! { (#widget).interactive((#value).clone()) },
        // 设置刷选配置。
        "brush" => quote! { (#widget).brush((#value).clone()) },
        // 设置 tooltip 配置。
        "tooltip" => quote! { (#widget).tooltip((#value).clone()) },
        // 调用方已经通过共享属性白名单收窄集合。
        _ => unreachable!("图表共同属性集合已穷尽"),
    })
}

// 把类型化参考线集合投影为重复 builder 调用。
fn reference_lines(
    // 接收前序图表构建器链。
    widget: TokenStream,
    // 接收参考线集合属性。
    attribute: &Attribute,
) -> Result<TokenStream, Diagnostic> {
    // 参考线必须通过 Rust 表达式提供类型化集合。
    let AttributeValue::Expression(expression) = &attribute.value else {
        // 返回明确的集合类型诊断。
        return Err(Diagnostic::new(
            // 指向非法参考线属性。
            attribute.span,
            // 说明目标值形状。
            "图表 referenceLines 必须是类型化参考线集合表达式",
            // 给出公开集合类型。
            "使用 Vec<(f32, String, LineStyle)> 表达式",
        ));
    };
    // 生成受限集合表达式。
    let value = generate_expression(&expression.expression, None)?;
    // 先固定公开元素类型，再折叠到既有 reference_line builder。
    Ok(quote! {
        ::std::iter::IntoIterator::into_iter((#value).clone())
            .collect::<::std::vec::Vec<(
                f32,
                ::std::string::String,
                ::uix_app::prelude::LineStyle
            )>>()
            .into_iter()
            .fold(#widget, |chart, (value, label, style)| {
                chart.reference_line(value, label, style)
            })
    })
}

// 生成静态或动态图例位置。
fn legend_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状生成。
    match &attribute.value {
        // 静态值在宏展开期映射到公开枚举。
        AttributeValue::Literal(value) => {
            // 按文档登记值选择枚举成员。
            let mapped = match value.as_str() {
                // 图例位于顶部。
                "top" => quote! { ::uix_app::prelude::LegendPosition::Top },
                // 图例位于底部。
                "bottom" => quote! { ::uix_app::prelude::LegendPosition::Bottom },
                // 图例位于左侧。
                "left" => quote! { ::uix_app::prelude::LegendPosition::Left },
                // 图例位于右侧。
                "right" => quote! { ::uix_app::prelude::LegendPosition::Right },
                // 显式隐藏图例。
                "none" => quote! { ::uix_app::prelude::LegendPosition::None },
                // 其他值不在公开契约中。
                _ => {
                    // 返回完整允许集合诊断。
                    return Err(Diagnostic::new(
                        // 指向非法图例属性。
                        attribute.span,
                        // 点名非法静态值。
                        format!("图表 legend 不支持值 '{value}'"),
                        // 给出全部静态选项和动态入口。
                        "使用 top、bottom、left、right、none 或 LegendPosition 表达式",
                    ));
                }
            };
            // 返回静态枚举路径。
            Ok(mapped)
        }
        // 动态表达式由 Rust 核对 LegendPosition 类型。
        AttributeValue::Expression(expression) => {
            // 生成受限表达式并克隆声明快照。
            let value = generate_expression(&expression.expression, None)?;
            // 返回不会移动外部绑定的表达式。
            Ok(quote! { (#value).clone() })
        }
        // 内联样式不能成为图例位置。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向非法属性。
            attribute.span,
            // 说明图例值形状。
            "图表 legend 不能使用内联样式",
            // 给出合法静态或动态写法。
            "使用 legend=\"bottom\" 或 legend={position}",
        )),
    }
}

// 生成动画或交互配置表达式。
fn typed_config_expression(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 字符串与布尔简写不能伪装成运行时配置对象。
    let AttributeValue::Expression(expression) = &attribute.value else {
        // 返回带具体属性名的类型化配置诊断。
        return Err(Diagnostic::new(
            // 指向非法配置属性。
            attribute.span,
            // 说明目标值形状。
            format!("图表 {} 必须是类型化配置表达式", attribute.name),
            // 指引调用方传入公开配置类型。
            format!(
                "使用 {}={{config}}，由 Rust 核对公开配置类型",
                attribute.name
            ),
        ));
    };
    // 生成受限表达式，具体类型由公开 builder 与 Rust 编译器共同核对。
    generate_expression(&expression.expression, None)
}

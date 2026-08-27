// 引入 usize 字面量与过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::{Literal, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 复用基础图表的数值与必填属性校验。
use super::basic_chart_codegen::{f32_value, required_attribute};
// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入十二类图表共享的高级配置映射。
use super::chart_common_codegen::{apply_common_chart_attribute, is_common_chart_attribute};
// 复用内联图表数据构造器错配校验。
use super::static_chart_codegen::validate_inline_data;
// 引入属性、表达式与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, generate_expression, literal_string};

// 生成雷达图或组合图的类型化多集合映射。
pub(crate) fn generate_series_chart(
    // 接收已经解析的图表元素。
    element: &Element,
) -> Result<TokenStream, Diagnostic> {
    // 图表自行绘制完整内容，不接受 UIX 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整图表元素。
            element.span,
            // 点名当前图表标签。
            format!("<{}> 不接受子节点", element.name),
            // 给出属性式声明写法。
            format!("使用 <{} ... />", element.name),
        ));
    }
    // 按标签生成初始组件和专有属性集合。
    let (mut widget, consumed): (TokenStream, &[&str]) = match element.name.as_str() {
        // 雷达图要求轴和系列两个精确集合。
        "RadarChart" => generate_radar_start(element)?,
        // 组合图要求一个自定义系列入口或至少一个柱线系列集合。
        "ComboChart" => generate_combo_start(element)?,
        // 分派入口只允许两个已登记标签。
        _ => unreachable!("系列图表分派已限制标签集合"),
    };
    // 按源码顺序应用非集合专有属性。
    for attribute in &element.attributes {
        // 集合属性已经在初始构造阶段消费。
        if matches!(
            attribute.name.as_str(),
            "axes" | "series" | "barSeries" | "lineSeries"
        ) {
            // 跳过重复处理。
            continue;
        }
        // 公共 View 属性留给统一映射层。
        if !consumed.contains(&attribute.name.as_str()) {
            // 不在专有集合中的属性稍后统一处理或诊断。
            continue;
        }
        // 按属性选择精确公开构建器。
        widget = apply_chart_attribute(widget, attribute)?;
    }
    // 把现有 Chart Widget 物化为叶 View。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 应用公共尺寸、样式与自动化属性。
    apply_common_attributes(view, &element.attributes, consumed)
}

// 生成 RadarChart 的必需轴与系列构建器。
fn generate_radar_start(
    // 接收完整雷达图元素。
    element: &Element,
) -> Result<(TokenStream, &'static [&'static str]), Diagnostic> {
    // 雷达图要求显式轴定义。
    let axes_attribute = required_attribute(element, "axes")?;
    // 雷达图要求显式系列定义。
    let series_attribute = required_attribute(element, "series")?;
    // 轴定义必须是类型化表达式。
    let axes = typed_expression(axes_attribute, "RadarAxis")?;
    // 系列定义必须是类型化表达式。
    let series = typed_expression(series_attribute, "ChartSeries")?;
    // 返回精确泛型集合构造与专有属性。
    Ok((
        // 同时收集轴和雷达系列，避免运行时 Any 错配。
        quote! {
            ::uix::prelude::RadarChart::new()
                .axes(
                    ::std::iter::IntoIterator::into_iter((#axes).clone())
                        .collect::<::std::vec::Vec<::uix::prelude::RadarAxis>>()
                )
                .series(
                    ::std::iter::IntoIterator::into_iter((#series).clone())
                        .collect::<::std::vec::Vec<
                            ::uix::prelude::ChartSeries<
                                ::std::vec::Vec<::uix::prelude::RadarData>
                            >
                        >>()
                )
        },
        // 登记雷达图专有属性。
        &[
            "axes",
            "series",
            "shape",
            "gridLevels",
            "fillOpacity",
            "title",
            "subtitle",
            "responsive",
            "legend",
            "animation",
            "interactive",
            "brush",
            "tooltip",
        ],
    ))
}

// 生成 ComboChart 的自定义系列或一个到两个类型化专用系列集合。
fn generate_combo_start(
    // 接收完整组合图元素。
    element: &Element,
) -> Result<(TokenStream, &'static [&'static str]), Diagnostic> {
    // 查找可选柱系列属性。
    let bar_attribute = find_attribute(element, "barSeries");
    // 查找可选线系列属性。
    let line_attribute = find_attribute(element, "lineSeries");
    // 查找可选自定义组合系列属性。
    let custom_attribute = find_attribute(element, "series");
    // 自定义入口与专用入口不能共同拥有同一图表载荷。
    if custom_attribute.is_some() && (bar_attribute.is_some() || line_attribute.is_some()) {
        // 返回互斥数据源诊断。
        return Err(Diagnostic::new(
            // 指向完整组合图元素。
            element.span,
            // 说明三个入口的互斥关系。
            "<ComboChart> 的 series 不能与 barSeries 或 lineSeries 同时使用",
            // 给出两种合法选择。
            "单独使用 series={items}，或使用 barSeries / lineSeries 专用入口",
        ));
    }
    // 至少要有一种系列入口才能形成组合图。
    if custom_attribute.is_none() && bar_attribute.is_none() && line_attribute.is_none() {
        // 返回组合图数据源诊断。
        return Err(Diagnostic::new(
            // 指向完整组合图元素。
            element.span,
            // 说明至少一个系列入口要求。
            "<ComboChart> 至少需要 series、barSeries 或 lineSeries",
            // 给出最小合法写法。
            "使用 <ComboChart series={items} /> 或声明专用柱线系列",
        ));
    }
    // 从现有运行时组合图构造器开始。
    let mut widget = quote! { ::uix::prelude::ComboChart::new() };
    // 有自定义系列时精确收集 ComboSeries<Vec<LineData>>。
    if let Some(attribute) = custom_attribute {
        // 读取类型化 ComboSeries 集合表达式。
        let series = typed_expression(attribute, "ComboSeries")?;
        // 应用既有公开通用系列构建器。
        widget = quote! {
            (#widget).series(
                ::std::iter::IntoIterator::into_iter((#series).clone())
                    .collect::<::std::vec::Vec<
                        ::uix::prelude::ComboSeries<
                            ::std::vec::Vec<::uix::prelude::LineData>
                        >
                    >>()
            )
        };
    }
    // 有柱系列时精确收集 BarData 泛型系列。
    if let Some(attribute) = bar_attribute {
        // 读取类型化 ChartSeries 集合表达式。
        let series = typed_expression(attribute, "ChartSeries")?;
        // 应用公开柱系列构建器。
        widget = quote! {
            (#widget).bar_series(
                ::std::iter::IntoIterator::into_iter((#series).clone())
                    .collect::<::std::vec::Vec<
                        ::uix::prelude::ChartSeries<
                            ::std::vec::Vec<::uix::prelude::BarData>
                        >
                    >>()
            )
        };
    }
    // 有线系列时精确收集 LineData 泛型系列。
    if let Some(attribute) = line_attribute {
        // 读取类型化 ChartSeries 集合表达式。
        let series = typed_expression(attribute, "ChartSeries")?;
        // 应用公开线系列构建器。
        widget = quote! {
            (#widget).line_series(
                ::std::iter::IntoIterator::into_iter((#series).clone())
                    .collect::<::std::vec::Vec<
                        ::uix::prelude::ChartSeries<
                            ::std::vec::Vec<::uix::prelude::LineData>
                        >
                    >>()
            )
        };
    }
    // 返回组合图构建器与专有属性集合。
    Ok((
        // 返回已经应用集合的构建器。
        widget,
        // 登记组合图专有属性。
        &[
            "series",
            "barSeries",
            "lineSeries",
            "yAxisLeft",
            "yAxisRight",
            "title",
            "subtitle",
            "responsive",
            "legend",
            "referenceLines",
            "animation",
            "interactive",
            "brush",
            "tooltip",
        ],
    ))
}

// 读取类型化集合属性并校验内联顶层构造器。
fn typed_expression(
    // 接收待解析集合属性。
    attribute: &Attribute,
    // 接收内联数组期望构造器名。
    expected: &str,
) -> Result<TokenStream, Diagnostic> {
    // 字符串不能伪装成结构化集合。
    let AttributeValue::Expression(expression) = &attribute.value else {
        // 返回集合表达式诊断。
        return Err(Diagnostic::new(
            // 指向非法集合属性。
            attribute.span,
            // 说明目标类型化集合。
            format!("{} 必须是类型化数据表达式", attribute.name),
            // 给出目标构造器写法。
            format!("使用 {expected}(...) 数组或对应 Vec 表达式"),
        ));
    };
    // 内联数组中的已知图表构造器必须与目标一致。
    validate_inline_data(&expression.expression, expected)?;
    // 生成受限集合表达式。
    generate_expression(&expression.expression, None)
}

// 应用一个已经登记的雷达图或组合图属性。
fn apply_chart_attribute(
    // 接收前序构建器链。
    widget: TokenStream,
    // 接收当前属性。
    attribute: &Attribute,
) -> Result<TokenStream, Diagnostic> {
    // 共享属性统一投影到 ChartPlaceholder 公开 builder。
    if is_common_chart_attribute(&attribute.name) {
        // 返回共享属性映射结果。
        return apply_common_chart_attribute(widget, attribute);
    }
    // 组合图轴标题使用静态字符串。
    if matches!(attribute.name.as_str(), "yAxisLeft" | "yAxisRight") {
        // 读取静态文本。
        let value = literal_string(attribute, "图表文本属性")?;
        // 映射到精确公开构建器。
        return Ok(match attribute.name.as_str() {
            // 设置组合图左轴标题。
            "yAxisLeft" => quote! { (#widget).y_axis_left(#value) },
            // 设置组合图右轴标题。
            "yAxisRight" => quote! { (#widget).y_axis_right(#value) },
            // 文本属性集合已经穷尽。
            _ => unreachable!("系列图表文本属性集合已穷尽"),
        });
    }
    // 雷达图形状只接受静态语义值。
    if attribute.name == "shape" {
        // 读取静态枚举文本。
        let value = literal_string(attribute, "雷达图形状")?;
        // 映射到公开雷达形状枚举。
        let shape = match value.as_str() {
            // 映射多边形网格。
            "polygon" => quote! { ::uix::prelude::RadarShape::Polygon },
            // 映射圆形网格。
            "circle" => quote! { ::uix::prelude::RadarShape::Circle },
            // 其他值不在登记表。
            _ => {
                // 返回允许值诊断。
                return Err(Diagnostic::new(
                    // 指向非法形状属性。
                    attribute.span,
                    // 点名非法值。
                    format!("RadarChart shape 不支持值 '{value}'"),
                    // 给出完整允许集合。
                    "使用 polygon 或 circle",
                ));
            }
        };
        // 应用公开雷达形状构建器。
        return Ok(quote! { (#widget).shape(#shape) });
    }
    // 网格层级使用精确 usize 契约。
    if attribute.name == "gridLevels" {
        // 生成 usize 字面量或表达式。
        let value = usize_value(attribute)?;
        // 应用公开网格层级构建器。
        return Ok(quote! { (#widget).grid_levels(#value) });
    }
    // 剩余专有属性是雷达填充不透明度。
    let value = f32_value(attribute)?;
    // 应用公开填充构建器。
    Ok(quote! { (#widget).fill_opacity(#value) })
}

// 生成 usize 字面量或动态表达式。
fn usize_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状生成。
    match &attribute.value {
        // 静态数值在宏展开期验证。
        AttributeValue::Literal(source) => {
            // 解析公开 API 使用的 usize。
            let value = source.parse::<usize>().map_err(|_| {
                // 返回整数属性诊断。
                Diagnostic::new(
                    // 指向非法网格属性。
                    attribute.span,
                    // 说明整数要求。
                    "RadarChart gridLevels 必须是 usize",
                    // 给出合法示例。
                    "使用 gridLevels=\"5\" 或 usize 表达式",
                )
            })?;
            // 生成显式 usize 字面量。
            let literal = Literal::usize_unsuffixed(value);
            // 返回已经验证的层级数。
            Ok(quote! { #literal })
        }
        // 动态表达式由 Rust 核对 usize 类型。
        AttributeValue::Expression(expression) => generate_expression(&expression.expression, None),
        // 内联样式不能成为层级数。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向非法属性。
            attribute.span,
            // 说明整数形状要求。
            "RadarChart gridLevels 不能使用内联样式",
            // 给出合法写法。
            "使用整数或 usize 表达式",
        )),
    }
}

// 查找元素上的具名属性。

// 引入 i32 字面量与过程宏令牌流。
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
// 引入属性、表达式、布尔值与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, boolean_value, generate_expression,
    literal_string,
};

// 生成热力图或瀑布图的类型化静态映射。
pub(crate) fn generate_matrix_delta_chart(
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
            // 给出类型化数据写法。
            format!("使用 <{} data={{items}} />", element.name),
        ));
    }
    // 两类图表都要求显式类型化数据表达式。
    let data_attribute = required_attribute(element, "data")?;
    // 字符串不能伪装成结构化图表数据。
    let AttributeValue::Expression(data_expression) = &data_attribute.value else {
        // 返回数据表达式诊断。
        return Err(Diagnostic::new(
            // 指向非法 data 属性。
            data_attribute.span,
            // 说明当前标签需要的类型化数据。
            format!("{} data 必须是类型化数据表达式", element.name),
            // 给出本批公开数据构造器。
            "使用 HeatmapCell(...) 或 WaterfallData(...) 数组表达式",
        ));
    };
    // 按标签确定目标数据类型。
    let expected_data_type = match element.name.as_str() {
        // 热力图使用矩阵单元。
        "Heatmap" => "HeatmapCell",
        // 瀑布图使用变化项。
        "WaterfallChart" => "WaterfallData",
        // 分派入口只允许两个已登记标签。
        _ => unreachable!("矩阵与增量图表分派已限制标签集合"),
    };
    // 内联数组中的已知图表构造器必须与标签一致。
    validate_inline_data(&data_expression.expression, expected_data_type)?;
    // 生成受限数据表达式。
    let data = generate_expression(&data_expression.expression, None)?;
    // 按标签生成精确类型收集和专有属性集合。
    let (mut widget, consumed): (TokenStream, &[&str]) = match element.name.as_str() {
        // 热力图收集矩阵单元。
        "Heatmap" => (
            // 生成精确元素类型收集。
            quote! {
                ::uix_app::prelude::Heatmap::new().data(
                    ::std::iter::IntoIterator::into_iter((#data).clone())
                        .collect::<::std::vec::Vec<::uix_app::prelude::HeatmapCell>>()
                )
            },
            // 登记热力图专有属性。
            &[
                "data",
                "xLabels",
                "yLabels",
                "calendarMode",
                "year",
                "cellSize",
                "cellGap",
                "showValues",
                "colorRange",
                "colorStops",
                "title",
                "subtitle",
                "responsive",
                "legend",
                "animation",
                "interactive",
                "brush",
                "tooltip",
            ],
        ),
        // 瀑布图收集变化项。
        "WaterfallChart" => (
            // 生成精确元素类型收集。
            quote! {
                ::uix_app::prelude::WaterfallChart::new().data(
                    ::std::iter::IntoIterator::into_iter((#data).clone())
                        .collect::<::std::vec::Vec<::uix_app::prelude::WaterfallData>>()
                )
            },
            // 登记瀑布图专有属性。
            &[
                "data",
                "horizontal",
                "xAxis",
                "yAxis",
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
        ),
        // 分派入口已经限制标签集合。
        _ => unreachable!("矩阵与增量图表分派已限制标签集合"),
    };
    // 按源码顺序应用当前图表允许的专有属性。
    for attribute in &element.attributes {
        // data 已在构造阶段消费。
        if attribute.name == "data" {
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
    let view = quote! { ::uix_app::prelude::ViewNode::leaf(#widget) };
    // 应用公共尺寸、样式与自动化属性。
    apply_common_attributes(view, &element.attributes, consumed)
}

// 应用一个已经登记的热力图或瀑布图属性。
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
    // 热力图颜色配置保持精确公开类型，避免运行时 Any 接口静默忽略错型值。
    if matches!(attribute.name.as_str(), "colorRange" | "colorStops") {
        // 生成对应的颜色范围或带位置色阶。
        return apply_heatmap_color_attribute(widget, attribute);
    }
    // 坐标轴标题使用静态字符串。
    if matches!(attribute.name.as_str(), "xAxis" | "yAxis") {
        // 读取静态文本。
        let value = literal_string(attribute, "图表文本属性")?;
        // 映射到精确公开构建器。
        return Ok(match attribute.name.as_str() {
            // 设置横轴标题。
            "xAxis" => quote! { (#widget).x_axis(#value) },
            // 设置纵轴标题。
            "yAxis" => quote! { (#widget).y_axis(#value) },
            // 文本属性集合已经穷尽。
            _ => unreachable!("矩阵与增量图表文本属性集合已穷尽"),
        });
    }
    // 标签集合要求可迭代字符串表达式。
    if matches!(attribute.name.as_str(), "xLabels" | "yLabels") {
        // 字符串不能伪装成标签集合。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回标签集合诊断。
            return Err(Diagnostic::new(
                // 指向非法标签属性。
                attribute.span,
                // 说明目标类型。
                "Heatmap 标签必须是字符串集合表达式",
                // 给出合法示例。
                "使用 xLabels={['周一', '周二']} 或 Vec<String> 表达式",
            ));
        };
        // 生成受限标签集合表达式。
        let labels = generate_expression(&expression.expression, None)?;
        // 按值收集为精确 String 集合。
        let labels = quote! {
            ::std::iter::IntoIterator::into_iter((#labels).clone())
                .map(|label| ::std::convert::Into::<::std::string::String>::into(label))
                .collect::<::std::vec::Vec<::std::string::String>>()
        };
        // 映射到横向或纵向标签构建器。
        return Ok(if attribute.name == "xLabels" {
            // 设置横向标签。
            quote! { (#widget).x_labels(#labels) }
        } else {
            // 设置纵向标签。
            quote! { (#widget).y_labels(#labels) }
        });
    }
    // 布尔属性复用统一简写和表达式规则。
    if matches!(
        attribute.name.as_str(),
        "calendarMode" | "showValues" | "horizontal"
    ) {
        // 生成布尔值。
        let value = boolean_value(attribute)?;
        // 映射到精确公开构建器。
        return Ok(match attribute.name.as_str() {
            // 设置日历热力图模式。
            "calendarMode" => quote! { (#widget).calendar_mode(#value) },
            // 设置热力值文字显示。
            "showValues" => quote! { (#widget).show_values(#value) },
            // 设置横向瀑布图。
            "horizontal" => quote! { (#widget).horizontal(#value) },
            // 布尔属性集合已经穷尽。
            _ => unreachable!("矩阵与增量图表布尔属性集合已穷尽"),
        });
    }
    // 日历年份使用精确 i32 契约。
    if attribute.name == "year" {
        // 生成 i32 字面量或表达式。
        let value = i32_value(attribute)?;
        // 应用公开年份构建器。
        return Ok(quote! { (#widget).year(#value) });
    }
    // 其余专有属性都映射到 f32 构建器。
    let value = f32_value(attribute)?;
    // 映射到精确公开构建器。
    Ok(match attribute.name.as_str() {
        // 设置日历热力图单元尺寸。
        "cellSize" => quote! { (#widget).cell_size(#value) },
        // 设置热力图单元间隙。
        "cellGap" => quote! { (#widget).cell_gap(#value) },
        // 专有数值属性集合已经穷尽。
        _ => unreachable!("矩阵与增量图表数值属性集合已穷尽"),
    })
}

// 应用一个热力图自定义色阶属性。
fn apply_heatmap_color_attribute(
    // 接收前序构建器链。
    widget: TokenStream,
    // 接收颜色配置属性。
    attribute: &Attribute,
) -> Result<TokenStream, Diagnostic> {
    // 颜色配置必须通过 Rust 表达式提供类型化值。
    let AttributeValue::Expression(expression) = &attribute.value else {
        // 返回包含具体属性名的诊断。
        return Err(Diagnostic::new(
            // 指向非法颜色配置属性。
            attribute.span,
            // 说明类型化表达式要求。
            format!("Heatmap {} 必须是类型化颜色表达式", attribute.name),
            // 给出两类已登记颜色契约。
            "使用 colorRange={color_range} 或 colorStops={color_stops}",
        ));
    };
    // 生成受限 Rust 表达式。
    let value = generate_expression(&expression.expression, None)?;
    // 按属性映射到现有 Heatmap builder，并显式固定公开类型。
    Ok(match attribute.name.as_str() {
        // 两端颜色使用精确 Color 元组。
        "colorRange" => quote! {{
            let color_range: (::uix_app::prelude::Color, ::uix_app::prelude::Color) = (#value).clone();
            (#widget).color_range(color_range.0, color_range.1)
        }},
        // 多段色阶按值收集为带归一化位置的 Color 集合。
        "colorStops" => quote! {
            (#widget).color_stops(
                ::std::iter::IntoIterator::into_iter((#value).clone())
                    .collect::<::std::vec::Vec<(f32, ::uix_app::prelude::Color)>>()
            )
        },
        // 调用方已经收窄为两个颜色属性。
        _ => unreachable!("Heatmap 颜色属性集合已穷尽"),
    })
}

// 生成 i32 字面量或动态表达式。
fn i32_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状生成。
    match &attribute.value {
        // 静态数值在宏展开期验证。
        AttributeValue::Literal(source) => {
            // 解析公开 API 使用的 i32。
            let value = source.parse::<i32>().map_err(|_| {
                // 返回整数属性诊断。
                Diagnostic::new(
                    // 指向非法年份属性。
                    attribute.span,
                    // 说明整数要求。
                    "Heatmap year 必须是 i32",
                    // 给出合法示例。
                    "使用 year=\"2026\" 或 i32 表达式",
                )
            })?;
            // 生成显式 i32 字面量。
            let literal = Literal::i32_unsuffixed(value);
            // 返回已经验证的年份。
            Ok(quote! { #literal })
        }
        // 动态表达式由 Rust 核对 i32 类型。
        AttributeValue::Expression(expression) => generate_expression(&expression.expression, None),
        // 内联样式不能成为年份。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向非法属性。
            attribute.span,
            // 说明整数形状要求。
            "Heatmap year 不能使用内联样式",
            // 给出合法写法。
            "使用整数或 i32 表达式",
        )),
    }
}

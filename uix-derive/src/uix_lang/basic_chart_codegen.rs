// 引入 f32 字面量与过程宏令牌流。
use proc_macro2::{Literal, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入十二类图表共享的高级配置映射。
use super::chart_common_codegen::{apply_common_chart_attribute, is_common_chart_attribute};
// 引入属性、表达式、布尔值与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, boolean_value, generate_expression};

// 生成三类基础图表的类型化静态数据映射。
pub(crate) fn generate_basic_chart(element: &Element) -> Result<TokenStream, Diagnostic> {
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
    // 三类图表都要求显式类型化数据表达式。
    let data_attribute = required_attribute(element, "data")?;
    // 字符串不能伪装成结构化图表数据。
    let AttributeValue::Expression(data_expression) = &data_attribute.value else {
        // 返回数据表达式诊断。
        return Err(Diagnostic::new(
            // 指向非法 data 属性。
            data_attribute.span,
            // 说明当前标签需要的类型化数据。
            format!("{} data 必须是类型化数据表达式", element.name),
            // 给出三类公开数据构造器。
            "使用 BarData(...)、LineData(...) 或 PieData(...) 数组表达式",
        ));
    };
    // 生成受限数据表达式。
    let data = generate_expression(&data_expression.expression, None)?;
    // 按标签选择公开组件和元素类型。
    let (mut widget, consumed): (TokenStream, &[&str]) = match element.name.as_str() {
        // 柱状图取得 BarData 集合所有权。
        "BarChart" => (
            quote! {
                ::uix::prelude::BarChart::new().data(
                    ::std::iter::IntoIterator::into_iter((#data).clone())
                        .collect::<::std::vec::Vec<::uix::prelude::BarData>>()
                )
            },
            &[
                "data",
                "maxValue",
                "showValue",
                "barRadius",
                "grouped",
                "stacked",
                "horizontal",
                "barGap",
                "categoryGap",
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
        // 折线图取得 LineData 集合所有权。
        "LineChart" => (
            quote! {
                ::uix::prelude::LineChart::new().data(
                    ::std::iter::IntoIterator::into_iter((#data).clone())
                        .collect::<::std::vec::Vec<::uix::prelude::LineData>>()
                )
            },
            &[
                "data",
                "maxValue",
                "autoMin",
                "showGrid",
                "showDots",
                "lineWidth",
                "dotRadius",
                "smooth",
                "step",
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
        // 饼图取得 PieData 集合所有权。
        "PieChart" => (
            quote! {
                ::uix::prelude::PieChart::new().data(
                    ::std::iter::IntoIterator::into_iter((#data).clone())
                        .collect::<::std::vec::Vec<::uix::prelude::PieData>>()
                )
            },
            &[
                "data",
                "donut",
                "rose",
                "startAngle",
                "endAngle",
                "total",
                "labelVisible",
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
        // 分派入口只允许三个已登记标签。
        _ => unreachable!("基础图表分派已限制标签集合"),
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
        // 按标签和属性选择精确公开构建器。
        widget = apply_chart_attribute(widget, element, attribute)?;
    }
    // 把现有图表 Component 物化为叶 View。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 应用公共尺寸、样式与自动化属性。
    apply_common_attributes(view, &element.attributes, consumed)
}

// 应用一个已经登记的图表专有属性。
fn apply_chart_attribute(
    // 接收前序构建器链。
    widget: TokenStream,
    // 接收完整元素以区分图表种类。
    element: &Element,
    // 接收当前属性。
    attribute: &Attribute,
) -> Result<TokenStream, Diagnostic> {
    // 共享属性统一投影到 ChartPlaceholder 公开 builder。
    if is_common_chart_attribute(&attribute.name) {
        // 返回共享属性映射结果。
        return apply_common_chart_attribute(widget, attribute);
    }
    // 布尔属性复用统一简写和表达式规则。
    if matches!(
        attribute.name.as_str(),
        "showValue"
            | "grouped"
            | "stacked"
            | "horizontal"
            | "autoMin"
            | "showGrid"
            | "showDots"
            | "smooth"
            | "step"
            | "rose"
            | "labelVisible"
    ) {
        // 生成布尔值。
        let value = boolean_value(attribute)?;
        // 映射到同名或语义等价公开构建器。
        return Ok(match attribute.name.as_str() {
            // 柱值标签开关。
            "showValue" => quote! { (#widget).show_value(#value) },
            // 分组柱开关。
            "grouped" => quote! { (#widget).grouped(#value) },
            // 堆叠柱开关。
            "stacked" => quote! { (#widget).stacked(#value) },
            // 横向柱开关。
            "horizontal" => quote! { (#widget).horizontal(#value) },
            // 折线自动下界开关。
            "autoMin" => quote! { (#widget).auto_min(#value) },
            // 折线网格开关。
            "showGrid" => quote! { (#widget).show_grid(#value) },
            // 折线数据点开关。
            "showDots" => quote! { (#widget).show_dots(#value) },
            // 平滑折线开关。
            "smooth" => quote! { (#widget).smooth(#value) },
            // 阶梯折线开关。
            "step" => quote! { (#widget).step(#value) },
            // 玫瑰图开关。
            "rose" => quote! { (#widget).rose(#value) },
            // 饼图标签开关。
            "labelVisible" => quote! { (#widget).label_visible(#value) },
            // 布尔集合已经穷尽。
            _ => unreachable!("图表布尔属性集合已穷尽"),
        });
    }
    // 其余专有属性都映射到 f32 构建器。
    let value = f32_value(attribute)?;
    // 按精确名称调用公开运行时方法。
    Ok(match (element.name.as_str(), attribute.name.as_str()) {
        // 两类坐标图共享最大值上限。
        ("BarChart" | "LineChart", "maxValue") => quote! { (#widget).max_value(#value) },
        // 柱形圆角。
        ("BarChart", "barRadius") => quote! { (#widget).bar_radius(#value) },
        // 组内柱间距。
        ("BarChart", "barGap") => quote! { (#widget).bar_gap(#value) },
        // 分类间距。
        ("BarChart", "categoryGap") => quote! { (#widget).category_gap(#value) },
        // 折线宽度。
        ("LineChart", "lineWidth") => quote! { (#widget).line_width(#value) },
        // 折线数据点半径。
        ("LineChart", "dotRadius") => quote! { (#widget).dot_radius(#value) },
        // 环形内径比例。
        ("PieChart", "donut") => quote! { (#widget).donut(#value) },
        // 饼图起始角度。
        ("PieChart", "startAngle") => quote! { (#widget).start_angle(#value) },
        // 饼图结束角度。
        ("PieChart", "endAngle") => quote! { (#widget).end_angle(#value) },
        // 饼图显式总量。
        ("PieChart", "total") => quote! { (#widget).total(#value) },
        // 已登记集合与图表种类不匹配属于内部错误。
        _ => unreachable!("图表专有属性已按标签登记"),
    })
}

// 生成 f32 字面量或动态表达式。
pub(super) fn f32_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状生成。
    match &attribute.value {
        // 静态数值在宏展开期验证。
        AttributeValue::Literal(source) => {
            // 解析公开 API 使用的 f32。
            let value = source.parse::<f32>().map_err(|_| {
                // 返回数值属性诊断。
                Diagnostic::new(
                    attribute.span,
                    "图表数值属性必须是 f32",
                    "使用有限数值或 f32 表达式",
                )
            })?;
            // 静态非有限值不得进入声明。
            if !value.is_finite() {
                // 返回有限性诊断。
                return Err(Diagnostic::new(
                    attribute.span,
                    "图表数值属性必须有限",
                    "使用有限 f32 数值",
                ));
            }
            // 生成无后缀 f32 字面量。
            let literal = Literal::f32_unsuffixed(value);
            // 返回已验证字面量。
            Ok(quote! { #literal })
        }
        // 动态表达式由 Rust 核对 f32 类型。
        AttributeValue::Expression(expression) => generate_expression(&expression.expression, None),
        // 内联样式不能成为组件数值。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向非法属性。
            attribute.span,
            // 说明数值形状要求。
            "图表数值属性不能使用内联样式",
            // 给出合法写法。
            "使用数值字面量或 f32 表达式",
        )),
    }
}

// 查找元素上的具名属性。
fn find_attribute<'a>(element: &'a Element, name: &str) -> Option<&'a Attribute> {
    // 解析器已经保证同名属性唯一。
    element
        .attributes
        .iter()
        .find(|attribute| attribute.name == name)
}

// 查找基础图表必需属性。
pub(super) fn required_attribute<'a>(
    // 接收需要查询的图表元素。
    element: &'a Element,
    // 接收必需属性名。
    name: &str,
) -> Result<&'a Attribute, Diagnostic> {
    // 缺失数据时返回确定诊断。
    find_attribute(element, name).ok_or_else(|| {
        // 点名当前图表与缺失属性。
        Diagnostic::new(
            // 指向完整元素。
            element.span,
            // 说明必需属性。
            format!("<{}> 缺少必需的 {name} 属性", element.name),
            // 给出最小合法写法。
            format!("使用 <{} data={{items}} />", element.name),
        )
    })
}

// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 复用基础图表的数值与必填属性校验。
use super::basic_chart_codegen::{f32_value, required_attribute};
// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入十二类图表共享的高级配置映射。
use super::chart_common_codegen::{apply_common_chart_attribute, is_common_chart_attribute};
// 引入属性、表达式、布尔值与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, Expression, ExpressionKind, boolean_value,
    data_chain_root, generate_expression, literal_string,
};

// 保存当前图表数据构造器集合，用于拒绝标签间的内联类型错配。
const CHART_DATA_CONSTRUCTORS: &[&str] = &[
    // 柱状图数据。
    "BarData",
    // 折线与面积图数据。
    "LineData",
    // 饼图数据。
    "PieData",
    // 散点图数据。
    "ScatterData",
    // 气泡图数据。
    "BubbleData",
    // 漏斗图数据。
    "FunnelData",
    // 矩形树图节点。
    "TreemapNode",
    // 仪表盘色带。
    "GaugeRange",
    // 热力图矩阵单元。
    "HeatmapCell",
    // 瀑布图变化项。
    "WaterfallData",
    // 通用图表系列。
    "ChartSeries",
    // 雷达图维度轴。
    "RadarAxis",
    // 雷达图维度值。
    "RadarData",
];

// 生成面积、散点与漏斗三类静态图表映射。
pub(crate) fn generate_static_chart(element: &Element) -> Result<TokenStream, Diagnostic> {
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
    // 解析当前标签唯一且精确的数据入口。
    let (data_attribute, expected_data_type) = chart_data_contract(element)?;
    // 字符串不能伪装成结构化图表数据。
    let AttributeValue::Expression(data_expression) = &data_attribute.value else {
        // 返回数据表达式诊断。
        return Err(Diagnostic::new(
            // 指向非法 data 属性。
            data_attribute.span,
            // 说明当前标签需要的类型化数据。
            format!(
                "{} {} 必须是类型化数据表达式",
                element.name, data_attribute.name
            ),
            // 给出本批公开数据构造器。
            "使用 LineData(...)、ScatterData(...)、BubbleData(...) 或 FunnelData(...) 数组表达式",
        ));
    };
    // 内联数组中的已知图表构造器必须与标签一致。
    validate_inline_data(&data_expression.expression, expected_data_type)?;
    // 生成受限数据表达式。
    let data = generate_expression(&data_expression.expression, None)?;
    // 按标签选择公开组件和精确元素类型。
    let (mut widget, consumed): (TokenStream, &[&str]) = match element.name.as_str() {
        // 面积图取得 LineData 集合所有权。
        "AreaChart" => (
            // 生成精确类型收集，避免运行时泛型 downcast 静默失败。
            quote! {
                ::uix::prelude::AreaChart::new().data(
                    ::std::iter::IntoIterator::into_iter((#data).clone())
                        .collect::<::std::vec::Vec<::uix::prelude::LineData>>()
                )
            },
            // 登记面积图专有属性。
            &[
                "data",
                "stacked",
                "smooth",
                "step",
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
        ),
        // 散点图按显式入口取得 ScatterData 或 BubbleData 集合所有权。
        "ScatterChart" => {
            // 按数据契约选择精确集合类型，避免 Any 下转失败静默为空图。
            let widget = if expected_data_type == "BubbleData" {
                // 气泡入口精确收集 BubbleData。
                quote! {
                    ::uix::prelude::ScatterChart::new().data(
                        ::std::iter::IntoIterator::into_iter((#data).clone())
                            .collect::<::std::vec::Vec<::uix::prelude::BubbleData>>()
                    )
                }
            } else {
                // 普通散点入口精确收集 ScatterData。
                quote! {
                    ::uix::prelude::ScatterChart::new().data(
                        ::std::iter::IntoIterator::into_iter((#data).clone())
                            .collect::<::std::vec::Vec<::uix::prelude::ScatterData>>()
                    )
                }
            };
            // 返回构造链与完整属性白名单。
            (
                widget,
                // 登记散点图与气泡图专有属性。
                &[
                    "data",
                    "bubbleData",
                    "xAxis",
                    "yAxis",
                    "pointSize",
                    "pointStyle",
                    "bubbleScale",
                    "title",
                    "subtitle",
                    "responsive",
                    "legend",
                    "animation",
                    "interactive",
                    "brush",
                    "tooltip",
                ],
            )
        }
        // 漏斗图取得 FunnelData 集合所有权。
        "FunnelChart" => (
            // 生成精确类型收集。
            quote! {
                ::uix::prelude::FunnelChart::new().data(
                    ::std::iter::IntoIterator::into_iter((#data).clone())
                        .collect::<::std::vec::Vec<::uix::prelude::FunnelData>>()
                )
            },
            // 登记漏斗图专有属性。
            &[
                "data",
                "showConversionRate",
                "labelVisible",
                "labelPosition",
                "align",
                "shape",
                "gap",
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
        _ => unreachable!("静态高级图表分派已限制标签集合"),
    };
    // 按源码顺序应用当前图表允许的专有属性。
    for attribute in &element.attributes {
        // 两类数据入口已在构造阶段消费。
        if matches!(attribute.name.as_str(), "data" | "bubbleData") {
            // 跳过重复处理。
            continue;
        }
        // 公共 View 属性留给统一映射层。
        if !consumed.contains(&attribute.name.as_str()) {
            // 不在专有集合中的属性稍后统一处理或诊断。
            continue;
        }
        // 按标签和属性选择精确公开构建器。
        widget = apply_chart_attribute(widget, attribute)?;
    }
    // 把现有 Chart Component 物化为叶 View。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 应用公共尺寸、样式与自动化属性。
    apply_common_attributes(view, &element.attributes, consumed)
}

// 解析每类静态图表的唯一类型化数据入口。
fn chart_data_contract<'a>(
    // 接收完整图表元素。
    element: &'a Element,
) -> Result<(&'a Attribute, &'static str), Diagnostic> {
    // 非散点标签继续要求既有 data 属性。
    if element.name != "ScatterChart" {
        // 按标签返回既有精确元素类型。
        return Ok(match element.name.as_str() {
            // 面积图复用折线数据。
            "AreaChart" => (required_attribute(element, "data")?, "LineData"),
            // 漏斗图使用阶段数据。
            "FunnelChart" => (required_attribute(element, "data")?, "FunnelData"),
            // 分派入口只允许三个已登记标签。
            _ => unreachable!("静态高级图表分派已限制标签集合"),
        });
    }
    // 查找普通散点数据入口。
    let scatter = element
        .attributes
        .iter()
        .find(|attribute| attribute.name == "data");
    // 查找气泡数据入口。
    let bubble = element
        .attributes
        .iter()
        .find(|attribute| attribute.name == "bubbleData");
    // 两个数据入口必须恰好选择一个。
    let (attribute, expected) = match (scatter, bubble) {
        // 普通散点入口。
        (Some(attribute), None) => (attribute, "ScatterData"),
        // 气泡图入口。
        (None, Some(attribute)) => (attribute, "BubbleData"),
        // 同时提供会产生不明确的运行时载荷。
        (Some(_), Some(attribute)) => {
            // 返回互斥诊断。
            return Err(Diagnostic::new(
                // 指向第二个数据入口。
                attribute.span,
                // 说明互斥契约。
                "ScatterChart data 与 bubbleData 不能同时使用",
                // 给出两种精确选择。
                "普通散点使用 data={items}；气泡图使用 bubbleData={items}",
            ));
        }
        // 缺少两个入口时没有可绘制载荷。
        (None, None) => {
            // 返回必需入口诊断。
            return Err(Diagnostic::new(
                // 指向完整元素。
                element.span,
                // 说明至少需要一个入口。
                "<ScatterChart> 缺少 data 或 bubbleData 属性",
                // 给出两种最小合法写法。
                "使用 <ScatterChart data={items} /> 或 <ScatterChart bubbleData={items} />",
            ));
        }
    };
    // 气泡缩放不能成为普通散点的无效配置。
    if expected == "ScatterData"
        && let Some(scale) = element
            .attributes
            .iter()
            .find(|attribute| attribute.name == "bubbleScale")
    {
        // 返回数据入口与配置不匹配诊断。
        return Err(Diagnostic::new(
            // 指向无效缩放属性。
            scale.span,
            // 说明气泡缩放的数据前提。
            "ScatterChart bubbleScale 仅适用于 bubbleData",
            // 给出修复路径。
            "改用 bubbleData={items}，或移除 bubbleScale",
        ));
    }
    // 点大小与点形状不能成为气泡图的无效配置。
    if expected == "BubbleData"
        && let Some(point) = element.attributes.iter().find(|attribute| {
            // 查找普通散点专有外观属性。
            matches!(attribute.name.as_str(), "pointSize" | "pointStyle")
        })
    {
        // 返回数据入口与配置不匹配诊断。
        return Err(Diagnostic::new(
            // 指向无效散点属性。
            point.span,
            // 说明气泡大小的唯一来源。
            format!("ScatterChart {} 不适用于 bubbleData", point.name),
            // 给出气泡缩放入口。
            "使用 BubbleData 的 size 参数与 bubbleScale",
        ));
    }
    // 返回唯一入口及其精确元素类型。
    Ok((attribute, expected))
}

// 验证内联数组没有借用其他图表的数据构造器。
pub(super) fn validate_inline_data(
    // 接收可能是内联数组的数据表达式。
    expression: &Expression,
    // 接收当前图表要求的数据构造器名。
    expected: &str,
) -> Result<(), Diagnostic> {
    // 动态集合表达式继续交给 Rust 精确核对元素类型。
    let ExpressionKind::Array(items) = &expression.kind else {
        // 非数组不需要生成层结构检查。
        return Ok(());
    };
    // 逐项核对显式图表数据构造链。
    for item in items {
        // 只约束已知图表数据构造器，不阻止返回目标类型的业务 helper。
        let Some(actual) =
            data_chain_root(item).filter(|name| CHART_DATA_CONSTRUCTORS.contains(name))
        else {
            // 普通变量或业务 helper 由 Rust 类型系统检查。
            continue;
        };
        // 正确构造器继续处理下一项。
        if actual == expected {
            // 当前项类型正确。
            continue;
        }
        // 返回目标类型明确诊断。
        return Err(Diagnostic::new(
            // 指向错配的数据项。
            item.span,
            // 点名实际与期望构造器。
            format!("当前图表 data 需要 {expected}，不能使用 {actual}"),
            // 给出目标构造器修复建议。
            format!("改用 {expected}(...)，或传入 Vec<{expected}> 表达式"),
        ));
    }
    // 全部显式数据项均符合目标类型。
    Ok(())
}

// 应用一个已经登记的静态图表专有属性。
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
            _ => unreachable!("静态图表文本属性集合已穷尽"),
        });
    }
    // 布尔属性复用统一简写和表达式规则。
    if matches!(
        attribute.name.as_str(),
        "stacked" | "smooth" | "step" | "showConversionRate" | "labelVisible"
    ) {
        // 生成布尔值。
        let value = boolean_value(attribute)?;
        // 映射到精确公开构建器。
        return Ok(match attribute.name.as_str() {
            // 设置面积堆叠。
            "stacked" => quote! { (#widget).stacked(#value) },
            // 设置平滑曲线。
            "smooth" => quote! { (#widget).smooth(#value) },
            // 设置阶梯曲线。
            "step" => quote! { (#widget).step(#value) },
            // 设置漏斗相邻阶段转化率。
            "showConversionRate" => quote! { (#widget).show_conversion_rate(#value) },
            // 设置漏斗数据标签可见性。
            "labelVisible" => quote! { (#widget).label_visible(#value) },
            // 布尔属性集合已经穷尽。
            _ => unreachable!("静态图表布尔属性集合已穷尽"),
        });
    }
    // 枚举属性只接受文档登记的静态语义值。
    if matches!(
        attribute.name.as_str(),
        "pointStyle" | "labelPosition" | "align" | "shape"
    ) {
        // 读取静态枚举文本。
        let value = literal_string(attribute, "图表枚举属性")?;
        // 解析并应用精确枚举路径。
        return apply_enum_attribute(widget, attribute, &value);
    }
    // 其余专有属性都映射到 f32 构建器。
    let value = f32_value(attribute)?;
    // 映射到精确公开构建器。
    Ok(match attribute.name.as_str() {
        // 设置面积填充不透明度。
        "fillOpacity" => quote! { (#widget).fill_opacity(#value) },
        // 设置散点大小。
        "pointSize" => quote! { (#widget).point_size(#value) },
        // 设置气泡大小缩放系数。
        "bubbleScale" => quote! { (#widget).bubble_scale(#value) },
        // 设置漏斗层级间隙。
        "gap" => quote! { (#widget).gap(#value) },
        // 专有数值属性集合已经穷尽。
        _ => unreachable!("静态图表数值属性集合已穷尽"),
    })
}

// 应用一个文档登记的图表枚举属性。
fn apply_enum_attribute(
    // 接收前序构建器链。
    widget: TokenStream,
    // 接收当前属性以生成精确诊断。
    attribute: &Attribute,
    // 接收已经解析的静态值。
    value: &str,
) -> Result<TokenStream, Diagnostic> {
    // 按属性名和值选择公开枚举路径。
    let mapped = match (attribute.name.as_str(), value) {
        // 映射圆形散点。
        ("pointStyle", "circle") => quote! { ::uix::prelude::PointStyle::Circle },
        // 映射菱形散点。
        ("pointStyle", "diamond") => quote! { ::uix::prelude::PointStyle::Diamond },
        // 映射十字散点。
        ("pointStyle", "cross") => quote! { ::uix::prelude::PointStyle::Cross },
        // 映射内部标签。
        ("labelPosition", "inside") => quote! { ::uix::prelude::LabelPosition::Inside },
        // 映射外部标签。
        ("labelPosition", "outside") => quote! { ::uix::prelude::LabelPosition::Outside },
        // 映射右侧标签。
        ("labelPosition", "right") => quote! { ::uix::prelude::LabelPosition::Right },
        // 映射居中漏斗。
        ("align", "center") => quote! { ::uix::prelude::FunnelAlign::Center },
        // 映射左对齐漏斗。
        ("align", "left") => quote! { ::uix::prelude::FunnelAlign::Left },
        // 映射右对齐漏斗。
        ("align", "right") => quote! { ::uix::prelude::FunnelAlign::Right },
        // 映射常规漏斗形状。
        ("shape", "normal") => quote! { ::uix::prelude::FunnelShape::Normal },
        // 映射对称漏斗形状。
        ("shape", "symmetric") => quote! { ::uix::prelude::FunnelShape::Symmetric },
        // 其他组合不在登记表。
        _ => {
            // 返回允许值诊断。
            return Err(Diagnostic::new(
                // 指向非法枚举属性。
                attribute.span,
                // 点名属性和值。
                format!("图表 {} 不支持值 '{value}'", attribute.name),
                // 给出各枚举的完整允许集合。
                "pointStyle 使用 circle/diamond/cross；labelPosition 使用 inside/outside/right；align 使用 center/left/right；shape 使用 normal/symmetric",
            ));
        }
    };
    // 调用对应公开构建器。
    Ok(match attribute.name.as_str() {
        // 设置散点形状。
        "pointStyle" => quote! { (#widget).point_style(#mapped) },
        // 设置漏斗标签位置。
        "labelPosition" => quote! { (#widget).label_position(#mapped) },
        // 设置漏斗对齐方式。
        "align" => quote! { (#widget).align(#mapped) },
        // 设置漏斗几何形状。
        "shape" => quote! { (#widget).shape(#mapped) },
        // 枚举属性集合已经穷尽。
        _ => unreachable!("静态图表枚举属性集合已穷尽"),
    })
}

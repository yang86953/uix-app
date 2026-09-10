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
// 复用内联图表数据构造器错配校验。
use super::static_chart_codegen::validate_inline_data;
// 引入属性、表达式、布尔值与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, boolean_value, generate_expression,
    literal_string,
};

// 生成矩形树图或仪表盘的类型化静态映射。
pub(crate) fn generate_hierarchy_gauge_chart(
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
        // 矩形树图要求显式类型化层级数据。
        "Treemap" => generate_treemap_start(element)?,
        // 仪表盘要求显式当前值。
        "Gauge" => generate_gauge_start(element)?,
        // 分派入口只允许两个已登记标签。
        _ => unreachable!("层级与仪表图表分派已限制标签集合"),
    };
    // 按源码顺序应用当前图表允许的专有属性。
    for attribute in &element.attributes {
        // 初始构造阶段已经消费必需属性。
        if matches!(
            (element.name.as_str(), attribute.name.as_str()),
            ("Treemap", "data") | ("Gauge", "value")
        ) {
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
    // 把现有 Chart Widget 物化为叶 View。
    let view = quote! { ::uix_app::prelude::ViewNode::leaf(#widget) };
    // 应用公共尺寸、样式与自动化属性。
    apply_common_attributes(view, &element.attributes, consumed)
}

// 生成矩形树图的类型化初始构建器。
fn generate_treemap_start(
    // 接收完整矩形树图元素。
    element: &Element,
) -> Result<(TokenStream, &'static [&'static str]), Diagnostic> {
    // 矩形树图要求显式数据表达式。
    let data_attribute = required_attribute(element, "data")?;
    // 字符串不能伪装成层级节点集合。
    let AttributeValue::Expression(data_expression) = &data_attribute.value else {
        // 返回数据表达式诊断。
        return Err(Diagnostic::new(
            // 指向非法 data 属性。
            data_attribute.span,
            // 说明目标类型。
            "Treemap data 必须是类型化数据表达式",
            // 给出公开数据构造器。
            "使用 TreemapNode(...) 数组或 Vec<TreemapNode> 表达式",
        ));
    };
    // 内联数组只允许矩形树节点构造器。
    validate_inline_data(&data_expression.expression, "TreemapNode")?;
    // 生成受限数据表达式。
    let data = generate_expression(&data_expression.expression, None)?;
    // 返回精确元素收集和专有属性集合。
    Ok((
        // 收集为运行时公开节点类型。
        quote! {
            ::uix_app::prelude::Treemap::new().data(
                ::std::iter::IntoIterator::into_iter((#data).clone())
                    .collect::<::std::vec::Vec<::uix_app::prelude::TreemapNode>>()
            )
        },
        // 登记矩形树图专有属性。
        &[
            "data",
            "gap",
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
    ))
}

// 生成仪表盘的必需当前值构建器。
fn generate_gauge_start(
    // 接收完整仪表盘元素。
    element: &Element,
) -> Result<(TokenStream, &'static [&'static str]), Diagnostic> {
    // 仪表盘要求显式当前值。
    let value_attribute = required_attribute(element, "value")?;
    // 生成已经验证的 f32 当前值。
    let value = f32_value(value_attribute)?;
    // 返回仪表盘构造器与专有属性集合。
    Ok((
        // 设置必需当前值并固定仪表盘类型。
        quote! { ::uix_app::prelude::Gauge::new().value(#value) },
        // 登记仪表盘专有属性。
        &[
            "value",
            "min",
            "max",
            "rangeColors",
            "gaugeType",
            "pointerWidth",
            "pointerColor",
            // 登记类型化中心文本格式化器。
            "format",
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

// 应用一个已经登记的矩形树图或仪表盘属性。
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
    // 标签可见性复用统一表达式规则。
    if attribute.name == "labelVisible" {
        // 生成布尔值。
        let value = boolean_value(attribute)?;
        // 设置矩形树图标签可见性。
        return Ok(quote! { (#widget).label_visible(#value) });
    }
    // 仪表盘类型只接受静态语义值。
    if attribute.name == "gaugeType" {
        // 读取静态枚举文本。
        let value = literal_string(attribute, "仪表盘类型")?;
        // 映射到公开枚举路径。
        let gauge_type = match value.as_str() {
            // 映射半圆仪表盘。
            "dashboard" => quote! { ::uix_app::prelude::GaugeType::Dashboard },
            // 映射整圆仪表盘。
            "full" => quote! { ::uix_app::prelude::GaugeType::Full },
            // 映射环形仪表盘。
            "ring" => quote! { ::uix_app::prelude::GaugeType::Ring },
            // 其他值不在登记表。
            _ => {
                // 返回允许值诊断。
                return Err(Diagnostic::new(
                    // 指向非法类型属性。
                    attribute.span,
                    // 点名非法值。
                    format!("Gauge gaugeType 不支持值 '{value}'"),
                    // 给出完整允许集合。
                    "使用 dashboard、full 或 ring",
                ));
            }
        };
        // 应用公开仪表盘类型构建器。
        return Ok(quote! { (#widget).gauge_type(#gauge_type) });
    }
    // 色带集合要求类型化表达式。
    if attribute.name == "rangeColors" {
        // 字符串不能伪装成色带集合。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回色带表达式诊断。
            return Err(Diagnostic::new(
                // 指向非法色带属性。
                attribute.span,
                // 说明目标类型。
                "Gauge rangeColors 必须是类型化数据表达式",
                // 给出公开数据构造器。
                "使用 GaugeRange(...) 数组或 Vec<GaugeRange> 表达式",
            ));
        };
        // 内联数组只允许仪表盘色带构造器。
        validate_inline_data(&expression.expression, "GaugeRange")?;
        // 生成受限色带表达式。
        let ranges = generate_expression(&expression.expression, None)?;
        // 收集为运行时公开色带类型。
        return Ok(quote! {
            (#widget).range_colors(
                ::std::iter::IntoIterator::into_iter((#ranges).clone())
                    .collect::<::std::vec::Vec<::uix_app::prelude::GaugeRange>>()
            )
        });
    }
    // 指针颜色要求类型化 Color 表达式。
    if attribute.name == "pointerColor" {
        // 字符串不能隐式改变颜色解析边界。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回颜色表达式诊断。
            return Err(Diagnostic::new(
                // 指向非法颜色属性。
                attribute.span,
                // 说明目标类型。
                "Gauge pointerColor 必须是 Color 表达式",
                // 给出公开颜色构造器。
                "使用 pointerColor={Color('#1677ff')} 或 Color 变量",
            ));
        };
        // 生成受限颜色表达式。
        let color = generate_expression(&expression.expression, None)?;
        // 应用公开指针颜色构建器。
        return Ok(quote! { (#widget).pointer_color(#color) });
    }
    // 格式化器要求类型化 Rust 表达式并由声明快照克隆。
    if attribute.name == "format" {
        // 字符串不能伪装成可调用格式化器。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回类型化格式化器诊断。
            return Err(Diagnostic::new(
                // 指向非法格式化器属性。
                attribute.span,
                // 说明目标契约。
                "Gauge format 必须是类型化格式化器表达式",
                // 给出公开 Rust 函数边界。
                "使用 format={formatter}，由 Rust 核对 Fn(f32) -> String + Clone + 'static",
            ));
        };
        // 生成受限格式化器表达式。
        let formatter = generate_expression(&expression.expression, None)?;
        // 克隆声明快照后交给运行时图表模块持有。
        return Ok(quote! { (#widget).format((#formatter).clone()) });
    }
    // 其余专有属性都映射到 f32 构建器。
    let value = f32_value(attribute)?;
    // 映射到精确公开构建器。
    Ok(match attribute.name.as_str() {
        // 设置矩形树图节点间隙。
        "gap" => quote! { (#widget).gap(#value) },
        // 设置仪表盘最小值。
        "min" => quote! { (#widget).min(#value) },
        // 设置仪表盘最大值。
        "max" => quote! { (#widget).max(#value) },
        // 设置仪表盘指针宽度。
        "pointerWidth" => quote! { (#widget).pointer_width(#value) },
        // 专有数值属性集合已经穷尽。
        _ => unreachable!("层级与仪表图表数值属性集合已穷尽"),
    })
}

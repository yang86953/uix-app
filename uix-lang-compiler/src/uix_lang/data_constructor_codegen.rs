// 引入过程宏标识符与令牌流。
use proc_macro2::{Ident, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入表达式核心的递归生成入口。
use super::expression_codegen::generate_expression_inner;
// 引入数据构造规格与数字规范化能力。
use super::{
    CallArgument, DataConstructorSpec, Diagnostic, ExpressionKind, SourceSpan,
    data_constructor_spec, normalize_number_literals,
};

// 尝试生成一个已登记语言数据构造器，普通回调返回 None。
pub(super) fn generate_data_constructor(
    // 接收直接调用名称。
    name: &str,
    // 接收源码顺序中的参数。
    arguments: &[CallArgument],
    // 接收完整调用跨度。
    span: SourceSpan,
    // 接收可选事件载荷变量。
    event: Option<&Ident>,
) -> Option<Result<TokenStream, Diagnostic>> {
    // HeatmapCell 使用混合 usize/usize/f32 参数规则。
    if name == "HeatmapCell" {
        // 返回专用构造结果。
        return Some(generate_heatmap_cell(arguments, span, event));
    }
    // WaterfallData 使用字符串到公开枚举的规则。
    if name == "WaterfallData" {
        // 返回专用构造结果。
        return Some(generate_waterfall_data(arguments, span, event));
    }
    // RadarAxis 把最小/最大值组合为运行时闭区间。
    if name == "RadarAxis" {
        // 返回专用构造结果。
        return Some(generate_radar_axis(arguments, span, event));
    }
    // 查找普通数据类型的公开构造规格。
    let spec = data_constructor_spec(name)?;
    // 返回普通已登记构造结果。
    Some(generate_registered_data(arguments, event, spec))
}

// 生成 RadarAxis(label, min, max) 到闭区间的公开构造调用。
fn generate_radar_axis(
    // 接收源码顺序中的三个参数。
    arguments: &[CallArgument],
    // 接收完整调用跨度。
    span: SourceSpan,
    // 接收可选事件载荷变量。
    event: Option<&Ident>,
) -> Result<TokenStream, Diagnostic> {
    // 必须严格使用三个位置参数。
    if arguments.len() != 3 || arguments.iter().any(|argument| argument.name.is_some()) {
        // 返回精确参数形状诊断。
        return Err(Diagnostic::new(
            // 指向完整构造调用。
            span,
            // 说明参数要求。
            "RadarAxis 必须接收 label、min、max 三个位置参数",
            // 给出合法示例。
            "使用 RadarAxis('速度', 0, 100)",
        ));
    }
    // 生成维度标签表达式。
    let label = generate_expression_inner(&arguments[0].value, event)?;
    // 复制最小值以进行 f32 字面量规范化。
    let mut min = arguments[1].value.clone();
    // 规范化最小值中的整数数字。
    normalize_number_literals(&mut min);
    // 生成规范化后的最小值。
    let min = generate_expression_inner(&min, event)?;
    // 复制最大值以进行 f32 字面量规范化。
    let mut max = arguments[2].value.clone();
    // 规范化最大值中的整数数字。
    normalize_number_literals(&mut max);
    // 生成规范化后的最大值。
    let max = generate_expression_inner(&max, event)?;
    // 调用公开雷达轴构造器并形成闭区间。
    Ok(quote! { ::uix_app::prelude::RadarAxis::new(#label, (#min)..=(#max)) })
}

// 生成普通已登记数据类型的公开构造调用。
fn generate_registered_data(
    // 接收源码顺序中的参数。
    arguments: &[CallArgument],
    // 接收可选事件载荷变量。
    event: Option<&Ident>,
    // 接收公开构造规格。
    spec: DataConstructorSpec,
) -> Result<TokenStream, Diagnostic> {
    // 解构构造规格。
    let DataConstructorSpec {
        // 取出公开路径。
        path,
        // 取出可选构造函数名。
        method,
        // 取出数字规范化策略。
        normalize_numbers,
    } = spec;
    // 生成全部位置参数。
    let arguments = arguments
        // 遍历有序参数。
        .iter()
        // 转换每个参数表达式。
        .map(|argument| {
            // 复制参数值以支持数字形状规范化。
            let mut value = argument.value.clone();
            // 坐标等 f32 参数把整数规范化为小数形状。
            if normalize_numbers {
                // 规范化参数中的整数数字。
                normalize_number_literals(&mut value);
            }
            // 生成规范化后的参数。
            generate_expression_inner(&value, event)
        })
        // 收集或返回首个诊断。
        .collect::<Result<Vec<_>, _>>()?;
    // 选择公开构造函数（默认 new）。
    let method = method
        // 缺省映射为 new。
        .unwrap_or_else(|| Ident::new("new", proc_macro2::Span::mixed_site()));
    // 生成公开构造调用。
    Ok(quote! { #path::#method(#(#arguments),*) })
}

// 生成 HeatmapCell(x: usize, y: usize, value: f32) 混合参数构造。
fn generate_heatmap_cell(
    // 接收源码顺序中的三个参数。
    arguments: &[CallArgument],
    // 接收完整调用跨度。
    span: SourceSpan,
    // 接收可选事件载荷变量。
    event: Option<&Ident>,
) -> Result<TokenStream, Diagnostic> {
    // 必须严格使用三个位置参数。
    if arguments.len() != 3 || arguments.iter().any(|argument| argument.name.is_some()) {
        // 返回精确参数形状诊断。
        return Err(Diagnostic::new(
            // 指向完整构造调用。
            span,
            // 说明混合类型参数要求。
            "HeatmapCell 必须接收 x、y、value 三个位置参数",
            // 给出合法示例。
            "使用 HeatmapCell(0, 1, 12.0)",
        ));
    }
    // 两个坐标索引保持原始整数形状。
    let x = generate_expression_inner(&arguments[0].value, event)?;
    // 生成纵向索引。
    let y = generate_expression_inner(&arguments[1].value, event)?;
    // 复制第三参数以进行 f32 字面量规范化。
    let mut value = arguments[2].value.clone();
    // 只把 value 中的整数数字补为小数形状。
    normalize_number_literals(&mut value);
    // 生成规范化后的热力值。
    let value = generate_expression_inner(&value, event)?;
    // 调用公开热力图单元构造器。
    Ok(quote! { ::uix_app::prelude::HeatmapCell::new(#x, #y, #value) })
}

// 生成 WaterfallData(label, value, kind) 枚举语义构造。
fn generate_waterfall_data(
    // 接收源码顺序中的三个参数。
    arguments: &[CallArgument],
    // 接收完整调用跨度。
    span: SourceSpan,
    // 接收可选事件载荷变量。
    event: Option<&Ident>,
) -> Result<TokenStream, Diagnostic> {
    // 必须严格使用三个位置参数。
    if arguments.len() != 3 || arguments.iter().any(|argument| argument.name.is_some()) {
        // 返回精确参数形状诊断。
        return Err(Diagnostic::new(
            // 指向完整构造调用。
            span,
            // 说明参数要求。
            "WaterfallData 必须接收 label、value、kind 三个位置参数",
            // 给出合法示例。
            "使用 WaterfallData('收入', 500, 'increase')",
        ));
    }
    // 生成文本标签表达式。
    let label = generate_expression_inner(&arguments[0].value, event)?;
    // 复制数值参数以进行 f32 字面量规范化。
    let mut value = arguments[1].value.clone();
    // 把 value 中的整数数字补为小数形状。
    normalize_number_literals(&mut value);
    // 生成规范化后的变化值。
    let value = generate_expression_inner(&value, event)?;
    // 第三参数只接受静态字符串语义值。
    let ExpressionKind::String(kind) = &arguments[2].value.kind else {
        // 返回类型枚举形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 kind 参数。
            arguments[2].span,
            // 说明静态枚举要求。
            "WaterfallData kind 必须是字符串语义值",
            // 给出完整允许集合。
            "使用 'total'、'increase' 或 'decrease'",
        ));
    };
    // 把字符串语义值映射为公开枚举。
    let kind = match kind.as_str() {
        // 映射合计柱。
        "total" => quote! { ::uix_app::prelude::WaterfallKind::Total },
        // 映射增长柱。
        "increase" => quote! { ::uix_app::prelude::WaterfallKind::Increase },
        // 映射减少柱。
        "decrease" => quote! { ::uix_app::prelude::WaterfallKind::Decrease },
        // 其他值不在登记表。
        _ => {
            // 返回未知语义值诊断。
            return Err(Diagnostic::new(
                // 指向非法 kind 参数。
                arguments[2].span,
                // 点名未知值。
                format!("WaterfallData kind={kind:?} 不在登记表"),
                // 给出完整允许集合。
                "使用 'total'、'increase' 或 'decrease'",
            ));
        }
    };
    // 调用公开瀑布图数据构造器。
    Ok(quote! { ::uix_app::prelude::WaterfallData::new(#label, #value, #kind) })
}

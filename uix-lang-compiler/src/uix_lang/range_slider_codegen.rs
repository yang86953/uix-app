// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 复用滑块组件族唯一的数值与范围校验规则，并引入共享元素属性查找。
use super::find_attribute;
use super::slider_codegen::{
    f64_value, range_endpoint, validate_literal_range, validate_literal_step,
};
// 引入 RangeSlider 属性、结构表达式与诊断契约。
use super::{AttributeValue, Diagnostic, Element, ExpressionKind, generate_expression};

// 生成把结构化区间绑定映射到两个 State<f64> 句柄的区间滑块。
pub(crate) fn generate_range_slider(element: &Element) -> Result<TokenStream, Diagnostic> {
    // RangeSlider 是叶组件，子树不能被静默忽略。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 RangeSlider 元素。
            element.span,
            // 说明区间滑块不接受子节点。
            "<RangeSlider> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <RangeSlider value={{ start: lower, end: upper }} />",
        ));
    }

    // 同时为静态范围执行编译期顺序校验。
    validate_literal_range(element, "RangeSlider")?;
    // 生成显式或默认最小值。
    let minimum = range_endpoint(element, "min", 0.0, "RangeSlider")?;
    // 生成显式或默认最大值。
    let maximum = range_endpoint(element, "max", 100.0, "RangeSlider")?;
    // 通过公开构造器一次建立完整范围契约。
    let mut widget = quote! { ::uix::prelude::RangeSlider::new((#minimum)..=(#maximum)) };

    // 步长必须为正的有限数值。
    if let Some(attribute) = find_attribute(element, "step") {
        // 静态字面量在编译期拒绝非正值。
        validate_literal_step(attribute, "RangeSlider")?;
        // 生成 f64 步长。
        let value = f64_value(attribute, "RangeSlider step")?;
        // 在状态绑定前应用公开步长构建器。
        widget = quote! { (#widget).step(#value) };
    }
    // 可选 value 必须明确提供两个状态字段。
    if let Some(attribute) = find_attribute(element, "value") {
        // 只有表达式属性可以携带结构化对象。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回结构绑定诊断。
            return Err(value_shape_diagnostic(attribute.span));
        };
        // 对象字面量由本组件专用生成器消费。
        let ExpressionKind::Object(fields) = &expression.expression.kind else {
            // 普通表达式无法确定两个状态所有权句柄。
            return Err(value_shape_diagnostic(attribute.span));
        };
        // 拒绝文档契约外字段，防止静默忽略拼写错误。
        if let Some(field) = fields
            // 遍历对象字段。
            .iter()
            // 查找非 start/end 字段。
            .find(|field| field.name != "start" && field.name != "end")
        {
            // 返回额外字段诊断。
            return Err(Diagnostic::new(
                // 精确指向未知字段。
                field.span,
                // 说明实际字段名称。
                format!("RangeSlider value 不支持字段 {}", field.name),
                // 给出完整允许集合。
                "只声明 start 与 end 两个 State<f64> 字段",
            ));
        }
        // 查找起点状态字段。
        let start = fields
            // 遍历对象字段。
            .iter()
            // 匹配起点名称。
            .find(|field| field.name == "start")
            // 缺失字段返回结构诊断。
            .ok_or_else(|| missing_field_diagnostic(attribute.span, "start"))?;
        // 查找终点状态字段。
        let end = fields
            // 遍历对象字段。
            .iter()
            // 匹配终点名称。
            .find(|field| field.name == "end")
            // 缺失字段返回结构诊断。
            .ok_or_else(|| missing_field_diagnostic(attribute.span, "end"))?;
        // 生成起点 State<f64> 受限表达式。
        let start_state = generate_expression(&start.value, None)?;
        // 生成终点 State<f64> 受限表达式。
        let end_state = generate_expression(&end.value, None)?;
        // 按运行时归一化顺序绑定起点状态。
        widget = quote! { (#widget).start(&(#start_state)) };
        // 再绑定终点状态，完整建立区间所有权。
        widget = quote! { (#widget).end(&(#end_state)) };
    }

    // 物化为公开叶 View，再应用统一尺寸、样式与自动化属性。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费 RangeSlider 专有属性并返回公共 View 表达式。
    apply_common_attributes(
        // 传入已经配置的区间滑块 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["value", "min", "max", "step"],
    )
}

// 构造 value 必须是结构对象的诊断。
fn value_shape_diagnostic(span: super::SourceSpan) -> Diagnostic {
    // 返回确定性的结构绑定错误。
    Diagnostic::new(
        // 指向完整 value 属性。
        span,
        // 说明两个状态句柄的公开契约。
        "RangeSlider value 必须是包含 start/end State<f64> 表达式的对象",
        // 给出规范双花括号写法。
        "使用 value={{ start: lower, end: upper }}",
    )
}

// 构造缺失必需区间字段的诊断。
fn missing_field_diagnostic(span: super::SourceSpan, field: &str) -> Diagnostic {
    // 返回精确缺失字段错误。
    Diagnostic::new(
        // 指向完整 value 属性。
        span,
        // 说明缺失字段名称。
        format!("RangeSlider value 缺少 {field} 字段"),
        // 给出完整结构示例。
        "同时声明 start 与 end 两个 State<f64> 字段",
    )
}

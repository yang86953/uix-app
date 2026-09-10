// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入日期范围对象、表达式与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, ExpressionKind, SourceSpan, generate_expression,
};

// 生成把结构化起止日期绑定映射到两个 State<Date> 句柄的范围选择器。
pub(crate) fn generate_date_range_picker(
    // 接收完整 DateRangePicker 元素。
    element: &Element,
) -> Result<TokenStream, Diagnostic> {
    // DateRangePicker 是叶组件，不能静默丢弃子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 DateRangePicker 元素。
            element.span,
            // 说明日期范围选择器不接受子节点。
            "<DateRangePicker> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <DateRangePicker value={{ start: range_start, end: range_end }} />",
        ));
    }

    // 查找文档要求的结构化日期范围绑定。
    let value_attribute = required_attribute(element, "value")?;
    // 只有表达式属性可以携带结构化对象。
    let AttributeValue::Expression(value_expression) = &value_attribute.value else {
        // 返回结构绑定诊断。
        return Err(value_shape_diagnostic(value_attribute.span));
    };
    // DateRangePicker 专用生成器消费对象字面量。
    let ExpressionKind::Object(fields) = &value_expression.expression.kind else {
        // 普通表达式无法确定两个状态所有权句柄。
        return Err(value_shape_diagnostic(value_attribute.span));
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
            format!("DateRangePicker value 不支持字段 {}", field.name),
            // 给出完整允许集合。
            "只声明 start 与 end 两个 State<Date> 字段",
        ));
    }
    // 查找起点日期状态字段。
    let start = fields
        // 遍历对象字段。
        .iter()
        // 匹配起点名称。
        .find(|field| field.name == "start")
        // 缺失字段返回结构诊断。
        .ok_or_else(|| missing_field_diagnostic(value_attribute.span, "start"))?;
    // 查找终点日期状态字段。
    let end = fields
        // 遍历对象字段。
        .iter()
        // 匹配终点名称。
        .find(|field| field.name == "end")
        // 缺失字段返回结构诊断。
        .ok_or_else(|| missing_field_diagnostic(value_attribute.span, "end"))?;
    // 生成起点 State<Date> 受限表达式。
    let start_state = generate_expression(&start.value, None)?;
    // 生成终点 State<Date> 受限表达式。
    let end_state = generate_expression(&end.value, None)?;

    // 按运行时归一化顺序绑定起点和终点状态。
    let widget = quote! {
        ::uix_app::prelude::DateRangePicker::new()
            .start(&(#start_state))
            .end(&(#end_state))
    };
    // 物化为公开叶 View，再应用统一尺寸、样式与自动化属性。
    let view = quote! { ::uix_app::prelude::ViewNode::leaf(#widget) };
    // 消费 DateRangePicker 专有属性并返回公共 View 表达式。
    apply_common_attributes(
        // 传入已经配置的日期范围 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["value"],
    )
}

// 查找 DateRangePicker 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "DateRangePicker",
        "使用 <DateRangePicker value={{ start: range_start, end: range_end }} />",
    )
}

// 构造 value 必须是结构对象的诊断。
fn value_shape_diagnostic(span: SourceSpan) -> Diagnostic {
    // 返回确定性的结构绑定错误。
    Diagnostic::new(
        // 指向完整 value 属性。
        span,
        // 说明两个状态句柄的公开契约。
        "DateRangePicker value 必须是包含 start/end State<Date> 表达式的对象",
        // 给出规范双花括号写法。
        "使用 value={{ start: range_start, end: range_end }}",
    )
}

// 构造缺失必需日期范围字段的诊断。
fn missing_field_diagnostic(span: SourceSpan, field: &str) -> Diagnostic {
    // 返回精确缺失字段错误。
    Diagnostic::new(
        // 指向完整 value 属性。
        span,
        // 说明缺失字段名称。
        format!("DateRangePicker value 缺少 {field} 字段"),
        // 给出完整结构示例。
        "同时声明 start 与 end 两个 State<Date> 字段",
    )
}

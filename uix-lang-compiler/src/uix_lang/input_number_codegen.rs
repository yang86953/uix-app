// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 InputNumber 属性、表达式与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, generate_expression};

// 与运行时 f64 精度归一化上限保持一致。
const MAX_PRECISION: u8 = 15;

// 生成保持泛型 State<T> 双向绑定的数值输入节点。
pub(crate) fn generate_input_number(element: &Element) -> Result<TokenStream, Diagnostic> {
    // InputNumber 是叶组件，子树不能被静默忽略。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 InputNumber 元素。
            element.span,
            // 说明数值输入不接受子节点。
            "<InputNumber> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <InputNumber value={amount} />",
        ));
    }

    // 同时为静态范围执行编译期顺序校验。
    validate_literal_range(element)?;
    // 从公开默认构造器开始配置。
    let mut widget = quote! { ::uix::prelude::InputNumber::new() };

    // 先应用最小值，保证后续绑定按范围归一化。
    if let Some(attribute) = find_attribute(element, "min") {
        // 生成有限 f64 最小值。
        let value = f64_value(attribute, "InputNumber min")?;
        // 调用公开最小值构建器。
        widget = quote! { (#widget).min(#value) };
    }
    // 再应用最大值，完整建立运行时范围。
    if let Some(attribute) = find_attribute(element, "max") {
        // 生成有限 f64 最大值。
        let value = f64_value(attribute, "InputNumber max")?;
        // 调用公开最大值构建器。
        widget = quote! { (#widget).max(#value) };
    }
    // 步长必须为正的有限数值。
    if let Some(attribute) = find_attribute(element, "step") {
        // 静态字面量在编译期拒绝非正值。
        validate_literal_step(attribute)?;
        // 生成 f64 步长。
        let value = f64_value(attribute, "InputNumber step")?;
        // 调用公开步长构建器。
        widget = quote! { (#widget).step(#value) };
    }
    // 精度在值绑定前应用，避免声明顺序影响初值量化。
    if let Some(attribute) = find_attribute(element, "precision") {
        // 生成 u8 精度字面量或受限表达式。
        let value = precision_value(attribute)?;
        // 调用公开精度构建器。
        widget = quote! { (#widget).precision(#value) };
    }
    // 可选 value 必须保留泛型 State<T> 所有权句柄。
    if let Some(attribute) = find_attribute(element, "value") {
        // 字面量不能提供双向状态所有权。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回绑定形状诊断。
            return Err(Diagnostic::new(
                // 指向非法 value 属性。
                attribute.span,
                // 说明公开运行时绑定类型。
                "InputNumber value 必须绑定 State<T: InputNumberValue> 表达式",
                // 给出规范绑定写法。
                "使用 value={amount}",
            ));
        };
        // 生成受限状态表达式。
        let state = generate_expression(&expression.expression, None)?;
        // 借用状态句柄交给公开 InputNumber 双向绑定入口。
        widget = quote! { (#widget).value(&(#state)) };
    }

    // 物化为公开叶 View，再应用统一尺寸、样式与自动化属性。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费 InputNumber 专有属性并返回公共 View 表达式。
    apply_common_attributes(
        // 传入已经配置的数值输入 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["value", "min", "max", "step", "precision"],
    )
}

// 生成有限 f64 字面量或受限表达式。
fn f64_value(attribute: &Attribute, label: &str) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状生成数值。
    match &attribute.value {
        // 字面量必须能解析为有限 f64。
        AttributeValue::Literal(source) => {
            // 解析十进制或科学计数法字面量。
            let value = source.parse::<f64>().map_err(|_| {
                // 返回数值类型诊断。
                Diagnostic::new(
                    // 指向完整属性。
                    attribute.span,
                    // 说明字面量解析失败。
                    format!("{label} 必须是有限 f64 数值"),
                    // 给出合法示例。
                    "使用 0、1.5、-10 或对应 f64 表达式",
                )
            })?;
            // 无穷与 NaN 不能进入范围契约。
            if !value.is_finite() {
                // 返回有限性诊断。
                return Err(Diagnostic::new(
                    // 指向非法数值。
                    attribute.span,
                    // 说明有限性要求。
                    format!("{label} 必须是有限数值"),
                    // 给出修复建议。
                    "使用可由 f64 表达的有限数值",
                ));
            }
            // 生成类型明确的 f64 字面量。
            Ok(quote! { #value })
        }
        // 动态值保持 Rust 类型检查。
        AttributeValue::Expression(expression) => {
            // 生成受限数值表达式。
            generate_expression(&expression.expression, None)
        }
        // 结构化内联样式不可能用于数值属性。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向异常属性。
            attribute.span,
            // 说明内部属性形状不匹配。
            format!("{label} 不能使用内联样式值"),
            // 给出有效数值写法。
            "使用数值字面量或受限表达式",
        )),
    }
}

// 校验静态 min/max 不构成反向范围。
fn validate_literal_range(element: &Element) -> Result<(), Diagnostic> {
    // 同时存在两个字面量时才可在编译期比较。
    let Some(minimum) = literal_f64(find_attribute(element, "min"))? else {
        // 动态或缺失最小值交给运行时公开契约。
        return Ok(());
    };
    // 读取可比较的静态最大值。
    let Some(maximum) = literal_f64(find_attribute(element, "max"))? else {
        // 动态或缺失最大值交给运行时公开契约。
        return Ok(());
    };
    // 正向或相等范围均合法。
    if minimum <= maximum {
        // 返回校验成功。
        return Ok(());
    }
    // 反向范围指向完整元素以覆盖两个属性。
    Err(Diagnostic::new(
        // 指向完整 InputNumber。
        element.span,
        // 说明实际反向范围。
        format!("InputNumber min={minimum} 不能大于 max={maximum}"),
        // 给出修复建议。
        "调整 min/max，使 min <= max",
    ))
}

// 提取可选属性的静态有限 f64。
fn literal_f64(attribute: Option<&Attribute>) -> Result<Option<f64>, Diagnostic> {
    // 缺失属性没有静态值。
    let Some(attribute) = attribute else {
        // 返回无值。
        return Ok(None);
    };
    // 动态表达式不能在编译期比较。
    let AttributeValue::Literal(source) = &attribute.value else {
        // 返回无静态值。
        return Ok(None);
    };
    // 复用统一数值诊断并提取字面量。
    let tokens = f64_value(attribute, "InputNumber 范围")?;
    // 令牌生成已验证字面量，这里再次解析取得比较值。
    let value = source.parse::<f64>().map_err(|_| {
        // 理论上由 f64_value 提前返回，仅保留结构保护。
        Diagnostic::new(
            // 指向异常属性。
            attribute.span,
            // 说明内部解析不一致。
            "InputNumber 范围字面量解析不一致",
            // 给出重新声明建议。
            "重新声明有限 min/max 数值",
        )
    })?;
    // 明确消费验证令牌，避免重复实现漂移。
    let _ = tokens;
    // 返回静态比较值。
    Ok(Some(value))
}

// 校验静态步长为正。
fn validate_literal_step(attribute: &Attribute) -> Result<(), Diagnostic> {
    // 动态表达式由运行时 step 归一化。
    let AttributeValue::Literal(source) = &attribute.value else {
        // 返回校验成功。
        return Ok(());
    };
    // 复用有限数值解析。
    let value = source.parse::<f64>().map_err(|_| {
        // 返回步长类型诊断。
        Diagnostic::new(
            // 指向非法 step。
            attribute.span,
            // 说明数值要求。
            "InputNumber step 必须是正的有限 f64 数值",
            // 给出合法示例。
            "使用 1、0.5 或对应 f64 表达式",
        )
    })?;
    // 正有限值符合运行时步进契约。
    if value.is_finite() && value > 0.0 {
        // 返回校验成功。
        return Ok(());
    }
    // 拒绝零、负值和非有限值。
    Err(Diagnostic::new(
        // 指向非法 step。
        attribute.span,
        // 说明步长约束。
        "InputNumber step 必须大于 0",
        // 给出修复建议。
        "使用正的有限步长",
    ))
}

// 生成精度字面量或受限 u8 表达式。
fn precision_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按值形状生成精度。
    match &attribute.value {
        // 静态精度执行整数与上限校验。
        AttributeValue::Literal(source) => {
            // 解析 u8 整数。
            let precision = source.parse::<u8>().map_err(|_| {
                // 返回整数类型诊断。
                Diagnostic::new(
                    // 指向非法 precision。
                    attribute.span,
                    // 说明精度类型。
                    "InputNumber precision 必须是 0..=15 的整数",
                    // 给出合法示例。
                    "使用 precision=\"0\"、precision=\"2\" 或 u8 表达式",
                )
            })?;
            // 超过 f64 可兑现精度时编译期拒绝。
            if precision > MAX_PRECISION {
                // 返回上限诊断。
                return Err(Diagnostic::new(
                    // 指向越界 precision。
                    attribute.span,
                    // 说明实际与允许范围。
                    format!("InputNumber precision={precision} 超过 15"),
                    // 给出有效范围。
                    "使用 0..=15 的精度",
                ));
            }
            // 生成类型明确的 u8 字面量。
            Ok(quote! { #precision })
        }
        // 动态精度保持 u8 类型检查并由运行时归一化上限。
        AttributeValue::Expression(expression) => {
            // 生成受限精度表达式。
            generate_expression(&expression.expression, None)
        }
        // 结构化内联样式不可能用于精度属性。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向异常 precision。
            attribute.span,
            // 说明内部属性形状不匹配。
            "InputNumber precision 不能使用内联样式值",
            // 给出有效精度写法。
            "使用 0..=15 的整数或 u8 表达式",
        )),
    }
}

// 查找元素上的具名属性。

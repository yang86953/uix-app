// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Slider 属性、表达式与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, generate_expression};
// 引入共享元素属性查找，替代模块内副本。
use super::find_attribute;

// 生成保持 State<f64> 双向绑定的单值滑块节点。
pub(crate) fn generate_slider(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Slider 是叶组件，子树不能被静默忽略。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Slider 元素。
            element.span,
            // 说明单值滑块不接受子节点。
            "<Slider> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <Slider value={volume} />",
        ));
    }

    // 同时为静态范围执行编译期顺序校验。
    validate_literal_range(element, "Slider")?;
    // 生成显式或默认最小值。
    let minimum = range_endpoint(element, "min", 0.0, "Slider")?;
    // 生成显式或默认最大值。
    let maximum = range_endpoint(element, "max", 100.0, "Slider")?;
    // 通过公开构造器一次建立完整范围契约。
    let mut widget = quote! { ::uix::prelude::Slider::new((#minimum)..=(#maximum)) };

    // 步长必须为正的有限数值。
    if let Some(attribute) = find_attribute(element, "step") {
        // 静态字面量在编译期拒绝非正值。
        validate_literal_step(attribute, "Slider")?;
        // 生成 f64 步长。
        let value = f64_value(attribute, "Slider step")?;
        // 在状态绑定前应用公开步长构建器。
        widget = quote! { (#widget).step(#value) };
    }
    // 可选 value 必须保留 State<f64> 所有权句柄。
    if let Some(attribute) = find_attribute(element, "value") {
        // 字面量不能提供双向状态所有权。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回绑定形状诊断。
            return Err(Diagnostic::new(
                // 指向非法 value 属性。
                attribute.span,
                // 说明公开运行时绑定类型。
                "Slider value 必须绑定 State<f64> 表达式",
                // 给出规范绑定写法。
                "使用 value={volume}",
            ));
        };
        // 生成受限状态表达式。
        let state = generate_expression(&expression.expression, None)?;
        // 借用状态句柄交给公开 Slider 双向绑定入口。
        widget = quote! { (#widget).value(&(#state)) };
    }

    // 物化为公开叶 View，再应用统一尺寸、样式与自动化属性。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费 Slider 专有属性并返回公共 View 表达式。
    apply_common_attributes(
        // 传入已经配置的单值滑块 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["value", "min", "max", "step"],
    )
}

// 生成显式端点或文档默认端点。
pub(super) fn range_endpoint(
    // 借用完整 Slider 元素。
    element: &Element,
    // 指定待读取的端点属性名。
    name: &str,
    // 提供缺省端点值。
    default: f64,
    // 指定诊断中的组件名称。
    widget: &str,
) -> Result<TokenStream, Diagnostic> {
    // 显式端点沿用统一有限 f64 生成契约。
    if let Some(attribute) = find_attribute(element, name) {
        // 生成类型明确的动态或静态端点。
        return f64_value(attribute, &format!("{widget} {name}"));
    }
    // 缺失属性映射为文档声明的 f64 默认值。
    Ok(quote! { #default })
}

// 生成有限 f64 字面量或受限表达式。
pub(super) fn f64_value(
    // 借用待生成的数值属性。
    attribute: &Attribute,
    // 提供诊断使用的属性标签。
    label: &str,
) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状生成数值。
    match &attribute.value {
        // 字面量必须能解析为有限 f64。
        AttributeValue::Literal(_) => {
            // 解析并验证静态数值。
            let value = parse_literal_f64(attribute, label)?;
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

// 解析一个有限 f64 字面量。
fn parse_literal_f64(attribute: &Attribute, label: &str) -> Result<f64, Diagnostic> {
    // 调用方只会为字面量进入本函数。
    let AttributeValue::Literal(source) = &attribute.value else {
        // 防止内部调用破坏属性形状契约。
        return Err(Diagnostic::new(
            // 指向异常属性。
            attribute.span,
            // 说明内部类型不一致。
            format!("{label} 必须是数值字面量"),
            // 给出有效数值写法。
            "使用 0、1.5、-10 或对应 f64 表达式",
        ));
    };
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
    // 返回已验证的静态数值。
    Ok(value)
}

// 校验静态 min/max 不构成反向范围。
pub(super) fn validate_literal_range(
    // 借用声明范围的滑块元素。
    element: &Element,
    // 指定诊断中的组件名称。
    widget: &str,
) -> Result<(), Diagnostic> {
    // 读取可比较的静态最小值，缺失时采用文档默认值。
    let minimum = literal_endpoint(element, "min", 0.0, widget)?;
    // 读取可比较的静态最大值，缺失时采用文档默认值。
    let maximum = literal_endpoint(element, "max", 100.0, widget)?;
    // 任一动态端点都交给运行时公开归一化契约。
    let (Some(minimum), Some(maximum)) = (minimum, maximum) else {
        // 动态范围无法在编译期比较。
        return Ok(());
    };
    // 正向或相等范围均合法。
    if minimum <= maximum {
        // 返回校验成功。
        return Ok(());
    }
    // 反向范围指向完整元素以覆盖两个属性。
    Err(Diagnostic::new(
        // 指向完整 Slider。
        element.span,
        // 说明实际反向范围。
        format!("{widget} min={minimum} 不能大于 max={maximum}"),
        // 给出修复建议。
        "调整 min/max，使 min <= max",
    ))
}

// 提取可选端点的静态有限 f64。
fn literal_endpoint(
    // 借用完整 Slider 元素。
    element: &Element,
    // 指定端点属性名。
    name: &str,
    // 提供缺失属性的静态默认值。
    default: f64,
    // 指定诊断中的组件名称。
    widget: &str,
) -> Result<Option<f64>, Diagnostic> {
    // 缺失属性可直接参与静态范围比较。
    let Some(attribute) = find_attribute(element, name) else {
        // 返回文档默认端点。
        return Ok(Some(default));
    };
    // 动态表达式不能在编译期比较。
    if !matches!(attribute.value, AttributeValue::Literal(_)) {
        // 返回无静态值。
        return Ok(None);
    }
    // 解析并返回静态有限端点。
    parse_literal_f64(attribute, &format!("{widget} {name}")).map(Some)
}

// 校验静态步长为正。
pub(super) fn validate_literal_step(
    // 借用步长属性。
    attribute: &Attribute,
    // 指定诊断中的组件名称。
    widget: &str,
) -> Result<(), Diagnostic> {
    // 动态表达式由运行时 step 归一化。
    if !matches!(attribute.value, AttributeValue::Literal(_)) {
        // 返回校验成功。
        return Ok(());
    }
    // 复用有限数值解析。
    let value = parse_literal_f64(attribute, &format!("{widget} step"))?;
    // 正有限值符合运行时步进契约。
    if value > 0.0 {
        // 返回校验成功。
        return Ok(());
    }
    // 拒绝零和负值。
    Err(Diagnostic::new(
        // 指向非法 step。
        attribute.span,
        // 说明步长约束。
        format!("{widget} step 必须大于 0"),
        // 给出修复建议。
        "使用正的有限步长",
    ))
}

// 查找元素上的具名属性。

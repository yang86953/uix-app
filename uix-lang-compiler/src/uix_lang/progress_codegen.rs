// 引入 f32 字面量与过程宏令牌流。
use proc_macro2::{Literal, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 ProgressBar 属性、表达式、布尔值、关键字与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, boolean_value, generate_expression,
    literal_string,
};

// 生成保持运行时归一化、模式与动画唯一所有权的 ProgressBar。
pub(crate) fn generate_progress_bar(element: &Element) -> Result<TokenStream, Diagnostic> {
    // ProgressBar 自身绘制完整进度指示器，不接受内容子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 ProgressBar 元素。
            element.span,
            // 说明进度条不接受子节点。
            "<ProgressBar> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <ProgressBar progress={value} indeterminate={waiting} type=\"line\" />",
        ));
    }

    // 从公开默认构造器开始，保留 progress=0 与 line/determinate 默认值。
    let mut widget = quote! { ::uix::prelude::ProgressBar::new() };
    // 可选 progress 接受通过静态范围校验的字面量或 f32 表达式。
    if let Some(attribute) = find_attribute(element, "progress") {
        // 生成有限 fraction 或保留动态 Rust 类型检查。
        let progress = progress_value(attribute)?;
        // 运行时组件保存唯一归一值与可观察原因。
        widget = quote! { (#widget).progress(#progress) };
    }
    // 可选 indeterminate 显式选择最终模式，不依赖 builder 调用顺序。
    if let Some(attribute) = find_attribute(element, "indeterminate") {
        // 复用统一布尔值诊断与动态表达式生成。
        let indeterminate = boolean_value(attribute)?;
        // 运行时保留确定 fraction 并独占动画相位。
        widget = quote! { (#widget).indeterminate_when(#indeterminate) };
    }
    // 可选 type 只选择公开 line 或 circle 形态。
    if let Some(attribute) = find_attribute(element, "type") {
        // 关键字必须在编译期确定。
        let progress_type = literal_string(attribute, "ProgressBar type")?;
        // 映射批准的有限形态集合。
        match progress_type.as_str() {
            // line 保留公开默认形态。
            "line" => {}
            // circle 调用公开圆形构建器。
            "circle" => widget = quote! { (#widget).circle() },
            // 未登记形态不得静默回退为 line。
            _ => {
                // 返回有限关键字诊断。
                return Err(Diagnostic::new(
                    // 指向非法 type 属性。
                    attribute.span,
                    // 说明完整合法集合。
                    "ProgressBar type 只支持 line 或 circle",
                    // 给出规范修复写法。
                    "使用 type=\"line\" 或 type=\"circle\"",
                ));
            }
        }
    }

    // 经公开 View 契约进入组件自己的同目录 UIX 声明壳。
    let view = quote! { ::uix::prelude::View::build(#widget) };
    // 消费专有属性并应用公共尺寸、样式与自动化属性。
    apply_common_attributes(
        // 传入已经配置 fraction、模式与形态的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性被二次映射。
        &["progress", "indeterminate", "type"],
    )
}

// 生成 ProgressBar 的有限 f32 fraction。
fn progress_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状生成 fraction。
    match &attribute.value {
        // 静态字面量在编译期完成有限性与范围校验。
        AttributeValue::Literal(source) => {
            // 解析 f32 以匹配公开运行时 API。
            let value = source.parse::<f32>().map_err(|_| {
                // 返回明确数值类型诊断。
                Diagnostic::new(
                    // 指向非法 progress 属性。
                    attribute.span,
                    // 说明公开 fraction 类型。
                    "ProgressBar progress 必须是有限 f32 fraction",
                    // 给出合法静态或动态示例。
                    "使用 0.0..=1.0 数值或 f32 表达式",
                )
            })?;
            // NaN 与 Infinity 不进入运行时静态声明。
            if !value.is_finite() {
                // 返回有限性诊断。
                return Err(Diagnostic::new(
                    // 指向非有限字面量。
                    attribute.span,
                    // 说明有限性要求。
                    "ProgressBar progress 必须是有限数值",
                    // 给出合法范围。
                    "使用 0.0..=1.0 的有限 fraction",
                ));
            }
            // 静态越界直接报告来源位置。
            if !(0.0..=1.0).contains(&value) {
                // 返回范围诊断。
                return Err(Diagnostic::new(
                    // 指向越界字面量。
                    attribute.span,
                    // 说明统一 fraction 范围。
                    "ProgressBar progress 必须位于 0.0..=1.0",
                    // 给出单位转换建议。
                    "使用 fraction；业务百分数请在表达式边界显式除以 100",
                ));
            }
            // 生成无后缀 f32 字面量以匹配公开 API。
            let literal = Literal::f32_unsuffixed(value);
            // 返回已验证的静态 fraction。
            Ok(quote! { #literal })
        }
        // 动态表达式由 Rust 类型检查，运行时负责安全归一化与观察。
        AttributeValue::Expression(expression) => {
            // 生成受限 f32 表达式。
            generate_expression(&expression.expression, None)
        }
        // 内联样式不可能成为 fraction。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向异常属性。
            attribute.span,
            // 说明属性形状错误。
            "ProgressBar progress 不能使用内联样式值",
            // 给出有效写法。
            "使用 0.0..=1.0 数值或 f32 表达式",
        )),
    }
}

// 查找元素上的具名属性。
fn find_attribute<'a>(element: &'a Element, name: &str) -> Option<&'a Attribute> {
    // 解析器已经保证同名属性唯一。
    element
        // 借用有序属性集合。
        .attributes
        // 遍历每个属性。
        .iter()
        // 返回首个名称匹配项。
        .find(|attribute| attribute.name == name)
}

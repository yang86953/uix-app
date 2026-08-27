// 引入卫生标识符与过程宏令牌流。
use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 复用 Slider 组件族唯一的有限数值与范围校验规则，并引入共享元素属性查找。
use super::find_attribute;
use super::slider_codegen::{
    f64_value, range_endpoint, validate_literal_range, validate_literal_step,
};
// 引入表单字段语法树、诊断与静态成员映射契约。
use super::{Attribute, Diagnostic, Element, Node, literal_string, rust_identifier};

// 生成单个类型化 f64 滑块字段链。
pub(super) fn generate_form_slider_field(
    // 接收 Form 的直接滑块字段子项。
    element: &Element,
) -> Result<TokenStream, Diagnostic> {
    // 读取稳定字段 key。
    let field_attribute = required_attribute(element, "field")?;
    // 字段 key 必须是编译期字符串。
    let field = literal_string(field_attribute, "FormSliderItem field")?;
    // 验证字段可投影 Rust 成员。
    let field_ident = rust_identifier(&field, field_attribute.span)?;
    // 表单标签省略时沿用字段 key。
    let label = find_attribute(element, "label")
        // 显式标签必须是编译期字符串。
        .map(|attribute| literal_string(attribute, "FormSliderItem label"))
        // 转置可选诊断。
        .transpose()?
        // 保持字段 key 回退行为。
        .unwrap_or_else(|| field.clone());

    // 同时为静态范围执行编译期顺序校验。
    validate_literal_range(element, "FormSliderItem")?;
    // 生成显式或文档默认最小值。
    let minimum = range_endpoint(element, "min", 0.0, "FormSliderItem")?;
    // 生成显式或文档默认最大值。
    let maximum = range_endpoint(element, "max", 100.0, "FormSliderItem")?;
    // 建立类型化字段基础构建链。
    let mut item = quote! {
        ::uix::prelude::FormSliderItem::new(#field, (#minimum)..=(#maximum))
            .label(#label)
    };
    // 可选步长必须为正的有限数值。
    if let Some(attribute) = find_attribute(element, "step") {
        // 静态字面量在编译期拒绝非正值。
        validate_literal_step(attribute, "FormSliderItem")?;
        // 生成 f64 步长表达式。
        let step = f64_value(attribute, "FormSliderItem step")?;
        // 应用运行时滑块步长构建器。
        item = quote! { (#item).step(#step) };
    }

    // 拒绝静态适配器未登记的字段属性。
    for attribute in &element.attributes {
        // 只消费当前类型化滑块字段属性。
        if !matches!(
            // 检查稳定属性名。
            attribute.name.as_str(),
            // 保持数值字段的最小公开契约。
            "field" | "label" | "min" | "max" | "step"
        ) {
            // 返回属性映射诊断。
            return Err(Diagnostic::new(
                // 指向未知属性。
                attribute.span,
                // 说明未登记的具体属性。
                format!("FormSliderItem 属性 {} 尚无已登记映射", attribute.name),
                // 列出当前允许的完整属性集合。
                "当前使用 field、label、min、max 与 step",
            ));
        }
    }
    // 滑块字段项必须是叶节点。
    if element
        // 遍历全部直接子节点。
        .children
        // 获取只读迭代器。
        .iter()
        // 排版空白以外的节点均不合法。
        .any(|child| !matches!(child, Node::Text(text) if text.value.trim().is_empty()))
    {
        // 返回叶节点诊断。
        return Err(Diagnostic::new(
            // 指向完整滑块字段项。
            element.span,
            // 说明子节点不属于滑块字段契约。
            "<FormSliderItem> 不接受子节点",
            // 给出规范自闭合写法。
            "使用自闭合 FormSliderItem",
        ));
    }

    // 创建卫生模型访问器参数。
    let model = Ident::new("__uix_form_model", Span::mixed_site());
    // 生成 f64 字段投影和运行时字段配置。
    Ok(quote! {
        .field(
            #field,
            |#model| &mut #model.#field_ident,
            #item,
        )
    })
}

// 拒绝失去 Form 类型化上下文的滑块字段项。
pub(crate) fn generate_orphan_form_slider_item(
    // 接收越界滑块字段项。
    element: &Element,
) -> Result<TokenStream, Diagnostic> {
    // 返回父子归属诊断。
    Err(Diagnostic::new(
        // 指向完整越界元素。
        element.span,
        // 说明类型化字段必须由 Form 解释。
        "<FormSliderItem> 只能作为 <Form> 的直接子项",
        // 给出恢复上下文的规范写法。
        "把 FormSliderItem 放入绑定 model 的 Form 内",
    ))
}

// 查找必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        // 诊断标签跟随实际元素名。
        &element.name,
        "按 Form 类型化映射文档补齐必需属性",
    )
}

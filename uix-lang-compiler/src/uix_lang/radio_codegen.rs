// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Radio 属性、表达式与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, generate_expression};

// 生成绑定 State<String> 与字符串选项集合的单选组节点。
pub(crate) fn generate_radio(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Radio 是叶组件，子树不能被静默忽略。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Radio 元素。
            element.span,
            // 说明单选组不接受子节点。
            "<Radio> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <Radio value={gender} options={gender_options} />",
        ));
    }

    // 查找文档要求的 options 数据引用。
    let options_attribute = required_attribute(element, "options")?;
    // options 必须保持调用侧可迭代字符串集合的 Rust 类型检查。
    let AttributeValue::Expression(options_expression) = &options_attribute.value else {
        // 返回选项表达式形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 options 属性。
            options_attribute.span,
            // 说明公开运行时数据要求。
            "Radio options 必须是可迭代字符串选项表达式",
            // 给出规范数据引用写法。
            "使用 options={gender_options}",
        ));
    };
    // 生成受限选项数据表达式。
    let options = generate_expression(&options_expression.expression, None)?;
    // 查找文档要求的 value 双向绑定。
    let value_attribute = required_attribute(element, "value")?;
    // value 字面量不能提供双向状态所有权。
    let AttributeValue::Expression(value_expression) = &value_attribute.value else {
        // 返回绑定形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 value 属性。
            value_attribute.span,
            // 说明公开运行时绑定类型。
            "Radio value 必须绑定 State<String> 表达式",
            // 给出规范绑定写法。
            "使用 value={gender}",
        ));
    };
    // 生成受限状态表达式。
    let state = generate_expression(&value_expression.expression, None)?;

    // 先设置选项再绑定状态，确保运行时立即同步选中值。
    let widget = quote! {
        ::uix_app::prelude::Radio::new()
            .options((#options).clone())
            .value(&(#state))
    };
    // 物化为公开叶 View，再应用统一尺寸、样式与自动化属性。
    let view = quote! { ::uix_app::prelude::ViewNode::leaf(#widget) };
    // 消费 Radio 专有属性并返回公共 View 表达式。
    apply_common_attributes(
        // 传入已经配置的单选组 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["options", "value"],
    )
}

// 查找 Radio 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "Radio",
        "使用 <Radio value={gender} options={gender_options} />",
    )
}

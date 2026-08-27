// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 ColorPicker 属性、表达式与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, generate_expression};

// 生成绑定颜色状态的 ColorPicker 组件。
pub(crate) fn generate_color_picker(element: &Element) -> Result<TokenStream, Diagnostic> {
    // ColorPicker 是叶组件，不能静默丢弃子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 ColorPicker 元素。
            element.span,
            // 说明颜色选择器不接受子节点。
            "<ColorPicker> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <ColorPicker value={selected_color} />",
        ));
    }

    // 查找文档要求的颜色双向绑定。
    let value_attribute = required_attribute(element, "value")?;
    // value 字面量不能提供双向状态所有权。
    let AttributeValue::Expression(value_expression) = &value_attribute.value else {
        // 返回绑定形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 value 属性。
            value_attribute.span,
            // 说明公开运行时绑定类型。
            "ColorPicker value 必须绑定 State<Color> 表达式",
            // 给出规范状态引用写法。
            "使用 value={selected_color}",
        ));
    };
    // 生成受限颜色状态表达式。
    let state = generate_expression(&value_expression.expression, None)?;
    // 绑定颜色状态并保持运行时现有受控契约。
    let widget = quote! {
        ::uix::prelude::ColorPicker::new()
            .value(&(#state))
    };
    // 物化为公开叶 View，再应用统一尺寸、样式与自动化属性。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费 ColorPicker 专有属性并返回公共 View 表达式。
    apply_common_attributes(
        // 传入已经配置的颜色选择 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["value"],
    )
}

// 查找 ColorPicker 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "ColorPicker",
        "使用 <ColorPicker value={selected_color} />",
    )
}

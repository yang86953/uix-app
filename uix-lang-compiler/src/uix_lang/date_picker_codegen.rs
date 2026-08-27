// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 DatePicker 属性、表达式与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, generate_expression, literal_string};

// 生成绑定日期状态与选择粒度的 DatePicker 组件。
pub(crate) fn generate_date_picker(element: &Element) -> Result<TokenStream, Diagnostic> {
    // DatePicker 是叶组件，不能静默丢弃子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 DatePicker 元素。
            element.span,
            // 说明日期选择器不接受子节点。
            "<DatePicker> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <DatePicker value={selected_date} />",
        ));
    }

    // 查找文档要求的日期双向绑定。
    let value_attribute = required_attribute(element, "value")?;
    // value 字面量不能提供双向状态所有权。
    let AttributeValue::Expression(value_expression) = &value_attribute.value else {
        // 返回绑定形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 value 属性。
            value_attribute.span,
            // 说明公开运行时绑定类型。
            "DatePicker value 必须绑定 State<Date> 表达式",
            // 给出规范状态引用写法。
            "使用 value={selected_date}",
        ));
    };
    // 生成受限日期状态表达式。
    let state = generate_expression(&value_expression.expression, None)?;
    // 先绑定日期状态，保持运行时现有受控契约。
    let mut widget = quote! {
        ::uix::prelude::DatePicker::new()
            .value(&(#state))
    };
    // 可选 mode 只接受文档登记的编译期关键字。
    if let Some(attribute) = find_attribute(element, "mode") {
        // 选择粒度影响运行时枚举，要求使用字符串字面量。
        let mode = literal_string(attribute, "DatePicker mode")?;
        // 把文档关键字映射到公开 PickerMode 枚举。
        let mode = match mode.as_str() {
            // date 保持公开默认值。
            "date" => quote!(::uix::prelude::PickerMode::Date),
            // week 提交命中日期所在周的起点。
            "week" => quote!(::uix::prelude::PickerMode::Week),
            // month 提交命中月份的起点。
            "month" => quote!(::uix::prelude::PickerMode::Month),
            // quarter 提交命中季度的起点。
            "quarter" => quote!(::uix::prelude::PickerMode::Quarter),
            // 未登记关键字不能静默回退到 date。
            _ => {
                // 返回带属性跨度的枚举诊断。
                return Err(Diagnostic::new(
                    // 指向非法 mode 属性。
                    attribute.span,
                    // 说明具体非法值。
                    format!("DatePicker mode={mode:?} 不受支持"),
                    // 给出完整已登记关键字集合。
                    "使用 mode=\"date\"、\"week\"、\"month\" 或 \"quarter\"",
                ));
            }
        };
        // 应用确定的公开选择粒度枚举。
        widget = quote! { (#widget).mode(#mode) };
    }
    // 物化为公开叶 View，再应用统一尺寸、样式与自动化属性。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费 DatePicker 专有属性并返回公共 View 表达式。
    apply_common_attributes(
        // 传入已经配置的日期选择 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["value", "mode"],
    )
}

// 查找 DatePicker 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "DatePicker",
        "使用 <DatePicker value={selected_date} />",
    )
}

// 查找元素上的具名属性。

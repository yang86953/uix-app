// 引入卫生事件变量所需的标识符与跨度。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 SelectableList 属性、表达式、事件与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, generate_event_handler_expression,
    generate_expression,
};

// 生成拥有类型化条目并可绑定稳定活动 id 的可选中列表。
pub(crate) fn generate_selectable_list(element: &Element) -> Result<TokenStream, Diagnostic> {
    // SelectableList 自身绘制完整行列表，不接受 UIX 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 SelectableList 元素。
            element.span,
            // 说明可选中列表不接受子节点。
            "<SelectableList> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <SelectableList items={items} active={active_id} />",
        ));
    }

    // 查找文档要求的类型化条目集合。
    let items_attribute = required_attribute(element, "items")?;
    // 字面量不能提供 Vec<SelectableItem> 所有权。
    let AttributeValue::Expression(items_expression) = &items_attribute.value else {
        // 返回集合表达式形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 items 属性。
            items_attribute.span,
            // 说明公开运行时数据类型。
            "SelectableList items 必须是 Vec<SelectableItem> 表达式",
            // 给出规范数据引用写法。
            "使用 items={selectable_items}",
        ));
    };
    // 生成受限类型化条目表达式。
    let items = generate_expression(&items_expression.expression, None)?;
    // 从公开构造器和拥有型条目入口开始配置。
    let mut widget = quote! {
        ::uix_app::prelude::SelectableList::new().items((#items).clone())
    };

    // active 出现时必须保留 State<Option<String>> 所有权句柄。
    if let Some(attribute) = find_attribute(element, "active") {
        // 字面量不能提供可写回和可订阅的状态句柄。
        let AttributeValue::Expression(active_expression) = &attribute.value else {
            // 返回受控绑定形状诊断。
            return Err(Diagnostic::new(
                // 指向非法 active 属性。
                attribute.span,
                // 明确公开运行时状态类型。
                "SelectableList active 必须绑定 State<Option<String>> 表达式",
                // 给出规范状态引用写法。
                "使用 active={active_id}",
            ));
        };
        // 生成受限状态表达式并由 Rust 核对最终类型。
        let active = generate_expression(&active_expression.expression, None)?;
        // 绑定稳定 id 状态，空值仍由调用方拥有。
        widget = quote! { (#widget).active_state(&(#active)) };
    }

    // 物化为公开叶 View，再登记可选变化观察器。
    let mut view = quote! { ::uix_app::prelude::ViewNode::leaf(#widget) };
    // Change 事件发布已经写回状态的稳定条目 id。
    if let Some(attribute) = find_attribute(element, "@change") {
        // 事件解析器应始终提供受限表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回内部形状保护诊断。
            return Err(Diagnostic::new(
                // 指向完整事件属性。
                attribute.span,
                // 说明事件处理器形状。
                "SelectableList @change 必须是受限处理器表达式",
                // 给出带稳定 id 载荷的规范写法。
                "使用 @change=\"on_active_change($event)\"",
            ));
        };
        // 创建卫生的稳定 id 文本变量。
        let value = Ident::new("__uix_selectable_list_change", Span::mixed_site());
        // 生成裸处理器或显式载荷调用。
        let handler = generate_event_handler_expression(&expression.expression, &value, "@change")?;
        // 复用公开 View Change 注册入口。
        view = quote! {
            (#view).on_change_fn(move |#value| {
                // 丢弃处理器返回值并保留副作用。
                let _ = { #handler };
            })
        };
    }

    // 消费专有属性后应用统一尺寸、样式与其他公共事件。
    apply_common_attributes(
        // 传入已经配置状态和变化观察器的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性与 Change 事件被二次映射。
        &["items", "active", "@change"],
    )
}

// 查找 SelectableList 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "SelectableList",
        "使用 <SelectableList items={selectable_items} />",
    )
}

// 查找元素上的具名属性。

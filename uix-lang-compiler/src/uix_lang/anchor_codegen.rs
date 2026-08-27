// 引入卫生事件变量所需的标识符、跨度与令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Anchor 属性、表达式、事件、数值与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, boolean_value,
    generate_event_handler_expression, generate_expression, numeric_value,
};

// 生成拥有类型化滚动目标并由运行时保持选择生命周期的 Anchor。
pub(crate) fn generate_anchor(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Anchor 自身绘制链接列表，不接受 UIX 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Anchor 元素。
            element.span,
            // 说明锚点导航不接受子节点。
            "<Anchor> 不接受子节点",
            // 给出规范数据绑定写法。
            "使用 <Anchor items={anchor_items} />",
        ));
    }

    // 查找构造器必需的 items 数据来源。
    let items_attribute = required_attribute(element, "items")?;
    // items 必须保持调用侧可迭代 AnchorItem 集合的 Rust 类型检查。
    let AttributeValue::Expression(items_expression) = &items_attribute.value else {
        // 返回条目集合表达式诊断。
        return Err(Diagnostic::new(
            // 指向非法 items 属性。
            items_attribute.span,
            // 说明公开运行时数据要求。
            "Anchor items 必须是可迭代 AnchorItem 表达式",
            // 给出规范数据引用写法。
            "使用 items={anchor_items}",
        ));
    };
    // 生成受限条目数据表达式。
    let items = generate_expression(&items_expression.expression, None)?;
    // 把数组或 Vec 统一收集为运行时构造器要求的拥有型集合。
    let anchor_items = quote! {
        ::std::iter::IntoIterator::into_iter((#items).clone())
            .collect::<::std::vec::Vec<::uix::prelude::AnchorItem>>()
    };

    // 由 Anchor 运行时取得条目集合所有权。
    let mut widget = quote! { ::uix::prelude::Anchor::new(#anchor_items) };
    // 显式 offsetTop 映射到组件拥有的滚动定位偏移。
    if let Some(attribute) = find_attribute(element, "offsetTop") {
        // 复用统一像素字面量与数值表达式生成契约。
        let offset_top = numeric_value(attribute)?;
        // 调用公开 target_offset 构建器。
        widget = quote! { (#widget).target_offset(#offset_top) };
    }
    // 显式 showInk 只声明活动锚点指示线绘制策略。
    if let Some(attribute) = find_attribute(element, "showInk") {
        // 解析布尔简写、字面量或表达式。
        let show_ink = boolean_value(attribute)?;
        // 调用公开指示线显示入口。
        widget = quote! { (#widget).show_ink(#show_ink) };
    }

    // 先物化公开叶节点，Change 处理器与样式由 View 契约拥有。
    let mut view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 可选变化事件观察 Anchor 已建立的 href 选择事实。
    if let Some(attribute) = find_attribute(element, "@change") {
        // 事件解析器应始终提供受限表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回内部形状保护诊断。
            return Err(Diagnostic::new(
                // 指向完整事件属性。
                attribute.span,
                // 说明事件处理器形状。
                "Anchor @change 必须是受限处理器表达式",
                // 给出带 href 载荷的规范写法。
                "使用 @change=\"on_anchor_change($event)\"",
            ));
        };
        // 创建卫生的 href 文本变量。
        let value = Ident::new("__uix_anchor_change", Span::mixed_site());
        // 生成裸处理器或显式载荷调用。
        let handler = generate_event_handler_expression(&expression.expression, &value, "@change")?;
        // 使用公开 View Change 注册入口保存处理器。
        view = quote! {
            // 注册只接收现有 href 文本借用的闭包。
            (#view).on_change_fn(move |#value| {
                // 丢弃处理器返回值并保留调用方滚动副作用。
                let _ = { #handler };
            })
        };
    }

    // 消费 Anchor 专有属性后应用统一尺寸、样式与其他公共事件。
    apply_common_attributes(
        // 传入已经配置条目、偏移和事件的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性与 Change 事件被二次映射。
        &["items", "offsetTop", "showInk", "@change"],
    )
}

// 查找元素上的具名属性。

// 查找 Anchor 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "Anchor",
        "使用 <Anchor items={anchor_items} />",
    )
}

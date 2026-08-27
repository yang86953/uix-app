// 引入卫生事件变量所需的标识符、跨度与令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性、单节点生成与可见节点判定入口。
use super::codegen::{apply_common_attributes, generate_node_view, is_renderable_node};
// 引入 Dropdown 属性、表达式、事件与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, Node, generate_event_handler_expression,
    generate_expression, literal_string,
};

// 生成拥有唯一静态 trigger View 与 keyed 选项树的 Dropdown。
pub(crate) fn generate_dropdown(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 收集忽略排版空白后的直接 trigger 节点。
    let triggers = element
        // 借用有序直接子节点。
        .children
        // 遍历全部源码子节点。
        .iter()
        // 只保留会生成 View 的节点。
        .filter(|node| is_renderable_node(node))
        // 物化集合以核对静态基数。
        .collect::<Vec<_>>();
    // Dropdown 必须在编译期确定唯一 trigger View。
    if triggers.len() != 1 {
        // 返回零或多 trigger 的确定诊断。
        return Err(Diagnostic::new(
            // 指向完整 Dropdown 元素。
            element.span,
            // 说明唯一直接子树约束。
            "<Dropdown> 必须包含且仅包含一个直接 trigger View",
            // 给出规范组合写法。
            "使用 <Dropdown items={items}><Button>更多</Button></Dropdown>",
        ));
    }
    // 只允许普通静态元素承担稳定 trigger 身份。
    let trigger = match triggers[0] {
        // 直接控制流会令 trigger 基数和身份不稳定。
        Node::Element(child)
            if child.control.is_some() || matches!(child.name.as_str(), "If" | "For") =>
        {
            // 返回动态 trigger 诊断。
            return Err(Diagnostic::new(
                // 指向完整 Dropdown 元素。
                element.span,
                // 说明直接控制流不满足静态 owner 契约。
                "<Dropdown> 的直接 trigger View 不能是 If 或 For",
                // 建议把控制流放入唯一静态容器内部。
                "使用一个静态 Container 包裹 If 或 For",
            ));
        }
        // 普通直接元素递归生成完整 ViewNode。
        Node::Element(_) => generate_node_view(triggers[0])?,
        // 可见裸文本不能承担可交互 trigger 身份。
        Node::Text(_) => {
            // 返回裸文本 trigger 诊断。
            return Err(Diagnostic::new(
                // 指向完整 Dropdown 元素。
                element.span,
                // 说明需要显式 View 身份。
                "<Dropdown> 不接受可见文本作为直接 trigger",
                // 给出按钮或容器修复路径。
                "把文字放入 Button、Label 或 Container",
            ));
        }
        // 插值同样没有静态组件身份。
        Node::Interpolation(_) => {
            // 返回插值 trigger 诊断。
            return Err(Diagnostic::new(
                // 指向完整 Dropdown 元素。
                element.span,
                // 说明插值不满足组合生命周期契约。
                "<Dropdown> 不接受插值作为直接 trigger",
                // 给出显式静态 View 修复路径。
                "把插值放入唯一静态 Label 或 Container",
            ));
        }
        // 成员块是声明载体，不能承担 trigger 身份。
        Node::WidgetMember(_) => {
            // 返回成员块 trigger 诊断。
            return Err(Diagnostic::new(
                // 指向完整 Dropdown 元素。
                element.span,
                // 说明成员块不满足组合生命周期契约。
                "<Dropdown> 不接受成员声明块作为直接 trigger",
                // 给出显式静态 View 修复路径。
                "把 @props/@state/@computed/@actions 移入 <Widget> 模板声明区",
            ));
        }
    };

    // 查找必需的 keyed 选项集合表达式。
    let items_attribute = required_attribute(element, "items")?;
    // items 必须保留调用方可迭代 DropdownItem 的 Rust 类型检查。
    let AttributeValue::Expression(items_expression) = &items_attribute.value else {
        // 返回数据表达式诊断。
        return Err(Diagnostic::new(
            // 指向非法 items 属性。
            items_attribute.span,
            // 说明公开运行时数据类型。
            "Dropdown items 必须是可迭代 DropdownItem 表达式",
            // 给出规范数据引用写法。
            "使用 items={action_items}",
        ));
    };
    // 生成受限 keyed 数据表达式。
    let items = generate_expression(&items_expression.expression, None)?;
    // 运行时 owner 接收 keyed 数据并持有完整 trigger ViewNode。
    let mut widget = quote! {
        ::uix::prelude::Dropdown::new("")
            .keyed_items((#items).clone())
            .trigger_view(#trigger)
    };
    // 可选 trigger 只接受批准的四个有限关键字。
    if let Some(attribute) = find_attribute(element, "trigger") {
        // 映射为共享公开 TriggerMode 枚举。
        let trigger_mode = trigger_tokens(attribute)?;
        // 在物化前应用触发方式。
        widget = quote! { (#widget).trigger(#trigger_mode) };
    }
    // trigger 子树由运行时 build_view_children 提供，因此外层仍是叶声明。
    let mut view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 可选变化事件观察运行时已经提交的稳定 key。
    if let Some(attribute) = find_attribute(element, "@change") {
        // 事件解析器应始终提供受限表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回内部形状保护诊断。
            return Err(Diagnostic::new(
                // 指向完整事件属性。
                attribute.span,
                // 说明事件处理器形状。
                "Dropdown @change 必须是受限处理器表达式",
                // 给出带稳定 key 载荷的规范写法。
                "使用 @change=\"on_action($event)\"",
            ));
        };
        // 创建卫生的稳定 key 文本变量。
        let value = Ident::new("__uix_dropdown_change", Span::mixed_site());
        // 生成裸处理器或显式载荷调用。
        let handler = generate_event_handler_expression(&expression.expression, &value, "@change")?;
        // 复用公开 Change 注册入口发布 stable key。
        view = quote! {
            (#view).on_change_fn(move |#value| {
                // 丢弃处理器返回值并保留调用方副作用。
                let _ = { #handler };
            })
        };
    }

    // 消费 Dropdown 专有属性并应用公共尺寸、样式、身份与其他公共事件。
    apply_common_attributes(
        // 传入已经配置 keyed 数据、trigger 子树和事件的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性和 Change 事件被二次映射。
        &["items", "trigger", "@change"],
    )
}

// 映射 Dropdown 的确定触发方式。
fn trigger_tokens(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // trigger 必须在编译期选择公开枚举变体。
    let value = literal_string(attribute, "Dropdown trigger")?;
    // 按批准文档关键字生成运行时枚举。
    match value.as_str() {
        // 点击触发映射 Click。
        "click" => Ok(quote! { ::uix::prelude::TriggerMode::Click }),
        // 悬停触发映射 Hover。
        "hover" => Ok(quote! { ::uix::prelude::TriggerMode::Hover }),
        // 焦点范围触发映射 Focus。
        "focus" => Ok(quote! { ::uix::prelude::TriggerMode::Focus }),
        // 右键触发映射 ContextMenu。
        "contextMenu" => Ok(quote! { ::uix::prelude::TriggerMode::ContextMenu }),
        // 其他关键字不得静默回退为 click。
        _ => Err(Diagnostic::new(
            // 指向完整 trigger 属性。
            attribute.span,
            // 陈述有限合法集合。
            "Dropdown trigger 只支持 click、hover、focus 或 contextMenu",
            // 给出完整修复集合。
            "使用 trigger=\"click\"、\"hover\"、\"focus\" 或 \"contextMenu\"",
        )),
    }
}

// 查找元素上的具名属性。

// 查找 Dropdown 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "Dropdown",
        "使用 <Dropdown items={items}><Button>更多</Button></Dropdown>",
    )
}

// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性、有序子树生成与可见节点判定入口。
use super::codegen::{apply_common_attributes, generate_children, is_renderable_node};
// 引入 Tooltip 属性、语法树、字面量与诊断契约。
use super::{Attribute, Diagnostic, Element, Node, literal_string, string_value};

// 生成由一个静态直接触发 View 包裹的 Tooltip 文字提示。
pub(crate) fn generate_tooltip(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 收集忽略源码排版空白后的直接触发节点。
    let triggers = element
        // 借用有序子节点列表。
        .children
        // 遍历每个直接子节点。
        .iter()
        // 仅保留会生成 View 的节点。
        .filter(|node| is_renderable_node(node))
        // 物化集合以核对静态基数。
        .collect::<Vec<_>>();
    // Tooltip 必须在编译期确定唯一触发 View。
    if triggers.len() != 1 {
        // 返回触发子树形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Tooltip 元素。
            element.span,
            // 说明唯一直接触发节点约束。
            "<Tooltip> 必须包含且仅包含一个直接触发 View",
            // 给出规范按钮触发写法。
            "使用 <Tooltip text=\"更多操作\"><Button>悬停</Button></Tooltip>",
        ));
    }
    // 直接控制流会令运行时触发 View 的基数不确定。
    if matches!(triggers[0], Node::Element(child) if matches!(child.name.as_str(), "If" | "For")) {
        // 返回动态基数诊断。
        return Err(Diagnostic::new(
            // 指向完整 Tooltip 元素。
            element.span,
            // 说明直接控制流不能充当稳定触发器。
            "<Tooltip> 的直接触发 View 不能是 If 或 For",
            // 建议把控制流放入唯一静态容器内部。
            "使用一个静态 Container 包裹 If 或 For",
        ));
    }

    // text 是运行时提示内容来源，必须显式提供。
    let text_attribute = required_attribute(element, "text")?;
    // 生成字符串字面量或受限字符串表达式。
    let text = string_value(text_attribute)?;
    // 临时借用提示文字，让运行时 Tooltip 复制并独占内容。
    let mut widget = quote! { ::uix_app::prelude::Tooltip::new(&*(#text)) };
    // 可选 placement 在编译期选择四种公开方向之一。
    if let Some(attribute) = find_attribute(element, "placement") {
        // 把文档关键字映射到公开运行时枚举。
        let placement = tooltip_placement(attribute)?;
        // 定位、越界翻转与裁剪继续由运行时负责。
        widget = quote! { (#widget).placement(#placement) };
    }
    // 可选 trigger 在编译期选择四种公开交互方式之一。
    if let Some(attribute) = find_attribute(element, "trigger") {
        // 把文档关键字映射到公开运行时枚举。
        let trigger = tooltip_trigger(attribute)?;
        // 输入、待触发计时和显隐继续由运行时负责。
        widget = quote! { (#widget).trigger(#trigger) };
    }
    // 按源码顺序生成唯一触发 View 及其内部控制流。
    let children = generate_children(&element.children)?;
    // 使用公开 ViewNode 子树承载触发器、样式与生命周期身份。
    let view = quote! { ::uix_app::prelude::ViewNode::new(#widget, #children) };
    // 消费 Tooltip 专有属性并应用公共尺寸、样式、身份与事件。
    apply_common_attributes(view, &element.attributes, &["text", "placement", "trigger"])
    // 结束 Tooltip 生成函数。
}

// 映射 Tooltip 的确定放置方向。
fn tooltip_placement(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // placement 必须在编译期选择运行时枚举变体。
    let value = literal_string(attribute, "Tooltip placement")?;
    // 按公开四方向生成对应枚举。
    match value.as_str() {
        // 上方提示。
        "top" => Ok(quote! { ::uix_app::prelude::TooltipPlacement::Top }),
        // 下方提示。
        "bottom" => Ok(quote! { ::uix_app::prelude::TooltipPlacement::Bottom }),
        // 左侧提示。
        "left" => Ok(quote! { ::uix_app::prelude::TooltipPlacement::Left }),
        // 右侧提示。
        "right" => Ok(quote! { ::uix_app::prelude::TooltipPlacement::Right }),
        // 其他关键字不能静默回退到 Top。
        _ => Err(Diagnostic::new(
            // 指向完整 placement 属性。
            attribute.span,
            // 陈述未知方向。
            format!("Tooltip placement={value:?} 不受支持"),
            // 给出完整合法集合。
            "使用 top、bottom、left 或 right",
        )),
    }
}

// 映射 Tooltip 的确定触发方式。
fn tooltip_trigger(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // trigger 必须在编译期选择运行时枚举变体。
    let value = literal_string(attribute, "Tooltip trigger")?;
    // 按公开四种交互生成对应枚举。
    match value.as_str() {
        // 指针悬停触发。
        "hover" => Ok(quote! { ::uix_app::prelude::TriggerMode::Hover }),
        // 主按钮点击触发。
        "click" => Ok(quote! { ::uix_app::prelude::TriggerMode::Click }),
        // 键盘焦点进入触发。
        "focus" => Ok(quote! { ::uix_app::prelude::TriggerMode::Focus }),
        // 上下文菜单请求触发。
        "contextMenu" => Ok(quote! { ::uix_app::prelude::TriggerMode::ContextMenu }),
        // 其他关键字不能静默回退到 Hover。
        _ => Err(Diagnostic::new(
            // 指向完整 trigger 属性。
            attribute.span,
            // 陈述未知触发方式。
            format!("Tooltip trigger={value:?} 不受支持"),
            // 给出完整合法集合。
            "使用 hover、click、focus 或 contextMenu",
        )),
    }
}

// 查找元素上的具名属性。

// 查找 Tooltip 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "Tooltip",
        "使用 <Tooltip text=\"更多操作\"><Button>悬停</Button></Tooltip>",
    )
}

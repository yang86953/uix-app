// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性、有序子树生成与可见节点判定入口。
use super::codegen::{apply_common_attributes, generate_children, is_renderable_node};
// 引入 Tooltip 属性、语法树与诊断契约。
use super::{Attribute, Diagnostic, Element, Node, string_value};

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
    let widget = quote! { ::uix::prelude::Tooltip::new(&*(#text)) };
    // 按源码顺序生成唯一触发 View 及其内部控制流。
    let children = generate_children(&element.children)?;
    // 使用公开 ViewNode 子树承载触发器、样式与生命周期身份。
    let view = quote! { ::uix::prelude::ViewNode::new(#widget, #children) };
    // 消费 Tooltip 专有文字属性并应用公共尺寸、样式、身份与事件。
    apply_common_attributes(view, &element.attributes, &["text"])
    // 结束 Tooltip 生成函数。
}

// 查找元素上的具名属性。
fn find_attribute<'a>(element: &'a Element, name: &str) -> Option<&'a Attribute> {
    // 解析器已保证同名属性唯一。
    element
        // 借用有序属性列表。
        .attributes
        // 遍历每个属性。
        .iter()
        // 返回首个名称匹配项。
        .find(|attribute| attribute.name == name)
    // 结束属性查找函数。
}

// 查找 Tooltip 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 缺失属性时构造确定诊断。
    find_attribute(element, name).ok_or_else(|| {
        // 返回完整必需属性错误。
        Diagnostic::new(
            // 指向完整 Tooltip 元素。
            element.span,
            // 点名缺失属性。
            format!("<Tooltip> 缺少必需的 {name} 属性"),
            // 给出最小合法写法。
            "使用 <Tooltip text=\"更多操作\"><Button>悬停</Button></Tooltip>",
        )
    })
    // 结束必需属性查找函数。
}

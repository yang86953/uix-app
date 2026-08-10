// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性、单节点生成与可见节点判定入口。
use super::codegen::{apply_common_attributes, generate_node_view, is_renderable_node};
// 引入 Popover 属性、语法树、值生成器与诊断契约。
use super::{Attribute, Diagnostic, Element, Node, literal_string, string_value};

// 生成由唯一静态触发 View 驱动的 Popover 气泡卡片。
pub(crate) fn generate_popover(element: &Element) -> Result<TokenStream, Diagnostic> {
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
    // Popover 必须在编译期确定唯一触发 View。
    if triggers.len() != 1 {
        // 返回触发子树形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Popover 元素。
            element.span,
            // 说明唯一直接触发节点约束。
            "<Popover> 必须包含且仅包含一个直接触发 View",
            // 给出规范按钮触发写法。
            "使用 <Popover content=\"详情\"><Button>查看</Button></Popover>",
        ));
    }
    // 只允许普通静态元素承担稳定触发器身份。
    let trigger = match triggers[0] {
        // 直接控制流会令运行时触发 View 的基数不确定。
        Node::Element(child) if matches!(child.name.as_str(), "If" | "For") => {
            // 返回动态基数诊断。
            return Err(Diagnostic::new(
                // 指向完整 Popover 元素。
                element.span,
                // 说明直接控制流不能充当稳定触发器。
                "<Popover> 的直接触发 View 不能是 If 或 For",
                // 建议把控制流放入唯一静态容器内部。
                "使用一个静态 Container 包裹 If 或 For",
            ));
        }
        // 普通直接元素递归生成完整 View。
        Node::Element(_) => generate_node_view(triggers[0])?,
        // 可见文本不能承担可交互触发器契约。
        Node::Text(_) => {
            // 返回文本触发器诊断。
            return Err(Diagnostic::new(
                // 指向完整 Popover 元素。
                element.span,
                // 说明裸文本不是登记的触发 View。
                "<Popover> 不接受可见文本作为直接触发器",
                // 给出显式静态 View 修复路径。
                "把文字放入 Button、Label 或 Container",
            ));
        }
        // 插值不能在编译期声明稳定组件身份。
        Node::Interpolation(_) => {
            // 返回插值触发器诊断。
            return Err(Diagnostic::new(
                // 指向完整 Popover 元素。
                element.span,
                // 说明插值不是登记的触发 View。
                "<Popover> 不接受插值作为直接触发器",
                // 给出显式静态 View 修复路径。
                "把插值放入唯一静态 Label 或 Container",
            ));
        }
    };

    // content 是运行时气泡内容来源，必须显式提供。
    let content_attribute = required_attribute(element, "content")?;
    // 生成字符串字面量或受限字符串表达式。
    let content = string_value(content_attribute)?;
    // 临时借用内容，让运行时 Popover 复制并独占字符串。
    let mut widget = quote! { ::uix::prelude::Popover::new(&*(#content)) };
    // 可选 trigger 只接受文档登记的 click 与 hover。
    if let Some(attribute) = find_attribute(element, "trigger") {
        // 把确定关键字映射到公开运行时枚举。
        let trigger_mode = popover_trigger(attribute)?;
        // 在物化触发子树前应用公开触发模式构建器。
        widget = quote! { (#widget).trigger(#trigger_mode) };
    }
    // 把唯一静态 View 交给运行时组件拥有触发与显隐生命周期。
    widget = quote! { (#widget).trigger_view(#trigger) };
    // 使用公开叶节点入口物化 WidgetComponent，子树仍由运行时提供器构建。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费 Popover 专有属性并应用公共尺寸、样式、身份与事件。
    apply_common_attributes(view, &element.attributes, &["content", "trigger"])
    // 结束 Popover 生成函数。
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

// 查找 Popover 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 缺失属性时构造确定诊断。
    find_attribute(element, name).ok_or_else(|| {
        // 返回完整必需属性错误。
        Diagnostic::new(
            // 指向完整 Popover 元素。
            element.span,
            // 点名缺失属性。
            format!("<Popover> 缺少必需的 {name} 属性"),
            // 给出最小合法写法。
            "使用 <Popover content=\"详情\"><Button>查看</Button></Popover>",
        )
    })
    // 结束必需属性查找函数。
}

// 映射 Popover 的确定触发方式。
fn popover_trigger(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // trigger 必须在编译期选择运行时枚举变体。
    let value = literal_string(attribute, "Popover trigger")?;
    // 按文档登记关键字生成公开枚举。
    match value.as_str() {
        // 点击触发映射到 Click。
        "click" => Ok(quote! { ::uix::prelude::PopoverTrigger::Click }),
        // 悬停触发映射到 Hover。
        "hover" => Ok(quote! { ::uix::prelude::PopoverTrigger::Hover }),
        // 其他关键字不能静默回退。
        _ => Err(Diagnostic::new(
            // 指向完整 trigger 属性。
            attribute.span,
            // 陈述未知触发方式。
            format!("Popover trigger={value:?} 不受支持"),
            // 给出完整合法集合。
            "使用 click 或 hover",
        )),
    }
    // 结束触发方式映射函数。
}

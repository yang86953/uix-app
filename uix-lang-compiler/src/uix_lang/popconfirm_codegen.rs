// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性、单节点生成与可见节点判定入口。
use super::codegen::{apply_common_attributes, generate_node_view, is_renderable_node};
// 引入 Popconfirm 属性、表达式、事件与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, Node, boolean_value,
    generate_handler_expression, literal_string, string_value,
};

// 生成拥有唯一静态 trigger View 与同步操作回调的 Popconfirm。
pub(crate) fn generate_popconfirm(element: &Element) -> Result<TokenStream, Diagnostic> {
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
    // Popconfirm 必须在编译期确定唯一 trigger View。
    if triggers.len() != 1 {
        // 返回零或多 trigger 的确定诊断。
        return Err(Diagnostic::new(
            // 指向完整 Popconfirm 元素。
            element.span,
            // 说明唯一直接子树约束。
            "<Popconfirm> 必须包含且仅包含一个直接 trigger View",
            // 给出规范组合写法。
            "使用 <Popconfirm title=\"确定删除？\"><Button>删除</Button></Popconfirm>",
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
                // 指向完整 Popconfirm 元素。
                element.span,
                // 说明直接控制流不满足静态 owner 契约。
                "<Popconfirm> 的直接 trigger View 不能是 If 或 For",
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
                // 指向完整 Popconfirm 元素。
                element.span,
                // 说明需要显式 View 身份。
                "<Popconfirm> 不接受可见文本作为直接 trigger",
                // 给出按钮或容器修复路径。
                "把文字放入 Button、Label 或 Container",
            ));
        }
        // 插值同样没有静态组件身份。
        Node::Interpolation(_) => {
            // 返回插值 trigger 诊断。
            return Err(Diagnostic::new(
                // 指向完整 Popconfirm 元素。
                element.span,
                // 说明插值不满足组合生命周期契约。
                "<Popconfirm> 不接受插值作为直接 trigger",
                // 给出显式静态 View 修复路径。
                "把插值放入唯一静态 Label 或 Container",
            ));
        }
        // 成员块是声明载体，不能承担 trigger 身份。
        Node::WidgetMember(_) => {
            // 返回成员块 trigger 诊断。
            return Err(Diagnostic::new(
                // 指向完整 Popconfirm 元素。
                element.span,
                // 说明成员块不满足组合生命周期契约。
                "<Popconfirm> 不接受成员声明块作为直接 trigger",
                // 给出显式静态 View 修复路径。
                "把 @props/@state/@computed/@actions 移入 <Widget> 模板声明区",
            ));
        }
    };

    // title 是确认语义不可缺失的必需文本。
    let title_attribute = required_attribute(element, "title")?;
    // 生成统一字符串值令牌。
    let title = string_value(title_attribute)?;
    // 运行时 owner 接收标题与完整 trigger ViewNode。
    let mut widget = quote! {
        ::uix::prelude::Popconfirm::new()
            .title(#title)
            .trigger_view(#trigger)
    };
    // 可选确认按钮文字接受字符串字面量或受限表达式。
    if let Some(attribute) = find_attribute(element, "confirmText") {
        // 生成统一字符串值令牌。
        let confirm_text = string_value(attribute)?;
        // 把确认文字交给运行时复制保存。
        widget = quote! { (#widget).confirm_text(#confirm_text) };
    }
    // 可选取消按钮文字接受字符串字面量或受限表达式。
    if let Some(attribute) = find_attribute(element, "cancelText") {
        // 生成统一字符串值令牌。
        let cancel_text = string_value(attribute)?;
        // 把取消文字交给运行时复制保存。
        widget = quote! { (#widget).cancel_text(#cancel_text) };
    }
    // 可选 placement 只接受运行时已经实现的六个方向。
    if let Some(attribute) = find_attribute(element, "placement") {
        // 映射为公开 PopconfirmPlacement 枚举。
        let placement = placement_tokens(attribute)?;
        // 在物化前应用确定位置配置。
        widget = quote! { (#widget).placement(#placement) };
    }
    // 可选 arrow 接受布尔简写、字面量或受限表达式。
    if let Some(attribute) = find_attribute(element, "arrow") {
        // 生成统一布尔值令牌。
        let arrow = boolean_value(attribute)?;
        // 把箭头显隐交给运行时几何 owner。
        widget = quote! { (#widget).arrow(#arrow) };
    }
    // 可选 icon 接受布尔简写、字面量或受限表达式。
    if let Some(attribute) = find_attribute(element, "icon") {
        // 生成统一布尔值令牌。
        let icon = boolean_value(attribute)?;
        // 把警告图标显隐交给运行时绘制 owner。
        widget = quote! { (#widget).icon(#icon) };
    }
    // 可选确认事件映射为实例拥有的同步无载荷回调。
    if let Some(attribute) = find_attribute(element, "@confirm") {
        // 解析不携带额外事件载荷的受限处理器表达式。
        let handler = handler_expression(attribute, "@confirm")?;
        // 丢弃处理器返回值并保留确认副作用。
        widget = quote! { (#widget).on_confirm(move || { let _ = { #handler }; }) };
    }
    // 可选取消事件覆盖按钮、Escape、外部点击、trigger 重激活与焦点离开。
    if let Some(attribute) = find_attribute(element, "@cancel") {
        // 解析不携带额外事件载荷的受限处理器表达式。
        let handler = handler_expression(attribute, "@cancel")?;
        // 丢弃处理器返回值并保留取消副作用。
        widget = quote! { (#widget).on_cancel(move || { let _ = { #handler }; }) };
    }

    // trigger 子树由运行时 build_view_children 提供，因此外层仍是叶声明。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费 Popconfirm 专有属性并应用公共尺寸、样式、身份与其他公共事件。
    apply_common_attributes(
        // 传入已经配置 trigger、几何和回调的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性和事件被二次映射。
        &[
            "title",
            "confirmText",
            "cancelText",
            "placement",
            "arrow",
            "icon",
            "@confirm",
            "@cancel",
        ],
    )
}

// 映射 Popconfirm 的六个确定位置。
fn placement_tokens(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // placement 必须在编译期选择公开枚举变体。
    let value = literal_string(attribute, "Popconfirm placement")?;
    // 按批准文档关键字生成运行时枚举。
    match value.as_str() {
        // 顶部左对齐映射 TopLeft。
        "topLeft" => Ok(quote! { ::uix::prelude::PopconfirmPlacement::TopLeft }),
        // 顶部默认映射 Top。
        "top" => Ok(quote! { ::uix::prelude::PopconfirmPlacement::Top }),
        // 顶部右对齐映射 TopRight。
        "topRight" => Ok(quote! { ::uix::prelude::PopconfirmPlacement::TopRight }),
        // 底部左对齐映射 BottomLeft。
        "bottomLeft" => Ok(quote! { ::uix::prelude::PopconfirmPlacement::BottomLeft }),
        // 底部默认映射 Bottom。
        "bottom" => Ok(quote! { ::uix::prelude::PopconfirmPlacement::Bottom }),
        // 底部右对齐映射 BottomRight。
        "bottomRight" => Ok(quote! { ::uix::prelude::PopconfirmPlacement::BottomRight }),
        // 其他关键字不得静默回退为 top。
        _ => Err(Diagnostic::new(
            // 指向完整 placement 属性。
            attribute.span,
            // 陈述有限合法集合。
            "Popconfirm placement 只支持 topLeft、top、topRight、bottomLeft、bottom 或 bottomRight",
            // 给出完整修复集合。
            "使用 placement=\"top\" 或六个已登记位置之一",
        )),
    }
}

// 解析 Popconfirm 无载荷操作事件。
fn handler_expression(attribute: &Attribute, name: &str) -> Result<TokenStream, Diagnostic> {
    // 事件解析器应始终提供受限表达式。
    let AttributeValue::Expression(expression) = &attribute.value else {
        // 返回内部形状保护诊断。
        return Err(Diagnostic::new(
            // 指向完整事件属性。
            attribute.span,
            // 明确处理器表达式要求。
            format!("Popconfirm {name} 必须是受限处理器表达式"),
            // 给出无载荷处理器写法。
            format!("使用 {name}=\"on_popconfirm_action\""),
        ));
    };
    // 复用统一裸处理器或显式调用生成逻辑。
    generate_handler_expression(&expression.expression, None)
}

// 查找元素上的具名属性。

// 查找 Popconfirm 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "Popconfirm",
        "使用 <Popconfirm title=\"确定删除？\"><Button>删除</Button></Popconfirm>",
    )
}

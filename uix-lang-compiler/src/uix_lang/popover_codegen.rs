// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性、单节点生成与可见节点判定入口。
use super::codegen::{apply_common_attributes, generate_node_view, is_renderable_node};
// 引入 Popover 属性、语法树、值生成器与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, Node, boolean_value, generate_expression,
    literal_string, string_value,
};

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
        // 成员块是声明载体，不能承担触发生命周期契约。
        Node::WidgetMember(_) => {
            // 返回成员块触发器诊断。
            return Err(Diagnostic::new(
                // 指向完整 Popover 元素。
                element.span,
                // 说明成员块不是登记的触发 View。
                "<Popover> 不接受成员声明块作为直接触发器",
                // 给出声明区归位修复路径。
                "把 @props/@state/@computed/@actions 移入 <Widget> 模板声明区",
            ));
        }
    };

    // content 是运行时气泡内容来源，必须显式提供。
    let content_attribute = required_attribute(element, "content")?;
    // 生成字符串字面量或受限字符串表达式。
    let content = string_value(content_attribute)?;
    // 临时借用内容，让运行时 Popover 复制并独占字符串。
    let mut widget = quote! { ::uix::prelude::Popover::new(&*(#content)) };
    // 可选标题接受字符串字面量或受限字符串表达式。
    if let Some(attribute) = find_attribute(element, "title") {
        // 生成运行时拥有的标题字符串。
        let title = string_value(attribute)?;
        // 构造期间借用标题，运行时负责复制并绘制。
        widget = quote! { (#widget).title(&*(#title)) };
    }
    // 可选 placement 在编译期选择十二种公开方向之一。
    if let Some(attribute) = find_attribute(element, "placement") {
        // 把文档关键字映射到公开运行时枚举。
        let placement = popover_placement(attribute)?;
        // 定位与边缘翻转仍由运行时负责。
        widget = quote! { (#widget).placement(#placement) };
    }
    // 可选 arrow 只声明是否绘制指向触发器的箭头。
    if let Some(attribute) = find_attribute(element, "arrow") {
        // 接受布尔简写、字面量或受限表达式。
        let arrow = boolean_value(attribute)?;
        // 箭头几何继续由运行时按最终方向计算。
        widget = quote! { (#widget).arrow(#arrow) };
    }
    // 可选 trigger 接受运行时完整的 click、hover 与 focus 集合。
    if let Some(attribute) = find_attribute(element, "trigger") {
        // 把确定关键字映射到公开运行时枚举。
        let trigger_mode = popover_trigger(attribute)?;
        // 在物化触发子树前应用公开触发模式构建器。
        widget = quote! { (#widget).trigger(#trigger_mode) };
    }
    // 可选 open 必须保留声明端 State<bool> 句柄。
    if let Some(attribute) = find_attribute(element, "open") {
        // 解析受控状态表达式并保留 Rust 类型检查。
        let open = state_expression(attribute)?;
        // 运行时克隆状态句柄并负责用户关闭写回。
        widget = quote! { (#widget).controlled_open(&(#open)) };
    }
    // 把唯一静态 View 交给运行时组件拥有触发与显隐生命周期。
    widget = quote! { (#widget).trigger_view(#trigger) };
    // 使用公开叶节点入口物化 Widget，子树仍由运行时提供器构建。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费 Popover 专有属性并应用公共尺寸、样式、身份与事件。
    apply_common_attributes(
        // 传入已配置完整运行时契约的 Popover View。
        view,
        // 保留公共属性的源码顺序。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["content", "title", "placement", "arrow", "trigger", "open"],
    )
    // 结束 Popover 生成函数。
}

// 解析 Popover 的受控打开状态表达式。
fn state_expression(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 字面量不能提供可订阅和可写回的 State<bool> 句柄。
    let AttributeValue::Expression(expression) = &attribute.value else {
        // 返回带来源位置的受控状态诊断。
        return Err(Diagnostic::new(
            // 指向非法 open 属性。
            attribute.span,
            // 明确状态句柄类型要求。
            "Popover open 必须是 State<bool> 表达式",
            // 给出受控绑定写法。
            "使用 open={popover_open}",
        ));
    };
    // 生成受限 Rust 表达式并把最终类型检查交给调用 crate。
    generate_expression(&expression.expression, None)
}

// 映射 Popover 的确定放置方向。
fn popover_placement(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // placement 必须在编译期选择运行时枚举变体。
    let value = literal_string(attribute, "Popover placement")?;
    // 按公开十二方向生成对应枚举。
    match value.as_str() {
        // 上方居中。
        "top" => Ok(quote! { ::uix::prelude::PopoverPlacement::Top }),
        // 上方左对齐。
        "topLeft" => Ok(quote! { ::uix::prelude::PopoverPlacement::TopLeft }),
        // 上方右对齐。
        "topRight" => Ok(quote! { ::uix::prelude::PopoverPlacement::TopRight }),
        // 下方居中。
        "bottom" => Ok(quote! { ::uix::prelude::PopoverPlacement::Bottom }),
        // 下方左对齐。
        "bottomLeft" => Ok(quote! { ::uix::prelude::PopoverPlacement::BottomLeft }),
        // 下方右对齐。
        "bottomRight" => Ok(quote! { ::uix::prelude::PopoverPlacement::BottomRight }),
        // 左侧居中。
        "left" => Ok(quote! { ::uix::prelude::PopoverPlacement::Left }),
        // 左侧顶部对齐。
        "leftTop" => Ok(quote! { ::uix::prelude::PopoverPlacement::LeftTop }),
        // 左侧底部对齐。
        "leftBottom" => Ok(quote! { ::uix::prelude::PopoverPlacement::LeftBottom }),
        // 右侧居中。
        "right" => Ok(quote! { ::uix::prelude::PopoverPlacement::Right }),
        // 右侧顶部对齐。
        "rightTop" => Ok(quote! { ::uix::prelude::PopoverPlacement::RightTop }),
        // 右侧底部对齐。
        "rightBottom" => Ok(quote! { ::uix::prelude::PopoverPlacement::RightBottom }),
        // 其他关键字不能静默回退到 Top。
        _ => Err(Diagnostic::new(
            // 指向完整 placement 属性。
            attribute.span,
            // 陈述未知方向。
            format!("Popover placement={value:?} 不受支持"),
            // 给出完整合法集合。
            "使用 top、topLeft、topRight、bottom、bottomLeft、bottomRight、left、leftTop、leftBottom、right、rightTop 或 rightBottom",
        )),
    }
}

// 查找元素上的具名属性。

// 查找 Popover 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "Popover",
        "使用 <Popover content=\"详情\"><Button>查看</Button></Popover>",
    )
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
        // 焦点进入触发映射到 Focus。
        "focus" => Ok(quote! { ::uix::prelude::PopoverTrigger::Focus }),
        // 其他关键字不能静默回退。
        _ => Err(Diagnostic::new(
            // 指向完整 trigger 属性。
            attribute.span,
            // 陈述未知触发方式。
            format!("Popover trigger={value:?} 不受支持"),
            // 给出完整合法集合。
            "使用 click、hover 或 focus",
        )),
    }
    // 结束触发方式映射函数。
}

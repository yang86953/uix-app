// 引入卫生事件变量所需的标识符、跨度与令牌流。
use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与有序子树生成入口。
use super::codegen::{apply_common_attributes, generate_children};
// 引入 Tabs 属性、节点、表达式、事件与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, Node, generate_expression,
    generate_event_handler_expression,
};

// 生成由稳定 key 控制且保留全部面板状态的 Tabs 容器。
pub(crate) fn generate_tabs(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 直接 If/For 会动态改变面板数量，无法与静态 items 保持一一对应。
    if let Some(control) = element.children.iter().find_map(|node| match node {
        // 只拒绝 Tabs 的直接控制流子节点，面板内部仍可使用控制流。
        Node::Element(child) if child.control.is_some() => Some(child),
        // 其他静态直接节点保持源码顺序。
        _ => None,
    }) {
        // 返回带直接控制节点位置的确定诊断。
        return Err(Diagnostic::new(
            // 指向非法直接控制节点。
            control.span,
            // 说明静态面板数量约束。
            "Tabs 不接受直接 If/For 面板",
            // 引导把控制流放入稳定面板内部。
            "保持 items 与直接面板数量一一对应，并把 If/For 放入面板内部",
        ));
    }

    // 查找必需的标签元数据表达式。
    let items_attribute = required_attribute(element, "items")?;
    // items 必须保留调用侧可迭代 Tab 集合的 Rust 类型检查。
    let AttributeValue::Expression(items_expression) = &items_attribute.value else {
        // 返回标签元数据表达式诊断。
        return Err(Diagnostic::new(
            // 指向非法 items 属性。
            items_attribute.span,
            // 说明公开运行时数据要求。
            "Tabs items 必须是可迭代 Tab 表达式",
            // 给出规范数据引用写法。
            "使用 items={tabs}",
        ));
    };
    // 生成受限标签元数据表达式。
    let items = generate_expression(&items_expression.expression, None)?;
    // 把数组或 Vec 统一收集为运行时拥有的标签集合。
    let tabs = quote! {
        ::std::iter::IntoIterator::into_iter((#items).clone())
            .collect::<::std::vec::Vec<::uix::prelude::Tab>>()
    };

    // 查找必需的活动 key 状态表达式。
    let active_attribute = required_attribute(element, "activeKey")?;
    // activeKey 必须提供可订阅和可写回的 State<String> 句柄。
    let AttributeValue::Expression(active_expression) = &active_attribute.value else {
        // 返回受控状态句柄诊断。
        return Err(Diagnostic::new(
            // 指向非法 activeKey 属性。
            active_attribute.span,
            // 明确状态句柄类型要求。
            "Tabs activeKey 必须是 State<String> 表达式",
            // 给出规范受控绑定写法。
            "使用 activeKey={active_tab}",
        ));
    };
    // 生成受限活动 key 状态表达式。
    let active_key = generate_expression(&active_expression.expression, None)?;

    // 运行时取得静态标签集合并绑定活动 key 状态。
    let widget = quote! {
        ::uix::prelude::Tabs::new()
            .tabs(#tabs)
            .active_key(&(#active_key))
    };
    // 按源码顺序生成全部静态直接面板。
    let children = generate_children(&element.children)?;
    // 通过公开 ViewNode 让 Tabs 与所有面板共同保留生命周期。
    let mut view = quote! { ::uix::prelude::ViewNode::new(#widget, #children) };

    // 可选变化事件观察 Tabs 已写回的稳定 key。
    if let Some(attribute) = find_attribute(element, "@change") {
        // 事件解析器应始终提供受限表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回内部形状保护诊断。
            return Err(Diagnostic::new(
                // 指向完整事件属性。
                attribute.span,
                // 说明事件处理器形状。
                "Tabs @change 必须是受限处理器表达式",
                // 给出带稳定 key 载荷的规范写法。
                "使用 @change=\"on_tab_change($event)\"",
            ));
        };
        // 创建卫生的稳定 key 文本变量。
        let value = Ident::new("__uix_tabs_change", Span::mixed_site());
        // 生成裸处理器或显式载荷调用。
        let handler = generate_event_handler_expression(&expression.expression, &value, "@change")?;
        // 使用公开 View Change 注册入口保存处理器。
        view = quote! {
            (#view).on_change_fn(move |#value| {
                // 丢弃处理器返回值并保留调用方副作用。
                let _ = { #handler };
            })
        };
    }

    // 消费 Tabs 专有属性并应用统一尺寸、样式、身份与其他公共事件。
    apply_common_attributes(
        // 传入已经配置元数据、状态、面板与事件的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止 Tabs 专有属性和 Change 事件被二次映射。
        &["items", "activeKey", "@change"],
    )
}

// 查找元素上的具名属性。
fn find_attribute<'a>(element: &'a Element, name: &str) -> Option<&'a Attribute> {
    // 解析器已经保证同名属性唯一。
    element
        // 借用有序属性集合。
        .attributes
        // 遍历每个属性。
        .iter()
        // 返回首个名称匹配项。
        .find(|attribute| attribute.name == name)
}

// 查找 Tabs 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 缺失属性时构造确定诊断。
    find_attribute(element, name).ok_or_else(|| {
        // 返回完整必需属性错误。
        Diagnostic::new(
            // 指向完整 Tabs 元素。
            element.span,
            // 点名缺失属性。
            format!("<Tabs> 缺少必需的 {name} 属性"),
            // 给出最小合法写法。
            "使用 <Tabs items={tabs} activeKey={active_tab}>...</Tabs>",
        )
    })
}

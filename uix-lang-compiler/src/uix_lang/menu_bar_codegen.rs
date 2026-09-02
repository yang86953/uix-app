// 引入卫生事件变量所需的标识符、跨度与令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与可见节点判定入口。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 MenuBar 属性、表达式、事件与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, generate_event_handler_expression,
    generate_expression,
};

// 生成拥有 keyed 顶级菜单集与 Change 稳定 key 的 MenuBar。
pub(crate) fn generate_menu_bar(element: &Element) -> Result<TokenStream, Diagnostic> {
    // MenuBar 自身绘制入口行与弹层，不接受 UIX 内容子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 MenuBar 元素。
            element.span,
            // 说明菜单栏不接受子节点。
            "<MenuBar> 不接受子节点",
            // 给出规范类型化数据写法。
            "使用 <MenuBar menus={app_menus} @change=\"on_menu($event)\" />",
        ));
    }

    // 查找必需的 keyed 顶级菜单集合表达式。
    let menus_attribute = required_attribute(element, "menus")?;
    // menus 必须保留调用方可迭代 MenuBarMenu 的 Rust 类型检查。
    let AttributeValue::Expression(menus_expression) = &menus_attribute.value else {
        // 返回数据表达式诊断。
        return Err(Diagnostic::new(
            // 指向非法 menus 属性。
            menus_attribute.span,
            // 说明公开运行时数据类型。
            "MenuBar menus 必须是可迭代 MenuBarMenu 表达式",
            // 给出规范数据引用写法。
            "使用 menus={[MenuBarMenu('文件', 'file').item(MenuBarItem('新建', 'new'))]}",
        ));
    };
    // 生成受限 keyed 数据表达式。
    let menus = generate_expression(&menus_expression.expression, None)?;
    // 运行时 owner 接收 keyed 数据并持有完整菜单栏内核。
    let widget = quote! {
        ::uix::prelude::MenuBar::new().keyed_menus((#menus).clone())
    };
    // MenuBar 物化为公开叶 View。
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
                "MenuBar @change 必须是受限处理器表达式",
                // 给出带稳定 key 载荷的规范写法。
                "使用 @change=\"on_menu($event)\"",
            ));
        };
        // 创建卫生的稳定 key 文本变量。
        let value = Ident::new("__uix_menu_bar_change", Span::mixed_site());
        // 生成裸处理器或显式载荷调用。
        let handler = generate_event_handler_expression(&expression.expression, &value, "@change")?;
        // 复用公开 Change 注册入口发布 stable key；载荷物化为拥有型 String。
        view = quote! {
            (#view).on_change_fn(move |#value: &str| {
                // 声明层消费拥有型 key，借用载荷在此统一物化。
                let #value = #value.to_string();
                // 丢弃处理器返回值并保留调用方副作用。
                let _ = { #handler };
            })
        };
    }

    // 消费 MenuBar 专有属性并应用统一尺寸、样式、身份与其他公共事件。
    apply_common_attributes(
        // 传入已经配置 keyed 数据与事件的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性和 Change 事件被二次映射。
        &["menus", "@change"],
    )
}

// 查找 MenuBar 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "MenuBar",
        "使用 <MenuBar menus={app_menus} @change=\"on_menu($event)\" />",
    )
}

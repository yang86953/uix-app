// 引入卫生事件变量所需的标识符、跨度与令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Menu 属性、表达式、事件、布尔值与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, boolean_value,
    generate_event_handler_expression, generate_expression,
};

// 生成拥有 typed MenuItem 树与双受控状态的 Menu。
pub(crate) fn generate_menu(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Menu 自身绘制完整菜单树，不接受 UIX 内容子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Menu 元素。
            element.span,
            // 说明菜单不接受子节点。
            "<Menu> 不接受子节点",
            // 给出规范类型化数据写法。
            "使用 <Menu items={menu_items} selectedKey={selected} openKeys={open} />",
        ));
    }

    // 解析必需的 typed 菜单树表达式。
    let items = required_expression(element, "items", "可迭代 MenuItem<K> 表达式")?;
    // 解析必需的单选 State<Option<K>> 句柄。
    let selected = required_expression(element, "selectedKey", "State<Option<K>> 表达式")?;
    // 解析必需的展开 State<Vec<K>> 句柄。
    let open = required_expression(element, "openKeys", "State<Vec<K>> 表达式")?;

    // 由公开受控构造器建立 typed key 到绘制层字符串的唯一映射。
    let mut widget = quote! {
        ::uix::prelude::Menu::controlled(
            (#items).clone(),
            &(#selected),
            &(#open),
        )
    };
    // UIX 缺省使用文档约定的垂直模式。
    let mode = mode_tokens(element)?;
    // 显式设置方向，避免依赖 Rust API 的兼容默认值。
    widget = quote! { (#widget).mode(#mode) };
    // 可选 collapsible 只控制含 children 的菜单组。
    if let Some(attribute) = find_attribute(element, "collapsible") {
        // 复用统一布尔字面量或表达式诊断。
        let collapsible = boolean_value(attribute)?;
        // 把组展开策略交给运行时 Menu。
        widget = quote! { (#widget).collapsible(#collapsible) };
    }

    // Menu 物化为公开叶 View。
    let mut view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 可选选择事件观察运行时已经提交的稳定 key。
    if let Some(attribute) = find_attribute(element, "@select") {
        // 事件解析器应始终提供受限表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回内部形状保护诊断。
            return Err(Diagnostic::new(
                // 指向完整事件属性。
                attribute.span,
                // 说明事件处理器形状。
                "Menu @select 必须是受限处理器表达式",
                // 给出带稳定 key 载荷的规范写法。
                "使用 @select=\"on_menu_select($event)\"",
            ));
        };
        // 创建卫生的稳定 key 文本变量。
        let value = Ident::new("__uix_menu_select", Span::mixed_site());
        // 生成裸处理器或显式载荷调用。
        let handler = generate_event_handler_expression(&expression.expression, &value, "@select")?;
        // 复用公开 Change 语义注册入口承接 Menu 选择事实。
        view = quote! {
            (#view).on_change_fn(move |#value| {
                // 丢弃处理器返回值并保留调用方副作用。
                let _ = { #handler };
            })
        };
    }

    // 消费 Menu 专有属性并应用统一尺寸、样式、身份与其他公共事件。
    apply_common_attributes(
        // 传入已经配置数据、状态、方向与事件的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止 Menu 专有属性和 Select 事件被二次映射。
        &[
            "items",
            "selectedKey",
            "openKeys",
            "mode",
            "collapsible",
            "@select",
        ],
    )
}

// 解析 Menu 的必需表达式属性。
fn required_expression(
    element: &Element,
    name: &str,
    expected: &str,
) -> Result<TokenStream, Diagnostic> {
    // 查找具名必需属性。
    let attribute = find_attribute(element, name).ok_or_else(|| {
        // 缺失属性时返回确定诊断。
        Diagnostic::new(
            // 指向完整 Menu 元素。
            element.span,
            // 点名缺失属性。
            format!("<Menu> 缺少必需的 {name} 属性"),
            // 给出完整最小契约。
            "使用 <Menu items={menu_items} selectedKey={selected} openKeys={open} />",
        )
    })?;
    // 状态句柄与数据集合都必须保留表达式形状。
    let AttributeValue::Expression(expression) = &attribute.value else {
        // 返回具体期望类型诊断。
        return Err(Diagnostic::new(
            // 指向非法属性。
            attribute.span,
            // 说明公开运行时类型要求。
            format!("Menu {name} 必须是 {expected}"),
            // 给出规范表达式绑定写法。
            format!("使用 {name}={{...}}"),
        ));
    };
    // 生成受限 Rust 表达式并保留调用 crate 的完整类型检查。
    generate_expression(&expression.expression, None)
}

// 生成垂直、水平或始终展开的内联模式枚举路径。
fn mode_tokens(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 缺省模式固定为文档声明的 vertical。
    let Some(attribute) = find_attribute(element, "mode") else {
        // 返回公开垂直模式路径。
        return Ok(quote! { ::uix::prelude::MenuMode::Vertical });
    };
    // mode 是有限关键字，不接受动态表达式。
    let AttributeValue::Literal(value) = &attribute.value else {
        // 返回模式字面量诊断。
        return Err(mode_diagnostic(attribute));
    };
    // 映射三个公开文档关键字。
    match value.as_str() {
        // 水平模式映射公开枚举。
        "horizontal" => Ok(quote! { ::uix::prelude::MenuMode::Horizontal }),
        // 垂直模式映射公开枚举。
        "vertical" => Ok(quote! { ::uix::prelude::MenuMode::Vertical }),
        // 内联模式映射始终展开的公开枚举。
        "inline" => Ok(quote! { ::uix::prelude::MenuMode::Inline }),
        // 其他关键字不在首版 UIX 契约。
        _ => Err(mode_diagnostic(attribute)),
    }
}

// 构造 Menu mode 的统一诊断。
fn mode_diagnostic(attribute: &Attribute) -> Diagnostic {
    // 返回带来源位置的有限关键字错误。
    Diagnostic::new(
        // 指向 mode 属性。
        attribute.span,
        // 说明允许值集合。
        "Menu mode 只支持 vertical、horizontal 或 inline",
        // 给出全部已登记模式写法。
        "使用 mode=\"vertical\"、mode=\"horizontal\" 或 mode=\"inline\"",
    )
}

// 查找元素上的具名属性。

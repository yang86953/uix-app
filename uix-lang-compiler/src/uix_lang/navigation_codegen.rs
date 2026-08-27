// 引入卫生事件变量所需的标识符、跨度与令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Navigation 属性、表达式、事件、字符串与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, generate_event_handler_expression,
    generate_expression, string_value,
};

// 生成只拥有侧栏外壳并复用受控 Menu 的 Navigation。
pub(crate) fn generate_navigation(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Navigation 的完整菜单树只能来自 items，不接受第二套 UIX 内容子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回唯一数据树形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Navigation 元素。
            element.span,
            // 说明侧栏组合不接受子节点。
            "<Navigation> 不接受子节点",
            // 给出规范受控组合写法。
            "使用 <Navigation title=\"应用\" items={items} activeKey={active} openKeys={open} collapsed={collapsed} />",
        ));
    }

    // 标题允许字面量或字符串表达式，并由外壳精确拥有。
    let title = string_value(required_attribute(element, "title")?)?;
    // 菜单数据树必须保留调用方的 MenuItem<K> 类型。
    let items = required_expression(element, "items", "可迭代 MenuItem<K> 表达式")?;
    // 活动项必须是调用方拥有的 State<Option<K>>。
    let active = required_expression(element, "activeKey", "State<Option<K>> 表达式")?;
    // 展开组必须是调用方拥有的 State<Vec<K>>。
    let open = required_expression(element, "openKeys", "State<Vec<K>> 表达式")?;
    // 整栏折叠必须是调用方拥有的 State<bool>。
    let collapsed = required_expression(element, "collapsed", "State<bool> 表达式")?;
    // 可选版本精确传递调用方文本，不注入 locale 回退。
    let version = optional_string(element, "version")?;

    // 公开组合构造器建立 NavigationShell 与唯一受控 Menu 子树。
    let mut view = quote! {
        ::uix::prelude::Navigation::controlled(
            (#title).to_string(),
            (#items).clone(),
            &(#active),
            &(#open),
            &(#collapsed),
            #version,
        )
    };
    // 可选选择事件观察 Menu 已提交并向外壳冒泡的稳定 key。
    if let Some(attribute) = find_attribute(element, "@select") {
        // 事件解析器应始终提供受限表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回内部形状保护诊断。
            return Err(Diagnostic::new(
                // 指向完整事件属性。
                attribute.span,
                // 说明事件处理器形状。
                "Navigation @select 必须是受限处理器表达式",
                // 给出带稳定 key 载荷的规范写法。
                "使用 @select=\"on_navigation_select($event)\"",
            ));
        };
        // 创建卫生的稳定 key 文本变量。
        let value = Ident::new("__uix_navigation_select", Span::mixed_site());
        // 生成裸处理器或显式载荷调用。
        let handler = generate_event_handler_expression(&expression.expression, &value, "@select")?;
        // 根 View 的 Change 观察器承接唯一 Menu 子树冒泡的选择事实。
        view = quote! {
            (#view).on_change_fn(move |#value| {
                // 丢弃处理器返回值并保留调用方副作用。
                let _ = { #handler };
            })
        };
    }

    // 消费 Navigation 专有属性并应用统一尺寸、样式、身份与其他公共事件。
    apply_common_attributes(
        // 传入已经配置数据、状态、元数据与事件的组合 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有状态和选择事件被二次映射。
        &[
            "title",
            "items",
            "activeKey",
            "openKeys",
            "collapsed",
            "version",
            "@select",
        ],
    )
}

// 解析 Navigation 的必需表达式属性。
fn required_expression(
    element: &Element,
    name: &str,
    expected: &str,
) -> Result<TokenStream, Diagnostic> {
    // 查找具名必需属性。
    let attribute = required_attribute(element, name)?;
    // 状态句柄与数据集合都必须保留表达式形状。
    let AttributeValue::Expression(expression) = &attribute.value else {
        // 返回具体期望类型诊断。
        return Err(Diagnostic::new(
            // 指向非法属性。
            attribute.span,
            // 说明公开运行时类型要求。
            format!("Navigation {name} 必须是 {expected}"),
            // 给出规范表达式绑定写法。
            format!("使用 {name}={{...}}"),
        ));
    };
    // 生成受限 Rust 表达式并保留调用 crate 的完整类型检查。
    generate_expression(&expression.expression, None)
}

// 生成可选字符串属性的 Option<String> 令牌。
fn optional_string(element: &Element, name: &str) -> Result<TokenStream, Diagnostic> {
    // 缺失版本保持 None，避免隐式本地化回退。
    let Some(attribute) = find_attribute(element, name) else {
        // 生成明确空值。
        return Ok(quote! { ::core::option::Option::None });
    };
    // 复用统一字符串字面量或表达式转换。
    let value = string_value(attribute)?;
    // 把调用方精确文本物化为拥有型版本元数据。
    Ok(quote! { ::core::option::Option::Some((#value).to_string()) })
}

// 查找元素上的具名属性。

// 查找 Navigation 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "Navigation",
        "使用 <Navigation title=\"应用\" items={items} activeKey={active} openKeys={open} collapsed={collapsed} />",
    )
}

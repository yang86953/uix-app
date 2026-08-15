// 引入卫生事件变量所需的标识符、跨度与令牌流。
use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Calendar 属性、表达式、事件与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, generate_event_handler_expression,
    generate_expression,
};

// 生成使用运行时默认交互契约的 Calendar 叶节点。
pub(crate) fn generate_calendar(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Calendar 自身管理日期格、标题与交互，不接受 UIX 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Calendar 元素。
            element.span,
            // 说明日历不接受子节点。
            "<Calendar> 不接受子节点",
            // 给出当前文档化的自闭合写法。
            "使用 <Calendar />",
        ));
    }

    // 默认日期与受控值代表互斥的状态所有权模式。
    if let (Some(default_date), Some(_value)) = (
        // 查找非受控初值属性。
        find_attribute(element, "defaultDate"),
        // 查找受控状态属性。
        find_attribute(element, "value"),
    ) {
        // 返回明确的所有权冲突诊断。
        return Err(Diagnostic::new(
            // 指向首个冲突属性。
            default_date.span,
            // 说明两个入口不能同时声明。
            "Calendar defaultDate 与 value 不能同时使用",
            // 给出两种合法模式。
            "非受控模式使用 defaultDate={date}；受控模式使用 value={selected_date}",
        ));
    }

    // 从公开 Calendar 默认构造器开始配置运行时所有权。
    let mut widget = quote! { ::uix::prelude::Calendar::new() };
    // 可选默认日期只建立非受控初值。
    if let Some(attribute) = find_attribute(element, "defaultDate") {
        // 默认日期必须保持调用侧 Date 类型检查。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回字面量形状诊断。
            return Err(Diagnostic::new(
                // 指向非法默认日期属性。
                attribute.span,
                // 说明公开构造器需要 Date 表达式。
                "Calendar defaultDate 必须是 Date 表达式",
                // 给出规范引用写法。
                "使用 defaultDate={initial_date}",
            ));
        };
        // 生成受限日期表达式。
        let date = generate_expression(&expression.expression, None)?;
        // 把调用方日期快照交给非受控构造器。
        widget = quote! { (#widget).default_date((#date).clone()) };
    }
    // 可选 value 建立 State<Date> 双向受控契约。
    if let Some(attribute) = find_attribute(element, "value") {
        // 字面量不能提供响应式状态所有权。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回绑定形状诊断。
            return Err(Diagnostic::new(
                // 指向非法 value 属性。
                attribute.span,
                // 说明公开运行时绑定类型。
                "Calendar value 必须绑定 State<Date> 表达式",
                // 给出规范状态引用写法。
                "使用 value={selected_date}",
            ));
        };
        // 生成受限状态表达式。
        let state = generate_expression(&expression.expression, None)?;
        // 借用调用方状态句柄建立受控 Calendar。
        widget = quote! { (#widget).value(&(#state)) };
    }

    // 先物化公开叶节点，Change 处理器与公共样式由 View 契约拥有。
    let mut view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 可选 Change 观察运行时已提交的规范日期文本。
    if let Some(attribute) = find_attribute(element, "@change") {
        // 事件属性必须由解析器提供受限表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回内部形状保护诊断。
            return Err(Diagnostic::new(
                // 指向完整事件属性。
                attribute.span,
                // 说明事件处理器形状。
                "Calendar @change 必须是受限处理器表达式",
                // 给出带日期载荷的规范写法。
                "使用 @change=\"on_calendar_change($event)\"",
            ));
        };
        // 创建卫生的日期文本变量。
        let value = Ident::new("__uix_calendar_change", Span::mixed_site());
        // 生成裸处理器或显式载荷调用。
        let handler = generate_event_handler_expression(&expression.expression, &value, "@change")?;
        // 使用公开 View Change 注册入口保存处理器。
        view = quote! {
            // 注册只接收运行时规范日期文本借用的闭包。
            (#view).on_change_fn(move |#value| {
                // 丢弃处理器返回值并保留调用方副作用。
                let _ = { #handler };
            })
        };
    }

    // 消费 Calendar 专有属性后应用公共 View 属性。
    apply_common_attributes(
        // 传入已经配置日期所有权与事件的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性与 Change 事件被二次映射。
        &["defaultDate", "value", "@change"],
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

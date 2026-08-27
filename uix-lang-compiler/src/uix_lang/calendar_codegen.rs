// 引入卫生事件变量所需的标识符、跨度与令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Calendar 属性、表达式、事件与诊断契约。
use super::{
    AttributeValue, Diagnostic, Element, boolean_value, generate_event_handler_expression,
    generate_expression, numeric_value,
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
    // 可选默认显示日期只消费年月，并在选择初值后覆盖初始月视图。
    if let Some(attribute) = find_attribute(element, "defaultDisplayed") {
        // 默认显示月份必须由 Date 表达式提供类型化年月。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回字面量形状诊断。
            return Err(Diagnostic::new(
                // 指向非法默认显示属性。
                attribute.span,
                // 说明公开构造器需要 Date 表达式。
                "Calendar defaultDisplayed 必须是 Date 表达式",
                // 给出规范日期引用写法。
                "使用 defaultDisplayed={displayed_date}",
            ));
        };
        // 生成受限日期表达式。
        let date = generate_expression(&expression.expression, None)?;
        // 创建卫生的单次求值日期变量。
        let displayed = Ident::new("__uix_calendar_displayed", Span::mixed_site());
        // 只求值一次并把年月交给公开默认显示构造器。
        widget = quote! {{
            // 固定调用方表达式的 Date 类型并取得拥有型快照。
            let #displayed: ::uix::prelude::Date = (#date).clone();
            // 选择日期和显示月份保持两个独立运行时事实。
            (#widget).default_displayed(#displayed.year, #displayed.month)
        }};
    }
    // 可选日期格边长复用统一数值与像素字面量契约。
    if let Some(attribute) = find_attribute(element, "cellSize") {
        // 生成 f32 数值并保留运行时最小值归一化。
        let cell_size = numeric_value(attribute)?;
        // 调用公开日期格尺寸构造器。
        widget = quote! { (#widget).cell_size(#cell_size) };
    }
    // 可选整年跳转接受布尔简写、字面量或受限表达式。
    if let Some(attribute) = find_attribute(element, "yearJump") {
        // 复用统一布尔诊断与表达式类型检查。
        let year_jump = boolean_value(attribute)?;
        // 调用公开导航策略构造器。
        widget = quote! { (#widget).year_jump(#year_jump) };
    }
    // 可选事件标记集合映射到公开拥有型数据入口。
    if let Some(attribute) = find_attribute(element, "events") {
        // 字面量不能表达类型化 CalendarEvent 集合。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回集合形状诊断。
            return Err(Diagnostic::new(
                // 指向非法事件集合属性。
                attribute.span,
                // 说明公开运行时数据类型。
                "Calendar events 必须是可迭代 CalendarEvent 表达式",
                // 给出规范集合引用写法。
                "使用 events={calendar_events}",
            ));
        };
        // 生成受限事件集合表达式。
        let events = generate_expression(&expression.expression, None)?;
        // 把数组或 Vec 统一收集为公开拥有型事件集合。
        let events = quote! {
            ::std::iter::IntoIterator::into_iter((#events).clone())
                .collect::<::std::vec::Vec<::uix::prelude::CalendarEvent>>()
        };
        // 让 Calendar 运行时取得本轮事件数据所有权。
        widget = quote! { (#widget).events(#events) };
    }
    // 可选禁用日期策略映射到公开窄判定函数入口。
    if let Some(attribute) = find_attribute(element, "disabledDate") {
        // 字面量不能提供 Fn(Date) -> bool 策略。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回函数形状诊断。
            return Err(Diagnostic::new(
                // 指向非法禁用日期属性。
                attribute.span,
                // 说明公开判定函数类型。
                "Calendar disabledDate 必须是 Fn(Date) -> bool 表达式",
                // 给出规范函数引用写法。
                "使用 disabledDate={is_disabled_date}",
            ));
        };
        // 生成受限函数表达式并保留 Rust 类型检查。
        let predicate = generate_expression(&expression.expression, None)?;
        // 把窄日期策略交给 Calendar 运行时持有。
        widget = quote! { (#widget).disabled_date(#predicate) };
    }
    // 可选日期格工厂映射到运行时已经拥有的真实 View 子树入口。
    if let Some(attribute) = find_attribute(element, "dateCell") {
        // 字面量不能提供带日期上下文的类型化 View 工厂。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回工厂形状诊断。
            return Err(Diagnostic::new(
                // 指向非法日期格工厂属性。
                attribute.span,
                // 说明公开运行时函数契约。
                "Calendar dateCell 必须是 Fn(Date, CalendarCellInfo) -> View 表达式",
                // 给出规范函数引用写法。
                "使用 dateCell={calendar_date_cell}",
            ));
        };
        // 生成受限工厂表达式并保留 Rust 对输入与返回类型的检查。
        let factory = generate_expression(&expression.expression, None)?;
        // 让 Calendar 运行时继续物化、布局和协调每个日期格子树。
        widget = quote! { (#widget).date_cell(#factory) };
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
        &[
            // 消费非受控选中初值。
            "defaultDate",
            // 消费受控选中状态。
            "value",
            // 消费独立显示月份初值。
            "defaultDisplayed",
            // 消费日期格首选尺寸。
            "cellSize",
            // 消费标题导航跨度策略。
            "yearJump",
            // 消费事件标记数据。
            "events",
            // 消费禁用日期策略。
            "disabledDate",
            // 消费类型化日期格 View 工厂。
            "dateCell",
            // 消费统一日期变化事件。
            "@change",
        ],
    )
}

// 查找元素上的具名属性。

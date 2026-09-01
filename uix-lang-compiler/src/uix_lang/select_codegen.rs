// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Select 属性、表达式、值转换与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, boolean_value, generate_event_handler_expression,
    generate_expression, string_value,
};

// 生成绑定结构化选项和单选或多选状态的下拉选择器节点。
pub(crate) fn generate_select(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Select 是叶组件，子树不能被静默忽略。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Select 元素。
            element.span,
            // 说明下拉选择器不接受子节点。
            "<Select> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <Select value={city} options={city_options} />",
        ));
    }

    // 查找文档要求的结构化 options 数据引用。
    let options_attribute = required_attribute(element, "options")?;
    // options 必须保留调用侧 SelectOption 集合的 Rust 类型检查。
    let AttributeValue::Expression(options_expression) = &options_attribute.value else {
        // 返回选项表达式形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 options 属性。
            options_attribute.span,
            // 说明公开运行时数据要求。
            "Select options 必须是可迭代 SelectOption 表达式",
            // 给出规范数据引用写法。
            "使用 options={city_options}",
        ));
    };
    // 生成受限结构化选项数据表达式。
    let options = generate_expression(&options_expression.expression, None)?;
    // 查找文档要求的 value 双向绑定。
    let value_attribute = required_attribute(element, "value")?;
    // value 字面量不能提供双向状态所有权。
    let AttributeValue::Expression(value_expression) = &value_attribute.value else {
        // 返回绑定形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 value 属性。
            value_attribute.span,
            // 说明公开运行时绑定类型。
            "Select value 必须绑定 State<String> 或 State<HashSet<String>> 表达式",
            // 给出规范绑定写法。
            "使用 value={city}",
        ));
    };
    // 生成受限状态表达式。
    let state = generate_expression(&value_expression.expression, None)?;
    // 缺省模式遵循文档的单选默认值。
    let multiple = static_multiple_value(element)?;

    // 从公开 Select 构造器和结构化选项入口开始配置。
    let mut widget = quote! {
        ::uix::prelude::Select::new().select_options((#options).clone())
    };
    // 可搜索状态接受静态或动态布尔值。
    if let Some(attribute) = find_attribute(element, "searchable") {
        // 生成统一布尔属性令牌。
        let searchable = boolean_value(attribute)?;
        // 应用公开搜索开关构建器。
        widget = quote! { (#widget).searchable_enabled(#searchable) };
    }
    // 占位文本接受字符串字面量或受限表达式。
    if let Some(attribute) = find_attribute(element, "placeholder") {
        // 生成统一字符串属性令牌。
        let placeholder = string_value(attribute)?;
        // 应用公开占位文本构建器。
        widget = quote! { (#widget).placeholder(#placeholder) };
    }
    // 最后绑定状态并由 const 泛型核对单选或多选值类型。
    widget = quote! { (#widget).value_mode::<#multiple, _>(&(#state)) };

    // 物化为公开叶 View；Change 处理器由 View 契约拥有。
    let mut view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 可选值提交事件读取统一选中值文本载荷。
    if let Some(attribute) = find_attribute(element, "@change") {
        // 事件解析器应始终提供受限表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回内部形状保护诊断。
            return Err(Diagnostic::new(
                // 指向完整事件属性。
                attribute.span,
                // 说明事件处理器形状。
                "Select @change 必须是受限处理器表达式",
                // 给出带载荷的规范写法。
                "使用 @change=\"on_change($event)\"",
            ));
        };
        // 创建卫生的选中值载荷变量。
        let value = proc_macro2::Ident::new("__uix_change_value", proc_macro2::Span::mixed_site());
        // 生成裸处理器或显式载荷调用。
        let handler =
            generate_event_handler_expression(&expression.expression, &value, "@change")?;
        // 使用公开 View Change 注册入口保存处理器。
        view = quote! {
            // 注册只接收当前选中值文本借用的提交闭包。
            (#view).on_change_fn(move |#value| {
                // 丢弃处理器返回值并保留副作用。
                let _ = { #handler };
            })
        };
    }
    // 消费 Select 专有属性并返回公共 View 表达式。
    apply_common_attributes(
        // 传入已经配置的下拉选择器 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["options", "value", "multiple", "searchable", "placeholder", "@change"],
    )
}

// 解析影响状态类型的静态 multiple 属性。
fn static_multiple_value(element: &Element) -> Result<bool, Diagnostic> {
    // 未声明时使用文档登记的单选默认值。
    let Some(attribute) = find_attribute(element, "multiple") else {
        // 返回单选模式。
        return Ok(false);
    };
    // const 泛型要求 multiple 在宏展开时可确定。
    let AttributeValue::Literal(value) = &attribute.value else {
        // 返回动态模式诊断。
        return Err(Diagnostic::new(
            // 指向非法 multiple 属性。
            attribute.span,
            // 说明该属性同时选择状态类型。
            "Select multiple 必须是静态布尔值",
            // 给出单选和多选规范写法。
            "使用 multiple、multiple=\"true\" 或 multiple=\"false\"",
        ));
    };
    // 只接受统一布尔字面量集合。
    match value.as_str() {
        // 布尔简写与 true 都进入多选模式。
        "true" => Ok(true),
        // 显式 false 保持单选模式。
        "false" => Ok(false),
        // 其他字面量返回类型选择诊断。
        _ => Err(Diagnostic::new(
            // 指向非法 multiple 属性。
            attribute.span,
            // 说明非法布尔字面量。
            "Select multiple 需要 true 或 false",
            // 给出规范静态值。
            "使用 multiple、multiple=\"true\" 或 multiple=\"false\"",
        )),
    }
}

// 查找元素上的具名属性。

// 查找 Select 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "Select",
        "使用 <Select value={city} options={city_options} />",
    )
}

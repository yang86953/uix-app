// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Steps 属性、表达式与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, boolean_value, generate_expression};

// 生成绑定 Step 集合、State<usize> current 与方向的步骤条叶节点。
pub(crate) fn generate_steps(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Steps 自身绘制完整步骤条，不接受 UIX 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Steps 元素。
            element.span,
            // 说明步骤条不接受子节点。
            "<Steps> 不接受子节点",
            // 给出规范数据绑定写法。
            "使用 <Steps current={step} items={step_items} />",
        ));
    }

    // 查找构造器必需的 items 数据来源。
    let items_attribute = required_attribute(element, "items")?;
    // items 必须保持调用侧可迭代 Step 集合的 Rust 类型检查。
    let AttributeValue::Expression(items_expression) = &items_attribute.value else {
        // 返回步骤集合表达式诊断。
        return Err(Diagnostic::new(
            // 指向非法 items 属性。
            items_attribute.span,
            // 说明公开运行时数据要求。
            "Steps items 必须是可迭代 Step 表达式",
            // 给出规范数据引用写法。
            "使用 items={step_items}",
        ));
    };
    // 生成受限步骤数据表达式。
    let items = generate_expression(&items_expression.expression, None)?;
    // 把数组或 Vec 统一收集为运行时构造器要求的 Vec<Step>。
    let steps = quote! {
        ::std::iter::IntoIterator::into_iter((#items).clone())
            .collect::<::std::vec::Vec<::uix::prelude::Step>>()
    };

    // 查找文档要求的 current 双向绑定。
    let current_attribute = required_attribute(element, "current")?;
    // current 字面量不能提供双向状态所有权。
    let AttributeValue::Expression(current_expression) = &current_attribute.value else {
        // 返回绑定形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 current 属性。
            current_attribute.span,
            // 说明公开运行时绑定类型。
            "Steps current 必须绑定 State<usize> 表达式",
            // 给出规范绑定写法。
            "使用 current={step}",
        ));
    };
    // 生成受限状态表达式。
    let current = generate_expression(&current_expression.expression, None)?;
    // 生成文档默认或显式方向调用。
    let direction = generate_direction(element)?;

    // 先构造步骤集合，再绑定唯一 current 状态，最后应用方向。
    let mut widget = quote! {
        ::uix::prelude::Steps::new(#steps)
            .current_state(&(#current))
            #direction
    };
    // 可选点击能力只声明交互策略，current 状态仍由调用方拥有。
    if let Some(attribute) = find_attribute(element, "clickable") {
        // 复用统一布尔值诊断并保留动态 bool 类型检查。
        let clickable = boolean_value(attribute)?;
        // 把最终点击策略传给公开运行时构建器。
        widget = quote! { (#widget).clickable(#clickable) };
    }
    // 可选圆点样式只改变运行时绘制配置。
    if let Some(attribute) = find_attribute(element, "dot") {
        // 复用统一布尔值诊断并保留动态 bool 类型检查。
        let dot = boolean_value(attribute)?;
        // 把最终绘制策略传给公开运行时构建器。
        widget = quote! { (#widget).dot(#dot) };
    }
    // Steps 物化为公开叶 View。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费 Steps 专有属性并应用公共 View 属性。
    apply_common_attributes(
        // 传入已经配置的步骤条 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["items", "current", "direction", "clickable", "dot"],
    )
}

// 生成 horizontal 或 vertical 的公开构建器调用。
fn generate_direction(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 查找可选方向属性。
    let Some(attribute) = find_attribute(element, "direction") else {
        // 缺省值显式固定为文档声明的 horizontal。
        return Ok(quote! { .horizontal() });
    };
    // 方向是有限关键字，不接受动态表达式。
    let AttributeValue::Literal(value) = &attribute.value else {
        // 返回关键字形状诊断。
        return Err(direction_diagnostic(attribute));
    };
    // 将两个文档关键字映射到公开构建器。
    match value.as_str() {
        // 水平方向沿用运行时水平构建器。
        "horizontal" => Ok(quote! { .horizontal() }),
        // 垂直方向使用运行时垂直构建器。
        "vertical" => Ok(quote! { .vertical() }),
        // 其他字面量拒绝静默降级。
        _ => Err(direction_diagnostic(attribute)),
    }
}

// 构造统一方向诊断。
fn direction_diagnostic(attribute: &Attribute) -> Diagnostic {
    // 返回指向具体 direction 属性的错误。
    Diagnostic::new(
        // 精确标记非法属性。
        attribute.span,
        // 说明允许的有限关键字集合。
        "Steps direction 只接受 horizontal 或 vertical",
        // 给出可直接采用的写法。
        "使用 direction=\"horizontal\" 或 direction=\"vertical\"",
    )
}

// 查找元素上的具名属性。

// 查找 Steps 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "Steps",
        "使用 <Steps current={step} items={step_items} />",
    )
}

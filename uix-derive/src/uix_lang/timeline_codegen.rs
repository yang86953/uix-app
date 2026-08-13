// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Timeline 属性、表达式与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, boolean_value, generate_expression};

// 生成绑定类型化时间轴项集合的 Timeline 叶节点。
pub(crate) fn generate_timeline(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Timeline 自身绘制全部事件项，不接收 View 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Timeline 元素。
            element.span,
            // 说明时间轴不接受子节点。
            "<Timeline> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <Timeline items={timeline_items} />",
        ));
        // 结束叶节点形状检查。
    }

    // items 是时间轴内容来源，必须显式提供。
    let items_attribute = required_attribute(element, "items")?;
    // 字符串字面量不能表达类型化 TimelineItem 集合。
    let AttributeValue::Expression(items_expression) = &items_attribute.value else {
        // 返回数据形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 items 属性。
            items_attribute.span,
            // 说明公开运行时要求可迭代类型化集合。
            "Timeline items 必须是可迭代 TimelineItem 表达式",
            // 给出规范数据引用写法。
            "使用 items={timeline_items}",
        ));
        // 结束数据表达式形状匹配。
    };
    // 生成受限 Rust 数据表达式。
    let items_expression = generate_expression(&items_expression.expression, None)?;
    // 把数组或 Vec 统一收集为公开运行时要求的 Vec 类型。
    let items = quote! {
        ::std::iter::IntoIterator::into_iter((#items_expression).clone())
            .collect::<::std::vec::Vec<::uix::prelude::TimelineItem>>()
    };
    // 未声明 pending 时显式采用文档默认值。
    let pending = optional_boolean(element, "pending")?;
    // 未声明 reverse 时显式采用文档默认值。
    let reverse = optional_boolean(element, "reverse")?;

    // 按运行时公开构建顺序配置数据与两个布尔能力。
    let widget = quote! {
        ::uix::prelude::Timeline::new()
            .items(#items)
            .pending(#pending)
            .reverse(#reverse)
    };
    // Timeline 是公开叶 View。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费专有属性并应用公共尺寸、样式、事件与自动化属性。
    apply_common_attributes(
        // 传入已经配置的时间轴 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["items", "pending", "reverse"],
    )
    // 结束 Timeline 生成函数。
}

// 生成可选布尔属性，并显式保留 false 默认值。
fn optional_boolean(element: &Element, name: &str) -> Result<TokenStream, Diagnostic> {
    // 显式属性复用统一布尔诊断与动态表达式生成。
    find_attribute(element, name)
        // 把可选属性转换为可选生成结果。
        .map(boolean_value)
        // 把 Option<Result> 转换为 Result<Option>。
        .transpose()
        // 缺省时生成类型明确的布尔字面量。
        .map(|value| value.unwrap_or_else(|| quote! { false }))
    // 结束可选布尔属性生成函数。
}

// 查找元素上的具名属性。
fn find_attribute<'a>(element: &'a Element, name: &str) -> Option<&'a Attribute> {
    // 解析器已保证同名属性唯一。
    element
        // 借用有序属性列表。
        .attributes
        // 遍历每个属性。
        .iter()
        // 返回首个名称匹配项。
        .find(|attribute| attribute.name == name)
    // 结束属性查找函数。
}

// 查找 Timeline 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 缺失属性时构造确定诊断。
    find_attribute(element, name).ok_or_else(|| {
        // 返回完整必需属性错误。
        Diagnostic::new(
            // 指向完整 Timeline 元素。
            element.span,
            // 点名缺失属性。
            format!("<Timeline> 缺少必需的 {name} 属性"),
            // 给出最小合法写法。
            "使用 <Timeline items={timeline_items} />",
        )
        // 结束缺失属性诊断闭包。
    })
    // 结束必需属性查找函数。
}

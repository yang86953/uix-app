// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Descriptions 属性、表达式与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, generate_expression};

// 生成绑定类型化描述项集合的 Descriptions 叶节点。
pub(crate) fn generate_descriptions(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Descriptions 自身绘制全部键值项，不接收 View 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Descriptions 元素。
            element.span,
            // 说明描述列表不接受子节点。
            "<Descriptions> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <Descriptions data={description_items} />",
        ));
        // 结束叶节点形状检查。
    }

    // data 是运行时内容来源，必须显式提供。
    let data_attribute = required_attribute(element, "data")?;
    // 字符串字面量不能表达类型化 DescriptionsItem 集合。
    let AttributeValue::Expression(data_expression) = &data_attribute.value else {
        // 返回数据形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 data 属性。
            data_attribute.span,
            // 说明公开运行时要求可迭代类型化集合。
            "Descriptions data 必须是可迭代 DescriptionsItem 表达式",
            // 给出规范数据引用写法。
            "使用 data={description_items}",
        ));
        // 结束数据表达式形状匹配。
    };
    // 生成受限 Rust 数据表达式。
    let data = generate_expression(&data_expression.expression, None)?;
    // 把数组或 Vec 统一收集为公开运行时要求的 Vec 类型。
    let items = quote! {
        ::std::iter::IntoIterator::into_iter((#data).clone())
            .collect::<::std::vec::Vec<::uix::prelude::DescriptionsItem>>()
    };
    // 未声明 columns 时显式采用文档默认的一列。
    let columns = match find_attribute(element, "columns") {
        // 显式列数进入正整数或 usize 表达式生成。
        Some(attribute) => columns_value(attribute)?,
        // 缺省值不得泄漏运行时组件自身的三列默认值。
        None => quote! { 1usize },
    };

    // 先配置类型化数据，再应用确定列数。
    let widget = quote! {
        ::uix::prelude::Descriptions::new()
            .items(#items)
            .column(#columns)
    };
    // 经公开 View 契约进入 Descriptions 自己的同目录 UIX 声明根。
    let view = quote! { ::uix::prelude::View::build(#widget) };
    // 消费专有属性并应用公共尺寸、样式、事件与自动化属性。
    apply_common_attributes(view, &element.attributes, &["data", "columns"])
    // 结束 Descriptions 生成函数。
}

// 生成正整数字面量或动态 usize 列数。
fn columns_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 动态表达式保留给 Rust 核对 usize 类型。
    if let AttributeValue::Expression(expression) = &attribute.value {
        // 返回受限动态列数表达式。
        return generate_expression(&expression.expression, None);
        // 结束动态表达式分支。
    }
    // 静态列数必须能解析为非零 usize。
    let AttributeValue::Literal(value) = &attribute.value else {
        // 当前属性值枚举只有字面量或表达式。
        unreachable!("Descriptions columns 属性值形状已经穷尽");
        // 结束静态字面量匹配。
    };
    // 解析并拒绝零、负数、小数及非数字文本。
    let Some(columns) = value.parse::<usize>().ok().filter(|columns| *columns > 0) else {
        // 返回带来源位置的正整数诊断。
        return Err(Diagnostic::new(
            // 指向非法 columns 属性。
            attribute.span,
            // 说明列数静态约束。
            "Descriptions columns 必须是大于零的整数",
            // 给出静态与动态两类合法写法。
            "使用 columns=\"2\" 或 columns={column_count}",
        ));
        // 结束正整数解析。
    };
    // 返回类型明确的 usize 字面量。
    Ok(quote! { #columns })
    // 结束列数生成函数。
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

// 查找 Descriptions 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 缺失属性时构造确定诊断。
    find_attribute(element, name).ok_or_else(|| {
        // 返回完整必需属性错误。
        Diagnostic::new(
            // 指向完整 Descriptions 元素。
            element.span,
            // 点名缺失属性。
            format!("<Descriptions> 缺少必需的 {name} 属性"),
            // 给出最小合法写法。
            "使用 <Descriptions data={description_items} />",
        )
        // 结束缺失属性诊断闭包。
    })
    // 结束必需属性查找函数。
}

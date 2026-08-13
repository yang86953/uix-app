// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 List 属性、表达式、字符串与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, generate_expression, string_value};

// 生成只声明文本数据与字符串槽位的 List 叶节点。
pub(crate) fn generate_list(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 现有 List 运行时独占字符串行绘制，不接收任意 View 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 List 元素。
            element.span,
            // 说明文本列表不接受子节点。
            "<List> 不接受子节点",
            // 给出最小合法数据绑定写法。
            "使用 <List data={list_items} />",
        ));
    }

    // data 是列表文本来源，必须显式提供。
    let data_attribute = required_attribute(element, "data")?;
    // 字符串字面量不能表达拥有型可迭代列表。
    let AttributeValue::Expression(data_expression) = &data_attribute.value else {
        // 返回集合表达式形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 data 属性。
            data_attribute.span,
            // 说明公开运行时要求可迭代字符串集合。
            "List data 必须是可迭代字符串表达式",
            // 给出规范数据引用写法。
            "使用 data={list_items}",
        ));
    };
    // 生成受限 Rust 列表表达式。
    let data = generate_expression(&data_expression.expression, None)?;
    // 把数组或 Vec 的文本项统一收集为运行时拥有的 Vec<String>。
    let items = quote! {
        ::std::iter::IntoIterator::into_iter((#data).clone())
            .map(::std::convert::Into::into)
            .collect::<::std::vec::Vec<::std::string::String>>()
    };

    // 从公开构造器开始并把文本数据交给运行时持有。
    let mut widget = quote! { ::uix::prelude::List::new().items(#items) };
    // 可选页首只声明现有字符串槽位。
    if let Some(attribute) = find_attribute(element, "header") {
        // 生成字符串字面量或受限字符串表达式。
        let header = string_value(attribute)?;
        // 构造期间临时借用页首文本，运行时负责复制。
        widget = quote! { (#widget).header(&*(#header)) };
    }
    // 可选页尾只声明现有字符串槽位。
    if let Some(attribute) = find_attribute(element, "footer") {
        // 生成字符串字面量或受限字符串表达式。
        let footer = string_value(attribute)?;
        // 构造期间临时借用页尾文本，运行时负责复制。
        widget = quote! { (#widget).footer(&*(#footer)) };
    }
    // 可选加载更多文字只投影现有运行时文本入口。
    if let Some(attribute) = find_attribute(element, "loadMore") {
        // 生成字符串字面量或受限字符串表达式。
        let load_more = string_value(attribute)?;
        // 构造期间临时借用加载更多文字，运行时负责复制。
        widget = quote! { (#widget).load_more(&*(#load_more)) };
    }

    // 使用公开 View 契约保留空数据时的 Empty 替代生命周期。
    let view = quote! { ::uix::prelude::View::build(#widget) };
    // 消费 List 专有属性后应用统一尺寸、样式与自动化属性。
    apply_common_attributes(
        // 传入已经按运行时规则物化的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["data", "header", "footer", "loadMore"],
    )
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
}

// 查找 List 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 缺失属性时构造确定诊断。
    find_attribute(element, name).ok_or_else(|| {
        // 返回完整必需属性错误。
        Diagnostic::new(
            // 指向完整 List 元素。
            element.span,
            // 点名缺失属性。
            format!("<List> 缺少必需的 {name} 属性"),
            // 给出最小合法写法。
            "使用 <List data={list_items} />",
        )
    })
}

// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入属性、布尔值、表达式与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, boolean_value, generate_expression};

// 生成类型化 TableRow / TableColumn 集合的静态数据表格。
pub(crate) fn generate_table(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Table 自身绘制行列，不接受 UIX 内容子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Table 元素。
            element.span,
            // 说明表格不接受子节点。
            "<Table> 不接受子节点",
            // 给出规范类型化数据写法。
            "使用 <Table data={rows} columns={columns} />",
        ));
    }

    // 查找必需的行数据表达式。
    let data_attribute = required_attribute(element, "data")?;
    // 行数据必须由 Rust 类型系统核对 TableRow 元素。
    let AttributeValue::Expression(data_expression) = &data_attribute.value else {
        // 返回行集合表达式诊断。
        return Err(Diagnostic::new(
            // 指向非法 data 属性。
            data_attribute.span,
            // 说明公开运行时数据类型。
            "Table data 必须是可迭代 TableRow 表达式",
            // 给出规范数据引用写法。
            "使用 data={rows}",
        ));
    };
    // 生成受限行集合表达式。
    let data = generate_expression(&data_expression.expression, None)?;
    // 把数组或 Vec 统一收集为运行时拥有的行集合。
    let rows = quote! {
        ::std::iter::IntoIterator::into_iter((#data).clone())
            .collect::<::std::vec::Vec<::uix::prelude::TableRow>>()
    };

    // 查找必需的列定义表达式。
    let columns_attribute = required_attribute(element, "columns")?;
    // 列定义必须由 Rust 类型系统核对 TableColumn 元素。
    let AttributeValue::Expression(columns_expression) = &columns_attribute.value else {
        // 返回列集合表达式诊断。
        return Err(Diagnostic::new(
            // 指向非法 columns 属性。
            columns_attribute.span,
            // 说明公开运行时列类型。
            "Table columns 必须是可迭代 TableColumn 表达式",
            // 给出规范列引用写法。
            "使用 columns={columns}",
        ));
    };
    // 生成受限列集合表达式。
    let columns = generate_expression(&columns_expression.expression, None)?;
    // 把数组或 Vec 统一收集为运行时拥有的列集合。
    let columns = quote! {
        ::std::iter::IntoIterator::into_iter((#columns).clone())
            .collect::<::std::vec::Vec<::uix::prelude::TableColumn>>()
    };

    // 由公开 Table 运行时取得列与行的唯一所有权。
    let mut widget = quote! {
        ::uix::prelude::Table::new()
            .columns(#columns)
            .rows(#rows)
    };
    // 可选 virtual 映射到既有虚拟滚动开关。
    if let Some(attribute) = find_attribute(element, "virtual") {
        // 复用统一布尔字面量或表达式诊断。
        let enabled = boolean_value(attribute)?;
        // 调用公开 virtual_scroll 构建器。
        widget = quote! { (#widget).virtual_scroll(#enabled) };
    }
    // 可选 loading 映射到既有加载态所有权。
    if let Some(attribute) = find_attribute(element, "loading") {
        // 复用统一布尔字面量或表达式诊断。
        let enabled = boolean_value(attribute)?;
        // 调用公开 loading 构建器。
        widget = quote! { (#widget).loading(#enabled) };
    }

    // Table 物化为公开叶 View。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费 Table 专有属性并应用公共 View 属性。
    apply_common_attributes(
        // 传入完整类型化表格 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["data", "columns", "virtual", "loading"],
    )
}

// 查找元素上的具名属性。

// 查找 Table 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "Table",
        "使用 <Table data={rows} columns={columns} />",
    )
}

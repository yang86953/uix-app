// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Breadcrumb 属性、表达式、字符串与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, generate_expression, string_value};

// 生成拥有类型化路径数据并由运行时保持选择生命周期的 Breadcrumb。
pub(crate) fn generate_breadcrumb(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Breadcrumb 自身绘制路径条目，不接受 UIX 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Breadcrumb 元素。
            element.span,
            // 说明面包屑不接受子节点。
            "<Breadcrumb> 不接受子节点",
            // 给出规范数据绑定写法。
            "使用 <Breadcrumb items={breadcrumb_items} />",
        ));
    }

    // 查找构造器必需的 items 数据来源。
    let items_attribute = required_attribute(element, "items")?;
    // items 必须保持调用侧可迭代 BreadcrumbItem 集合的 Rust 类型检查。
    let AttributeValue::Expression(items_expression) = &items_attribute.value else {
        // 返回条目集合表达式诊断。
        return Err(Diagnostic::new(
            // 指向非法 items 属性。
            items_attribute.span,
            // 说明公开运行时数据要求。
            "Breadcrumb items 必须是可迭代 BreadcrumbItem 表达式",
            // 给出规范数据引用写法。
            "使用 items={breadcrumb_items}",
        ));
    };
    // 生成受限条目数据表达式。
    let items = generate_expression(&items_expression.expression, None)?;
    // 把数组或 Vec 统一收集为运行时构造器要求的拥有型集合。
    let breadcrumb_items = quote! {
        ::std::iter::IntoIterator::into_iter((#items).clone())
            .collect::<::std::vec::Vec<::uix_app::prelude::BreadcrumbItem>>()
    };

    // 由 Breadcrumb 运行时接收条目并落实文档的末项当前页语义。
    let mut widget = quote! {
        ::uix_app::prelude::Breadcrumb::new()
            .items(#breadcrumb_items)
            .last_active()
    };
    // separator 只声明可见路径条目之间的分隔文本。
    if let Some(attribute) = find_attribute(element, "separator") {
        // 解析字符串字面量或表达式。
        let separator = string_value(attribute)?;
        // 调用公开分隔符配置入口。
        widget = quote! { (#widget).separator(#separator) };
    }
    // maxItems 只声明溢出折叠前的最大可见条目数。
    if let Some(attribute) = find_attribute(element, "maxItems") {
        // 解析 usize 字面量或表达式。
        let max_items = usize_value(attribute)?;
        // 调用公开折叠阈值入口。
        widget = quote! { (#widget).max_items(#max_items) };
    }
    // Breadcrumb 物化为公开叶 View。
    let view = quote! { ::uix_app::prelude::ViewNode::leaf(#widget) };
    // 消费 Breadcrumb 专有属性并应用公共 View 属性。
    apply_common_attributes(
        // 传入已配置路径数据的 Breadcrumb View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有 items 进入公共映射。
        &["items", "separator", "maxItems"],
    )
}

// 生成 Breadcrumb usize 字面量或表达式属性。
fn usize_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状生成折叠阈值。
    match &attribute.value {
        // 字面量在生成期验证为十进制 usize。
        AttributeValue::Literal(source) => {
            // 拒绝负数、小数与溢出值。
            let value = source.parse::<usize>().map_err(|_| {
                // 返回精确 maxItems 诊断。
                Diagnostic::new(
                    // 指向非法属性。
                    attribute.span,
                    // 说明公开运行时类型。
                    "Breadcrumb maxItems 必须是 usize 整数",
                    // 给出字面量或表达式修复建议。
                    "使用 maxItems=\"3\" 或 maxItems={maximum}",
                )
            })?;
            // 返回已验证的 usize 字面量。
            Ok(quote! { #value })
        }
        // 动态表达式由 Rust 类型系统核对 usize。
        AttributeValue::Expression(expression) => {
            // 生成受限 Rust 表达式。
            generate_expression(&expression.expression, None)
        }
        // 内联样式不能表达折叠阈值。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向非法属性。
            attribute.span,
            // 说明公开运行时类型。
            "Breadcrumb maxItems 必须是 usize 整数",
            // 给出字面量或表达式修复建议。
            "使用 maxItems=\"3\" 或 maxItems={maximum}",
        )),
    }
}

// 查找元素上的具名属性。

// 查找 Breadcrumb 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "Breadcrumb",
        "使用 <Breadcrumb items={breadcrumb_items} />",
    )
}

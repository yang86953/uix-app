// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Breadcrumb 属性、表达式与诊断契约。
use super::{Attribute, AttributeValue, Diagnostic, Element, generate_expression};

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
        ::std::iter::IntoIterator::into_iter(#items)
            .collect::<::std::vec::Vec<::uix::prelude::BreadcrumbItem>>()
    };

    // 由 Breadcrumb 运行时接收条目并落实文档的末项当前页语义。
    let widget = quote! {
        ::uix::prelude::Breadcrumb::new()
            .items(#breadcrumb_items)
            .last_active()
    };
    // Breadcrumb 物化为公开叶 View。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费 Breadcrumb 专有属性并应用公共 View 属性。
    apply_common_attributes(
        // 传入已配置路径数据的 Breadcrumb View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有 items 进入公共映射。
        &["items"],
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

// 查找 Breadcrumb 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 缺失属性时构造确定诊断。
    find_attribute(element, name).ok_or_else(|| {
        // 返回完整必需属性错误。
        Diagnostic::new(
            // 指向完整 Breadcrumb 元素。
            element.span,
            // 点名缺失属性。
            format!("<Breadcrumb> 缺少必需的 {name} 属性"),
            // 给出最小合法写法。
            "使用 <Breadcrumb items={breadcrumb_items} />",
        )
    })
}

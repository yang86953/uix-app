use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Tree 属性、表达式与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, generate_expression, optional_boolean,
};

// 生成绑定类型化节点集合与初始交互配置的 Tree 叶节点。
pub(crate) fn generate_tree(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Tree 自身绘制完整层级并管理交互，不接受 UIX 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Tree 元素。
            element.span,
            // 说明树组件不接受子节点。
            "<Tree> 不接受子节点",
            // 给出规范数据绑定写法。
            "使用 <Tree data={tree_nodes} />",
        ));
    }

    // data 是树内容来源，必须显式提供。
    let data_attribute = required_attribute(element, "data")?;
    // 字符串字面量不能表达类型化 TreeNode 集合。
    let AttributeValue::Expression(data_expression) = &data_attribute.value else {
        // 返回数据形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 data 属性。
            data_attribute.span,
            // 说明公开运行时要求可迭代类型化集合。
            "Tree data 必须是可迭代 TreeNode 表达式",
            // 给出规范数据引用写法。
            "使用 data={tree_nodes}",
        ));
    };
    // 生成受限 Rust 数据表达式。
    let data = generate_expression(&data_expression.expression, None)?;
    // 把数组或 Vec 统一收集为运行时要求的 Vec<TreeNode>。
    let nodes = quote! {
        ::std::iter::IntoIterator::into_iter((#data).clone())
            .collect::<::std::vec::Vec<::uix::prelude::TreeNode>>()
    };
    // 未声明 checkable 时显式采用文档默认值。
    let checkable = optional_boolean(element, "checkable")?;
    // 未声明 defaultExpandAll 时显式采用文档默认值。
    let default_expand_all = optional_boolean(element, "defaultExpandAll")?;

    // 按公开构建顺序配置节点和两项树级能力。
    let widget = quote! {
        ::uix::prelude::Tree::new(#nodes)
            .checkable(#checkable)
            .default_expand_all(#default_expand_all)
    };
    // Tree 是公开叶 View。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费专有属性并应用公共尺寸、样式与自动化属性。
    apply_common_attributes(
        // 传入已经配置的树 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["data", "checkable", "defaultExpandAll"],
    )
}

// 生成可选布尔属性，并显式保留 false 默认值。

// 查找元素上的具名属性。

// 查找 Tree 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(element, name, "Tree", "使用 <Tree data={tree_nodes} />")
}

// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与有序子树生成入口。
use super::codegen::{apply_common_attributes, generate_children};
// 引入 Card 属性、表达式与诊断契约。
use super::{AttributeValue, Diagnostic, Element, generate_expression, string_value};

// 生成保留完整 ViewNode 子树的 Card 卡片容器。
pub(crate) fn generate_card(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 从无内部旧式子组件的公开 Card 构造器开始配置。
    let mut widget = quote! { ::uix::prelude::Card::new() };
    // 可选标题接受字符串字面量或受限字符串表达式。
    if let Some(attribute) = find_attribute(element, "title") {
        // 生成统一字符串值令牌。
        let title = string_value(attribute)?;
        // Card 会复制借用的标题，因此调用侧表达式无需满足静态生命周期。
        widget = quote! { (#widget).title(&*(#title)) };
        // 结束标题属性分支。
    }
    // 可选操作项只接受数组表达式引用。
    if let Some(attribute) = find_attribute(element, "actions") {
        // 字符串字面量不能表达结构化操作项集合。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回带来源位置的属性形状诊断。
            return Err(Diagnostic::new(
                // 指向非法 actions 属性。
                attribute.span,
                // 说明运行时构建器要求结构化集合。
                "Card actions 必须是操作项数组表达式",
                // 给出规范数据引用写法。
                "使用 actions={card_actions}",
            ));
            // 结束表达式形状匹配。
        };
        // 生成受限 Rust 表达式并保留调用侧集合类型检查。
        let actions = generate_expression(&expression.expression, None)?;
        // 把操作项集合所有权交给公开 Card 构建器。
        widget = quote! { (#widget).actions((#actions).clone()) };
        // 结束操作项属性分支。
    }

    // 按源码顺序生成子节点、条件与循环控制流。
    let children = generate_children(&element.children)?;
    // 经公开桥接进入 Card 自己的同目录 UIX 根声明，并原样移交拥有型子树。
    let view = quote! { (#widget).build_view_with_children(#children) };
    // 消费 Card 专有属性并应用公共尺寸、样式、事件与自动化属性。
    apply_common_attributes(view, &element.attributes, &["title", "actions"])
    // 结束 Card 生成函数。
}

// 查找元素上的具名属性。

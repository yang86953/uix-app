// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入共享元素属性查找。
use super::find_attribute;
// 引入 Terminal 属性、表达式与诊断契约。
use super::{AttributeValue, Diagnostic, Element, generate_expression, string_value};

// 生成绑定类型化输出行与可选提示符的 Terminal 叶节点。
pub(crate) fn generate_terminal(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Terminal 自身绘制输出缓冲与命令行，不接受 UIX 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Terminal 元素。
            element.span,
            // 说明终端组件不接受子节点。
            "<Terminal> 不接受子节点",
            // 给出规范数据绑定写法。
            "使用 <Terminal data={terminal_lines} prompt=\"$ \" />",
        ));
    }

    // data 是输出行来源，必须显式提供。
    let data_attribute = super::required_attribute(
        element,
        "data",
        "Terminal",
        "使用 <Terminal data={terminal_lines} />",
    )?;
    // 字符串字面量不能表达类型化 TerminalLine 集合。
    let AttributeValue::Expression(data_expression) = &data_attribute.value else {
        // 返回数据形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 data 属性。
            data_attribute.span,
            // 说明公开运行时要求可迭代类型化集合。
            "Terminal data 必须是可迭代 TerminalLine 表达式",
            // 给出规范数据引用写法。
            "使用 data={terminal_lines}",
        ));
    };
    // 生成受限 Rust 数据表达式。
    let data = generate_expression(&data_expression.expression, None)?;
    // 把数组或 Vec 统一收集为运行时要求的 Vec<TerminalLine>。
    let lines = quote! {
        ::std::iter::IntoIterator::into_iter((#data).clone())
            .collect::<::std::vec::Vec<::uix::prelude::TerminalLine>>()
    };
    // 从公开默认构造器开始并绑定输出行。
    let mut widget = quote! { ::uix::prelude::Terminal::new().lines(#lines) };
    // 可选提示符接受字符串字面量或受限字符串表达式。
    if let Some(attribute) = find_attribute(element, "prompt") {
        // 生成提示符字符串令牌。
        let prompt = string_value(attribute)?;
        // 调用公开提示符构建器。
        widget = quote! { (#widget).prompt(#prompt) };
    }

    // Terminal 是公开叶 View。
    let view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 消费专有属性并应用公共尺寸、样式与自动化属性。
    apply_common_attributes(
        // 传入已经配置的终端 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["data", "prompt"],
    )
}

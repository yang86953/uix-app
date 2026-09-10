// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入按钮生成器与公共属性生成器。
use super::codegen::{apply_common_attributes, generate_button_with_group_position};
// 引入 ButtonGroup 所需的语法树、源码标记与诊断。
use super::{Diagnostic, Element, Node, mark_source_tokens};

// 生成文档化 ButtonGroup 标签对应的公开 Rust View。
pub(crate) fn generate_button_group(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 收集保持源码顺序的直接 Button 子元素。
    let mut buttons = Vec::new();
    // 逐项验证 ButtonGroup 的静态直接子项边界。
    for node in &element.children {
        // 按节点形状决定收集或拒绝。
        match node {
            // 格式化空白不形成可见子项。
            Node::Text(text) if text.value.trim().is_empty() => {}
            // 直接 Button 子元素进入连体位置生成。
            Node::Element(button) if button.name == "Button" => buttons.push(button),
            // 其他元素不能静默近似为按钮。
            Node::Element(child) => {
                // 返回带子元素跨度的结构化形状诊断。
                return Err(Diagnostic::new(
                    // 指向非法直接子元素。
                    child.span,
                    // 说明具体非法元素。
                    format!("<ButtonGroup> 不接受直接 <{}> 子元素", child.name),
                    // 给出文档化静态子项边界。
                    "ButtonGroup 只放置直接 <Button> 子元素；动态 If/For 组合请改用 Row",
                ));
            }
            // 可见文本不能作为连体按钮。
            Node::Text(text) => {
                // 返回带文本跨度的结构化诊断。
                return Err(Diagnostic::new(
                    // 指向可见文本。
                    text.span,
                    // 说明 ButtonGroup 不接收裸文本。
                    "<ButtonGroup> 不接受可见文本子节点",
                    // 给出将文字放入 Button 的修复建议。
                    "把文字放入直接 <Button> 子元素",
                ));
            }
            // 插值无法确定为 Button 组件。
            Node::Interpolation(expression) => {
                // 返回带插值跨度的结构化诊断。
                return Err(Diagnostic::new(
                    // 指向插值表达式。
                    expression.span,
                    // 说明静态组件形状要求。
                    "<ButtonGroup> 不接受插值子节点",
                    // 给出动态列表的明确替代路径。
                    "ButtonGroup 只放置直接 <Button> 子元素；动态内容请改用 Row",
                ));
            }
            // 成员块是声明载体，不构成连体按钮子项。
            Node::WidgetMember(block) => {
                // 返回带块跨度的结构化诊断。
                return Err(Diagnostic::new(
                    // 指向成员块。
                    block.span,
                    // 说明静态组件形状要求。
                    "<ButtonGroup> 不接受成员声明块子节点",
                    // 给出正确归属提醒。
                    "把 @props/@state/@computed/@actions 移入 <Widget> 模板声明区",
                ));
            }
        }
    }
    // 保存静态按钮总数以确定每项连体位置。
    let button_count = buttons.len();
    // 预分配生成后的子 View 表达式。
    let mut generated_buttons = Vec::with_capacity(button_count);
    // 按源码顺序为每个按钮选择稳定位置。
    for (index, button) in buttons.into_iter().enumerate() {
        // 根据静态数量与索引选择现有公开位置枚举。
        let position = if button_count <= 1 {
            // 单按钮使用完整圆角。
            quote! { ::uix_app::prelude::ButtonGroupPosition::Single }
        } else if index == 0 {
            // 首按钮保留左侧圆角。
            quote! { ::uix_app::prelude::ButtonGroupPosition::Left }
        } else if index + 1 == button_count {
            // 末按钮保留右侧圆角。
            quote! { ::uix_app::prelude::ButtonGroupPosition::Right }
        } else {
            // 中间按钮移除相邻侧圆角。
            quote! { ::uix_app::prelude::ButtonGroupPosition::Middle }
        };
        // 复用完整 Button 属性、事件与公共样式生成路径。
        let generated = generate_button_with_group_position(button, Some(position))?;
        // 专用父子生成路径也要把子 Button 表达式绑定到精确源码跨度。
        let generated = mark_source_tokens(generated, button.span);
        // 统一物化为公开 ViewNode 子项。
        generated_buttons.push(quote! { ::uix_app::prelude::View::build(#generated) });
    }
    // 使用零间距行容器组合连体按钮并保持空组合法。
    let base = quote! {
        // 按源码顺序构造静态按钮向量。
        ::uix_app::prelude::row(::std::vec![#(#generated_buttons),*]).gap(0.0)
    };
    // ButtonGroup 没有专有属性，统一处理公共样式与未知属性诊断。
    apply_common_attributes(
        // 传入 ButtonGroup 基础 View。
        base,
        // 传入原始属性列表。
        &element.attributes,
        // 当前没有需要跳过的专有属性。
        &[],
    )
}

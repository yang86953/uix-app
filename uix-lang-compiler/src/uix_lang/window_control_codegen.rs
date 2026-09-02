// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入共享公共属性生成器与可见节点判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 WindowControl 所需的语法树、属性值生成器与诊断。
use super::{Diagnostic, Element, boolean_value, string_value};

// 生成文档化 WindowControl 标签对应的公开 Rust View。
pub(crate) fn generate_window_control(element: &Element) -> Result<TokenStream, Diagnostic> {
    // WindowControl 是组合叶标签，不能静默丢弃可见子节点。
    if element.children.iter().any(is_renderable_node) {
        // 返回带元素跨度的结构化形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 WindowControl 元素。
            element.span,
            // 说明 WindowControl 不接收子内容。
            "<WindowControl> 不接受子节点",
            // 给出文档化的自闭合写法。
            "使用 <WindowControl />，并通过 showMinimize、showMaximize、showClose 控制动作",
        ));
    }
    // 默认显示最小化动作。
    let mut show_minimize = quote! { true };
    // 默认显示最大化/还原动作。
    let mut show_maximize = quote! { true };
    // 默认显示关闭动作。
    let mut show_close = quote! { true };
    // 可选图标前景色保持未指定，运行时回退主题文本角色。
    let mut icon_color: Option<proc_macro2::TokenStream> = None;
    // 按源码顺序解析四个专有属性。
    for attribute in &element.attributes {
        // 按属性名更新对应显示表达式。
        match attribute.name.as_str() {
            // 映射最小化显示状态。
            "showMinimize" => show_minimize = boolean_value(attribute)?,
            // 映射最大化/还原显示状态。
            "showMaximize" => show_maximize = boolean_value(attribute)?,
            // 映射关闭显示状态。
            "showClose" => show_close = boolean_value(attribute)?,
            // 映射可选图标前景色字符串或表达式。
            "iconColor" => icon_color = Some(string_value(attribute)?),
            // 公共属性由统一生成器处理。
            _ => {}
        }
    }
    // 按是否声明图标前景色选择公开组合入口。
    let base = if let Some(color) = icon_color {
        // 指定前景色时把颜色表达式转换为 ColorValue 并进入定制组合。
        quote! {
            // 窗口动作、无障碍语义保持由运行时模块拥有。
            ::uix::prelude::window_controls_with_icon_color(
                #show_minimize,
                #show_maximize,
                #show_close,
                (#color).into(),
            )
        }
    } else {
        // 未指定前景色时保持既有主题文本色组合入口。
        quote! {
            // 保持窗口动作、无障碍语义与默认外观由运行时模块拥有。
            ::uix::prelude::window_controls(#show_minimize, #show_maximize, #show_close)
        }
    };
    // 专有属性消费后继续复用统一样式与未知属性诊断路径。
    apply_common_attributes(
        // 传入 WindowControl 基础 View。
        base,
        // 传入原始属性列表。
        &element.attributes,
        // 防止专有属性被公共映射重复处理。
        &["showMinimize", "showMaximize", "showClose", "iconColor"],
    )
}

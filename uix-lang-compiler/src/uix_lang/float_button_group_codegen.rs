// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入公共属性生成器与完整 FloatButton 子项生成器。
use super::codegen::apply_common_attributes;
// 引入组标签所需的语法树、字面量与诊断契约。
use super::{Diagnostic, Element, Node, generate_float_button, literal_string};

// 生成只接收静态直接 FloatButton 子项的浮动按钮组。
pub(crate) fn generate_float_button_group(
    // 借用已经通过解析的组元素。
    element: &Element,
    // 返回完整公开 View 表达式或结构化诊断。
) -> Result<TokenStream, Diagnostic> {
    // 缺省沿用运行时文档定义的悬停展开方式。
    let mut trigger = quote! { ::uix_app::prelude::TriggerMode::Hover };
    // 查找可选 trigger 专有属性。
    if let Some(attribute) = element
        // 借用源码顺序属性。
        .attributes
        // 遍历全部属性。
        .iter()
        // 只选择 trigger。
        .find(|attribute| attribute.name == "trigger")
    {
        // trigger 必须是编译期确定的文档关键字。
        let value = literal_string(attribute, "trigger")?;
        // 映射已登记的两种展开方式。
        trigger = match value.as_str() {
            // 点击触发由组父组件拥有。
            "click" => quote! { ::uix_app::prelude::TriggerMode::Click },
            // 悬停触发由组父组件拥有。
            "hover" => quote! { ::uix_app::prelude::TriggerMode::Hover },
            // 未登记关键字不能静默回退。
            _ => {
                // 返回精确属性诊断。
                return Err(Diagnostic::new(
                    // 指向非法 trigger 属性。
                    attribute.span,
                    // 点名不支持的值。
                    format!("FloatButtonGroup trigger={value:?} 不受支持"),
                    // 给出完整合法集合。
                    "使用 click 或 hover",
                ));
            }
        };
    }

    // 保存源码顺序稳定的直接按钮 ViewNode。
    let mut buttons = Vec::new();
    // 逐项验证组的直接子节点边界。
    for node in &element.children {
        // 按语法节点形状收集或拒绝。
        match node {
            // 源码排版空白不形成按钮子项。
            Node::Text(text) if text.value.trim().is_empty() => {}
            // 直接 FloatButton 复用完整属性、公共样式与标准事件生成路径。
            Node::Element(button) if button.name == "FloatButton" => {
                // 保留该子节点生成的 handler、key 与样式。
                buttons.push(generate_float_button(button)?);
            }
            // 动态直接子树会破坏父组件布局记录与子身份的一一对应。
            Node::Element(child) if matches!(child.name.as_str(), "If" | "For") => {
                // 返回明确动态形状诊断。
                return Err(Diagnostic::new(
                    // 指向非法控制流子树。
                    child.span,
                    // 说明当前静态直接子项限制。
                    "<FloatButtonGroup> 的直接子项不能是 If 或 For",
                    // 给出保持稳定身份的修复路径。
                    "直接列出源码顺序稳定的 <FloatButton>；动态组子项尚未登记",
                ));
            }
            // 其他直接元素不是父组件可验证的按钮根。
            Node::Element(child) => {
                // 返回具体非法元素诊断。
                return Err(Diagnostic::new(
                    // 指向非法直接元素。
                    child.span,
                    // 点名非法元素。
                    format!("<FloatButtonGroup> 不接受直接 <{}> 子元素", child.name),
                    // 给出唯一合法直接子项。
                    "只放置直接 <FloatButton> 子元素",
                ));
            }
            // 可见文本不能转换为浮动操作按钮。
            Node::Text(text) => {
                // 返回文本形状诊断。
                return Err(Diagnostic::new(
                    // 指向可见文本。
                    text.span,
                    // 说明裸文本非法。
                    "<FloatButtonGroup> 不接受可见文本子节点",
                    // 给出说明文字的公开属性路径。
                    "把文字放入直接 FloatButton 的 description 属性",
                ));
            }
            // 插值不能证明生成 FloatButton 直接根。
            Node::Interpolation(expression) => {
                // 返回插值形状诊断。
                return Err(Diagnostic::new(
                    // 指向插值表达式。
                    expression.span,
                    // 说明静态根类型要求。
                    "<FloatButtonGroup> 不接受插值子节点",
                    // 给出明确静态替代路径。
                    "直接列出源码顺序稳定的 <FloatButton> 子元素",
                ));
            }
            // 成员块是声明载体，不构成浮动按钮子项。
            Node::WidgetMember(block) => {
                // 返回块形状诊断。
                return Err(Diagnostic::new(
                    // 指向成员块。
                    block.span,
                    // 说明静态根类型要求。
                    "<FloatButtonGroup> 不接受成员声明块子节点",
                    // 给出正确归属提醒。
                    "把 @props/@state/@computed/@actions 移入 <Widget> 模板声明区",
                ));
            }
        }
    }

    // 使用运行时窄入口组合父组件与完整子 ViewNode。
    let base = quote! {
        // 先物化包装器为 ViewNode，随后才应用组自身公共属性。
        ::uix_app::prelude::View::build(
            // 父组件只接管展开方式与派生几何。
            (::uix_app::prelude::FloatButtonGroup::new())
                // 显式传入文档化触发方式。
                .trigger(#trigger)
                // 子 ViewNode 原样保留标准事件与协调身份。
                .button_views(::std::vec![#(#buttons),*])
        )
    };
    // 消费 trigger，并由统一入口处理组自身公共属性与事件。
    apply_common_attributes(base, &element.attributes, &["trigger"])
}

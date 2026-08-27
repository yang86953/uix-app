// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入共享公共属性生成器与可见节点判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Divider 所需的语法树、属性值生成器与诊断。
use super::{Diagnostic, Element, boolean_value, literal_string, string_value};

// 生成文档化 Divider 标签对应的公开 Rust View。
pub(crate) fn generate_divider(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Divider 是叶子组件，不能静默丢弃可见子节点。
    if element.children.iter().any(is_renderable_node) {
        // 返回带元素跨度的结构化形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Divider 元素。
            element.span,
            // 说明 Divider 不接收子内容。
            "<Divider> 不接受子节点",
            // 给出文档化的自闭合写法。
            "使用 <Divider />，并通过 text 属性声明标签文字",
        ));
    }
    // 从现有公开 Divider Widget 默认契约开始构建。
    let mut divider = quote! { ::uix::prelude::Divider::new() };
    // 按源码顺序应用 Divider 专有属性。
    for attribute in &element.attributes {
        // 按属性名选择现有公开构建器。
        match attribute.name.as_str() {
            // 可选标签文字支持字面量与受限字符串表达式。
            "text" => {
                // 生成字符串属性表达式。
                let text = string_value(attribute)?;
                // Divider 在构建时复制文字，因此只临时借用输入值。
                divider = quote! { (#divider).with_text(&(#text)) };
            }
            // 虚线状态支持布尔字面量与受限表达式。
            "dashed" => {
                // 生成确定的布尔表达式。
                let dashed = boolean_value(attribute)?;
                // 现有构建器只提供启用方法，使用同类型分支保留 false 默认值。
                divider = quote! {{
                    // 保存当前构建链，确保只求值一次。
                    let __uix_divider = #divider;
                    // 仅在属性为真时启用虚线。
                    if #dashed {
                        // 复用现有虚线构建器。
                        __uix_divider.dashed()
                    // 属性为假时保持原组件。
                    } else {
                        // 返回未启用虚线的组件。
                        __uix_divider
                    }
                }};
            }
            // 方向由公开文档中的确定关键字选择。
            "direction" => {
                // 方向影响构建器选择，因此要求编译期字符串字面量。
                let direction = literal_string(attribute, "Divider direction")?;
                // 按文档化方向映射现有组件能力。
                divider = match direction.as_str() {
                    // horizontal 是 Divider 的默认方向。
                    "horizontal" => divider,
                    // vertical 复用现有垂直构建器。
                    "vertical" => quote! { (#divider).vertical() },
                    // 其他方向不能静默近似。
                    _ => {
                        // 返回带属性跨度的枚举诊断。
                        return Err(Diagnostic::new(
                            // 指向非法 direction 属性。
                            attribute.span,
                            // 说明具体非法值。
                            format!("Divider direction={direction:?} 不受支持"),
                            // 给出已登记方向集合。
                            "使用 direction=\"horizontal\" 或 direction=\"vertical\"",
                        ));
                    }
                };
            }
            // 公共属性由统一生成器处理。
            _ => {}
        }
    }
    // 通过公开 View 契约进入 Divider 的 UIX 声明壳。
    let base = quote! { ::uix::prelude::View::build(#divider) };
    // 专有属性消费后继续复用统一样式与事件诊断路径。
    apply_common_attributes(
        // 传入 Divider 基础 View。
        base,
        // 传入原始属性列表。
        &element.attributes,
        // 防止专有属性被公共映射重复处理。
        &["text", "dashed", "direction"],
    )
}

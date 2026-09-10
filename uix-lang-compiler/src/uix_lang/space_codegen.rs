// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入共享公共属性生成器与有序子节点生成器。
use super::codegen::{apply_common_attributes, generate_children};
// 引入 Space 所需的语法树、属性值生成器与诊断。
use super::{Diagnostic, Element, boolean_value, literal_string, numeric_value};

// 生成文档化 Space 标签对应的公开 Rust View。
pub(crate) fn generate_space(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 从现有公开 Space Widget 默认契约开始构建。
    let mut space = quote! { ::uix_app::prelude::Space::new() };
    // 按源码顺序应用 Space 专有属性。
    for attribute in &element.attributes {
        // 按属性名选择现有公开构建器。
        match attribute.name.as_str() {
            // 排列方向由公开文档中的确定关键字选择。
            "direction" => {
                // 方向影响构建器选择，因此要求编译期字符串字面量。
                let direction = literal_string(attribute, "Space direction")?;
                // 按文档化方向映射现有组件能力。
                space = match direction.as_str() {
                    // horizontal 是 Space 的默认方向。
                    "horizontal" => space,
                    // vertical 复用现有垂直构建器。
                    "vertical" => quote! { (#space).vertical() },
                    // 其他方向不能静默近似。
                    _ => {
                        // 返回带属性跨度的枚举诊断。
                        return Err(Diagnostic::new(
                            // 指向非法 direction 属性。
                            attribute.span,
                            // 说明具体非法值。
                            format!("Space direction={direction:?} 不受支持"),
                            // 给出已登记方向集合。
                            "使用 direction=\"horizontal\" 或 direction=\"vertical\"",
                        ));
                    }
                };
            }
            // 子项间距映射到现有自定义 SpaceSize。
            "gap" => {
                // 生成有限数值、px 字面量或受限表达式。
                let gap = numeric_value(attribute)?;
                // 通过公开 SpaceSize 保留组件对间距的所有权。
                space = quote! { (#space).size(::uix_app::prelude::SpaceSize::Custom(#gap)) };
            }
            // 换行状态支持布尔字面量与受限表达式。
            "wrap" => {
                // 生成确定的布尔表达式。
                let wrap = boolean_value(attribute)?;
                // 现有构建器直接接收完整布尔状态。
                space = quote! { (#space).wrap(#wrap) };
            }
            // 公共属性由统一生成器处理。
            _ => {}
        }
    }
    // 生成保持源码顺序与控制流语义的子节点向量。
    let children = generate_children(&element.children)?;
    // 由公开 ViewNode 组合 Space 与声明式子树。
    let base = quote! { ::uix_app::prelude::ViewNode::new(#space, #children) };
    // 专有属性消费后继续复用统一样式与事件诊断路径。
    apply_common_attributes(
        // 传入 Space 基础 View。
        base,
        // 传入原始属性列表。
        &element.attributes,
        // 防止专有属性被公共映射重复处理。
        &["direction", "gap", "wrap"],
    )
}

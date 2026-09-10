// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
// 引入共享元素属性查找。
use super::codegen::{apply_common_attributes, is_renderable_node};
use super::find_attribute;
// 引入 Skeleton 属性、诊断与共享值生成契约。
use super::{Diagnostic, Element, literal_string, numeric_value};

// 生成只投影静态外观配置的 Skeleton 叶节点。
pub(crate) fn generate_skeleton(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Skeleton 是叶组件，子树不能被静默忽略。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Skeleton 元素。
            element.span,
            // 说明骨架屏不接受子节点。
            "<Skeleton> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <Skeleton shape=\"rect\" width=\"200\" height=\"16\" />",
        ));
    }

    // 从公开默认构造器开始配置。
    let mut widget = quote! { ::uix_app::prelude::Skeleton::new() };
    // 可选形状必须在编译期映射为公开枚举。
    if let Some(attribute) = find_attribute(element, "shape") {
        // 形状是确定的关键字，不接受运行时表达式。
        let shape_name = literal_string(attribute, "Skeleton shape")?;
        // 把文档关键字映射到公开运行时枚举。
        let shape = match shape_name.as_str() {
            // 映射矩形占位。
            "rect" => quote! { ::uix_app::prelude::SkeletonShape::Rect },
            // 映射圆形占位。
            "circle" => quote! { ::uix_app::prelude::SkeletonShape::Circle },
            // 映射文字行占位。
            "text" => quote! { ::uix_app::prelude::SkeletonShape::Text },
            // 拒绝文档外的形状关键字。
            _ => {
                // 返回包含合法集合的确定性诊断。
                return Err(Diagnostic::new(
                    // 指向非法 shape 属性。
                    attribute.span,
                    // 说明未登记值。
                    format!("Skeleton shape={shape_name:?} 不受支持"),
                    // 给出完整合法集合。
                    "使用 rect、circle 或 text",
                ));
            }
        };
        // 调用公开形状构建器。
        widget = quote! { (#widget).shape(#shape) };
    }
    // 可选宽度复用有限 f32 字面量与受限表达式规则。
    if let Some(attribute) = find_attribute(element, "width") {
        // 生成尺寸令牌。
        let width = numeric_value(attribute)?;
        // 把宽度交给 Skeleton 自身尺寸契约。
        widget = quote! { (#widget).width(#width) };
    }
    // 可选高度复用有限 f32 字面量与受限表达式规则。
    if let Some(attribute) = find_attribute(element, "height") {
        // 生成尺寸令牌。
        let height = numeric_value(attribute)?;
        // 把高度交给 Skeleton 自身尺寸契约。
        widget = quote! { (#widget).height(#height) };
    }

    // 经公开 View 契约进入组件自己的同目录 UIX 声明壳。
    let view = quote! { ::uix_app::prelude::View::build(#widget) };
    // 消费 Skeleton 专有尺寸，避免重复应用到外层 View。
    apply_common_attributes(
        // 传入已经配置的骨架屏 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["shape", "width", "height"],
    )
}

// 查找元素上的具名属性。

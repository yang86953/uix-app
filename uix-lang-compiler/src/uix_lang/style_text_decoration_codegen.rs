// 引入卫生标识符与生成令牌流。
use proc_macro2::{Ident, TokenStream};
// 引入确定性 Rust 令牌拼接宏。
use quote::quote;

// 引入结构化诊断与样式属性。
use super::{Diagnostic, StyleProperty};

// 把 textDecoration 关键字映射为显式 UI 文本装饰字段更新。
pub(super) fn text_decoration_field(
    // 接收 map_style 闭包中的卫生 Style 标识符。
    style: &Ident,
    // 接收保留源码跨度的 textDecoration 属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 规范关键字不允许额外参数或大小写漂移。
    let variant = match property.value.source.trim() {
        // none 显式关闭装饰并可覆盖继承值。
        "none" => quote! { ::uix_app::prelude::TextDecoration::None },
        // underline 在每个视觉行下缘绘制装饰。
        "underline" => quote! { ::uix_app::prelude::TextDecoration::Underline },
        // overline 在每个视觉行上缘绘制装饰。
        "overline" => quote! { ::uix_app::prelude::TextDecoration::Overline },
        // line-through 在每个视觉行中部绘制删除线。
        "line-through" => quote! { ::uix_app::prelude::TextDecoration::LineThrough },
        // 未登记值必须在编译期明确拒绝。
        _ => {
            // 返回指向完整属性值的修复性诊断。
            return Err(Diagnostic::new(
                // 精确标记失败值而非整个文档。
                property.value.span,
                // 列出文档支持的闭合取值集合。
                "textDecoration 只支持 none、underline、overline 或 line-through",
                // 给出最常用的文本装饰写法。
                "使用 textDecoration: underline",
            ));
        }
    };
    // Some 区分显式 none 与未声明值。
    Ok(quote! { #style.text_decoration = ::std::option::Option::Some(#variant); })
}

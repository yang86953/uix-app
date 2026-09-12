// 引入卫生标识符与生成令牌流。
use proc_macro2::{Ident, TokenStream};
// 引入确定性 Rust 令牌拼接宏。
use quote::quote;

// 引入结构化诊断与样式属性。
use super::{Diagnostic, StyleProperty};

// 把 textAlign 关键字映射为显式 UI 文本水平对齐字段更新。
pub(super) fn text_align_field(
    // 接收 map_style 闭包中的卫生 Style 标识符。
    style: &Ident,
    // 接收保留源码跨度的 textAlign 属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 规范关键字不允许额外参数或大小写漂移。
    let variant = match property.value.source.trim() {
        // left 显式选择左对齐并可覆盖继承值。
        "left" => quote! { ::uix_app::prelude::TextAlign::Left },
        // right 选择内容框右缘对齐。
        "right" => quote! { ::uix_app::prelude::TextAlign::Right },
        // center 选择内容框水平居中。
        "center" => quote! { ::uix_app::prelude::TextAlign::Center },
        // justify 选择段落非末行的两端对齐。
        "justify" => quote! { ::uix_app::prelude::TextAlign::Justify },
        // 未登记值必须在编译期明确拒绝。
        _ => {
            // 返回指向完整属性值的修复性诊断。
            return Err(Diagnostic::new(
                // 精确标记失败值而非整个文档。
                property.value.span,
                // 列出文档支持的闭合取值集合。
                "textAlign 只支持 left、right、center 或 justify",
                // 给出最常用的显式对齐写法。
                "使用 textAlign: center",
            ));
        }
    };
    // Some 区分显式 left 与未声明值。
    Ok(quote! { #style.text_align = ::std::option::Option::Some(#variant); })
}

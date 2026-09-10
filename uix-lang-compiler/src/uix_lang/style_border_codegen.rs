// 引入卫生标识符与生成令牌流。
use proc_macro2::{Ident, TokenStream};
// 引入确定性 Rust 令牌拼接宏。
use quote::quote;

// 引入结构化诊断与样式属性。
use super::{Diagnostic, StyleProperty};

// 把 borderStyle 关键字映射为显式 UI 边框线型字段更新。
pub(super) fn border_style_field(
    // 接收 map_style 闭包中的卫生 Style 标识符。
    style: &Ident,
    // 接收保留源码跨度的 borderStyle 属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 规范关键字不允许额外参数或大小写漂移。
    let variant = match property.value.source.trim() {
        // none 显式关闭绘制而不修改边框宽度。
        "none" => quote! { ::uix_app::prelude::BorderStyle::None },
        // solid 显式覆盖继承的其他线型。
        "solid" => quote! { ::uix_app::prelude::BorderStyle::Solid },
        // dashed 选择连续相位的虚线绘制。
        "dashed" => quote! { ::uix_app::prelude::BorderStyle::Dashed },
        // dotted 选择圆帽点状绘制。
        "dotted" => quote! { ::uix_app::prelude::BorderStyle::Dotted },
        // double 选择两道同色描边。
        "double" => quote! { ::uix_app::prelude::BorderStyle::Double },
        // 未登记值必须在编译期明确拒绝。
        _ => {
            // 返回指向完整属性值的修复性诊断。
            return Err(Diagnostic::new(
                // 精确标记失败值而非整个文档。
                property.value.span,
                // 列出文档支持的闭合取值集合。
                "borderStyle 只支持 none、solid、dashed、dotted 或 double",
                // 给出最常用的显式线型写法。
                "使用 borderStyle: solid",
            ));
        }
    };
    // Some 区分显式 solid 与未声明时的默认 solid。
    Ok(quote! { #style.border_style = ::std::option::Option::Some(#variant); })
}

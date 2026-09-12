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

// 引入共享长度解析。
use super::style_value_codegen::parse_length;

// 把 borderRadius 的一到四值展开映射为 Style 圆角字段更新。
//
// 单值沿用既有 `border_radius` 标量字段（含主题数值 token），多值写入
// `border_radius_corners`；两种形式是同一属性的不叠加输入，落地时互相
// 清除对方，声明差异同步登记，供状态层整体覆盖。
pub(super) fn border_radius_field(
    // 接收 map_style 闭包中的卫生 Style 标识符。
    style: &Ident,
    // 接收同一闭包中的卫生 StyleDiff 标识符，用于多值显式登记。
    decl: &Ident,
    // 接收保留源码跨度的 borderRadius 属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 按连续空白切分一至四个分量。
    let parts: Vec<&str> = property
        .value
        .source
        .split_whitespace()
        .collect::<Vec<_>>()
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect();
    // 超过四个分量没有 CSS 展开语义。
    if parts.len() > 4 {
        return Err(Diagnostic::new(
            property.value.span,
            "borderRadius 最多接受四个分量（左上、右上、右下、左下）",
            "使用 borderRadius: 8px 4px 8px 4px",
        ));
    }
    // CSS 一至四值展开：1=全角；2=左上/右下、右上/左下；3=左上、右上/左下、右下；4=顺时针四角。
    let (tl, tr, br, bl) = match parts.len() {
        1 => (parts[0], parts[0], parts[0], parts[0]),
        2 => (parts[0], parts[1], parts[0], parts[1]),
        3 => (parts[0], parts[1], parts[2], parts[1]),
        // 四值已是顺时针顺序。
        _ => (parts[0], parts[1], parts[2], parts[3]),
    };
    // 每个分量复用共享长度解析（数值/px/主题数值 token，非负有限）。
    let tl = parse_length(tl, property)?;
    let tr = parse_length(tr, property)?;
    let br = parse_length(br, property)?;
    let bl = parse_length(bl, property)?;
    // 单值保持既有标量字段与旧生成物逐字节一致。
    if parts.len() == 1 {
        return Ok(quote! {
            #style.border_radius = #tl;
            // 单值落地时清除四角形式（值与声明同步），保持同一属性不叠加；
            // 声明登记显式清角（Some(None)），恢复路径不丢"回到单值"信息。
            #style.border_radius_corners = ::std::option::Option::None;
            #decl.border_radius_corners = ::std::option::Option::Some(
                ::std::option::Option::None,
            );
        });
    }
    // 多值写入四角形式并同步登记声明，允许显式差异整体覆盖低层单值。
    Ok(quote! {
        #style.border_radius_corners = ::std::option::Option::Some(
            ::uix_app::prelude::CornerRadii::new(#tl, #tr, #br, #bl)
        );
        #decl.border_radius_corners = ::std::option::Option::Some(
            ::std::option::Option::Some(
                ::uix_app::prelude::CornerRadii::new(#tl, #tr, #br, #bl)
            )
        );
    })
}

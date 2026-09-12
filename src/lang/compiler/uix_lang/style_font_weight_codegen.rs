// 引入卫生标识符、整数字面量与生成令牌流。
use proc_macro2::{Ident, Literal, TokenStream};
// 引入确定性 Rust 令牌拼接宏。
use quote::quote;

// 引入结构化诊断与样式属性。
use super::{Diagnostic, StyleProperty};

// 把 fontWeight 关键字或整数映射为显式 UI 字体粗细字段更新。
pub(super) fn font_weight_field(
    // 接收 map_style 闭包中的卫生 Style 标识符。
    style: &Ident,
    // 接收保留源码跨度的 fontWeight 属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 去除属性值外围空白后匹配文档值。
    let source = property.value.source.trim();
    // 关键字直接选择公开常量，数值走受控构造器。
    let font_weight = match source {
        // normal 对应四百常规字重并保留显式覆盖身份。
        "normal" => quote! { ::uix_app::prelude::FontWeight::NORMAL },
        // bold 对应七百粗体字重。
        "bold" => quote! { ::uix_app::prelude::FontWeight::BOLD },
        // 其他值必须解析为文档闭区间内的整数。
        _ => {
            // 小数、负数、单位和未知关键字都会在这里拒绝。
            let value = source
                .parse::<u16>()
                .map_err(|_| font_weight_diagnostic(property))?;
            // 不允许运行时构造器承担宏期范围错误。
            if !(100..=900).contains(&value) {
                // 返回同一闭合支持边界诊断。
                return Err(font_weight_diagnostic(property));
            }
            // 生成无后缀整数以调用公开受控构造器。
            let value = Literal::u16_unsuffixed(value);
            // 宏期已经证明构造必定成功。
            quote! {
                // 运行时公开构造器再次守卫数值不变量。
                ::uix_app::prelude::FontWeight::from_numeric(#value)
                    // 该分支只由已验证字面量生成。
                    .expect("UIX 已验证 fontWeight 位于 100 到 900")
            }
        }
    };
    // Some 区分显式 normal 与未声明值。
    Ok(quote! { #style.font_weight = ::std::option::Option::Some(#font_weight); })
}

// 构造 fontWeight 值级修复性诊断。
fn font_weight_diagnostic(
    // 接收原始属性以定位值跨度。
    property: &StyleProperty,
) -> Diagnostic {
    // 返回统一支持边界和可执行示例。
    Diagnostic::new(
        // 精确标记失败值。
        property.value.span,
        // 列出文档登记的关键字与整数闭区间。
        "fontWeight 只支持 normal、bold 或 100 到 900 的整数",
        // 给出关键字和数值两种合法形态。
        "使用 fontWeight: bold 或 fontWeight: 600",
    )
}

// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入结构化诊断与样式属性。
use super::{Diagnostic, StyleProperty};

// 把 userSelect 文档值映射到公开文字选择策略枚举。
pub(super) fn user_select_value(
    // 接收已经完成语法解析的属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 去除属性值两端空白以匹配闭合关键字。
    let source = property.value.source.trim();
    // 只接受运行时契约明确实现的四种值。
    match source {
        // auto 保留组件自身的默认选择能力。
        "auto" => Ok(quote! { ::uix_app::prelude::UserSelect::Auto }),
        // none 禁止普通文字选择并清理既有范围。
        "none" => Ok(quote! { ::uix_app::prelude::UserSelect::None }),
        // text 为支持文字范围的组件启用选择。
        "text" => Ok(quote! { ::uix_app::prelude::UserSelect::Text }),
        // all 将选择边界提升到最近的显式 all 子树。
        "all" => Ok(quote! { ::uix_app::prelude::UserSelect::All }),
        // 其他值不得静默退回组件默认策略。
        _ => Err(Diagnostic::new(
            // 指向完整 userSelect 值。
            property.value.span,
            // 说明闭合的支持边界。
            "userSelect 只支持 auto、none、text 或 all",
            // 给出最常用的禁选写法。
            "使用 userSelect: none",
        )),
    }
}

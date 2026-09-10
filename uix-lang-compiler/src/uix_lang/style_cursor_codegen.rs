// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入结构化诊断与样式属性。
use super::{Diagnostic, StyleProperty};

// 把 cursor 文档值映射到公开平台无关光标枚举。
pub(super) fn cursor_value(property: &StyleProperty) -> Result<TokenStream, Diagnostic> {
    // 去除属性值两端空白以匹配规范关键字。
    let source = property.value.source.trim();
    // 只接受文档登记并且各平台后端已实现的值。
    match source {
        // CSS default 对应平台默认箭头。
        "default" => Ok(quote! { ::uix_app::prelude::CursorType::Arrow }),
        // CSS pointer 对应可点击手形。
        "pointer" => Ok(quote! { ::uix_app::prelude::CursorType::Hand }),
        // CSS text 对应文字插入光标。
        "text" => Ok(quote! { ::uix_app::prelude::CursorType::IBeam }),
        // CSS move 对应四向移动光标。
        "move" => Ok(quote! { ::uix_app::prelude::CursorType::Move }),
        // CSS not-allowed 对应平台禁止操作光标。
        "not-allowed" => Ok(quote! { ::uix_app::prelude::CursorType::NotAllowed }),
        // 其他值不得静默退回默认箭头。
        _ => Err(Diagnostic::new(
            // 指向完整 cursor 值。
            property.value.span,
            // 说明支持边界。
            "cursor 只支持 default、pointer、text、move 或 not-allowed",
            // 给出最常用的可点击控件写法。
            "使用 cursor: pointer",
        )),
    }
}

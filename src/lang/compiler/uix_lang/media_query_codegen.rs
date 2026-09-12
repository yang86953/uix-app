// 引入过程宏字面量与令牌流。
use proc_macro2::{Ident, Literal, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入媒体条件 AST。
use super::{MediaLength, MediaQuery};

// 把 @media 条件生成为构建期窗口宽度判定表达式。
//
// 逻辑客户区宽度来自所属窗口树的根 frame；screen token 从构建期有效主题
// （局部 Provider 优先，其次窗口主题）读取。边界含等号。
pub(super) fn media_condition(query: &MediaQuery) -> TokenStream {
    let min = threshold(query.min_width.as_ref());
    let max = threshold(query.max_width.as_ref());
    quote! { ::uix_app::ui::__private::uix_media_matches(#min, #max) }
}

// 生成单个阈值的 Option<f32> 表达式。
fn threshold(length: Option<&MediaLength>) -> TokenStream {
    match length {
        None => quote! { ::std::option::Option::None },
        Some(MediaLength::Px(millipx)) => {
            let value = Literal::f32_unsuffixed(*millipx as f32 / 1000.0);
            quote! { ::std::option::Option::Some(#value) }
        }
        Some(MediaLength::Token(name)) => {
            let method = Ident::new(&screen_method(name), Span::call_site());
            quote! {
                ::std::option::Option::Some({
                    let __uix_tokens = ::uix_app::ui::__private::uix_effective_build_tokens();
                    __uix_tokens.#method()
                })
            }
        }
    }
}

// 把已登记 screen token 名映射到 ThemeTokens 方法名。
fn screen_method(name: &str) -> String {
    match name {
        "screenXS" => "screen_xs",
        "screenSM" => "screen_sm",
        "screenMD" => "screen_md",
        "screenLG" => "screen_lg",
        "screenXL" => "screen_xl",
        "screenXXL" => "screen_xxl",
        // 解析器只放行上述六个名称。
        other => unreachable!("未登记的 screen token {other}"),
    }
    .to_string()
}

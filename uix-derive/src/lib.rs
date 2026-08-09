//! UIX derive macros（E-06）：路由 key 派生宏。
//!
//! # `Display`
//!
//! 为单元变体枚举派生 `std::fmt::Display`：变体名转 kebab-case 作为文本
//! （`Home` → `home`，`UserProfile` → `user-profile`），用于
//! `Navigation<K>` 的语义事件与快照文本，免除手写 `impl Display`。
//!
//! ```ignore
//! #[derive(Clone, PartialEq, uix::Display)]
//! enum Page { Home, UserProfile }
//! ```
//!
//! 仅支持单元变体；带字段变体与 struct 会给出编译错误提示。

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

// 编译期语言转换器按 Gate 逐步接入，当前先编译并测试核心解析事实层。
#[allow(dead_code)]
mod uix_lang;

/// 为路由枚举派生 `Display`（E-06）。
#[proc_macro_derive(Display)]
pub fn derive_display(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let Data::Enum(data) = &input.data else {
        return syn::Error::new_spanned(
            name,
            "uix::Display derive 仅支持枚举（路由 key 通常是单元变体枚举）",
        )
        .to_compile_error()
        .into();
    };

    let mut arms = Vec::new();
    for variant in &data.variants {
        let variant_name = &variant.ident;
        match &variant.fields {
            Fields::Unit => {
                let text = kebab_case(&variant_name.to_string());
                arms.push(quote! {
                    Self::#variant_name => formatter.write_str(#text),
                });
            }
            _ => {
                return syn::Error::new_spanned(
                    variant,
                    "uix::Display derive 仅支持单元变体；带字段变体请手写 Display",
                )
                .to_compile_error()
                .into();
            }
        }
    }

    quote! {
        impl ::std::fmt::Display for #name {
            fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                match self {
                    #(#arms)*
                }
            }
        }
    }
    .into()
}

/// `CamelCase` → `kebab-case`（连续大写缩写按词边界拆分：`APIVersion` → `api-version`）。
fn kebab_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (index, &ch) in chars.iter().enumerate() {
        let boundary = index > 0
            && {
                let prev = chars[index - 1];
                ((prev.is_ascii_lowercase() || prev.is_ascii_digit())
                    && (ch.is_ascii_uppercase() || ch.is_ascii_digit()))
                    || (prev.is_ascii_uppercase()
                        && ch.is_ascii_uppercase()
                        && chars
                            .get(index + 1)
                            .is_some_and(|next| next.is_ascii_lowercase()))
            };
        if boundary {
            out.push('-');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

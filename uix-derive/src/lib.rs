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
// 在过程宏边界唯一解析 UIX 文件依赖图。
mod uix_import;
// 集中覆盖文件导入、导出与失败契约。
#[cfg(test)]
mod uix_import_tests;
// 定义公开 uix! 的内嵌与文件编译期入口。
mod uix_entry;

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

// 公开内嵌字符串与 .uix 文件编译期入口。
#[proc_macro]
pub fn uix(input: TokenStream) -> TokenStream {
    // 要求入口接收单个字符串字面量。
    let input = syn::parse_macro_input!(input as syn::LitStr);
    // 自动选择内嵌源码或 .uix 文件并返回生成令牌。
    uix_entry::expand_public(&input).into()
}

/// 把根为 `<App>` 的 UIX 文档编译为现有 `uix::App` builder。
///
/// 宏不会调用 `run()`；调用方可继续链式配置启动钩子、Agent 控制或标题栏：
///
/// ```ignore
/// uix_app!("src/main.uix").on_start(...).run();
/// ```
#[proc_macro]
pub fn uix_app(input: TokenStream) -> TokenStream {
    // 要求入口接收单个字符串字面量。
    let input = syn::parse_macro_input!(input as syn::LitStr);
    // 自动选择内嵌源码或 .uix 文件并返回 App builder。
    uix_entry::expand_app_public(&input).into()
}

/// 把 `.uix` 文档中的 `<Record>` 声明生成为模块级结构体，供 uix! 与 Rust 侧共同引用。
///
/// 在模块级调用一次即可让 Rust 侧函数引用语言面声明的业务模型：
///
/// ```ignore
/// uix_items!("src/main.uix");
///
/// fn submit(model: Profile) { ... }
/// ```
#[proc_macro]
pub fn uix_items(input: TokenStream) -> TokenStream {
    // 要求入口接收单个字符串字面量。
    let input = syn::parse_macro_input!(input as syn::LitStr);
    // 自动选择内嵌源码或 .uix 文件并返回 record 结构体令牌。
    uix_entry::expand_items_public(&input).into()
}

// 为根 crate 的真实消费者编译 Gate 保留内部内嵌入口。
#[doc(hidden)]
// 声明内部过程宏以保持既有消费者测试兼容。
#[proc_macro]
pub fn __uix_view_internal(input: TokenStream) -> TokenStream {
    // 要求测试入口接收单个内嵌字符串字面量。
    let input = syn::parse_macro_input!(input as syn::LitStr);
    // 委托共享内嵌入口并返回生成令牌。
    uix_entry::expand_inline(&input).into()
}

// 为根 crate 的真实消费者编译 Gate 保留内部 App 内嵌入口。
#[doc(hidden)]
// 声明内部过程宏以隔离文件路径与消费者类型检查。
#[proc_macro]
pub fn __uix_app_internal(input: TokenStream) -> TokenStream {
    // 要求测试入口接收单个内嵌字符串字面量。
    let input = syn::parse_macro_input!(input as syn::LitStr);
    // 委托共享内嵌 App 入口并返回生成令牌。
    uix_entry::expand_app_inline(&input).into()
}

/// `CamelCase` → `kebab-case`（连续大写缩写按词边界拆分：`APIVersion` → `api-version`）。
fn kebab_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (index, &ch) in chars.iter().enumerate() {
        let boundary = index > 0 && {
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

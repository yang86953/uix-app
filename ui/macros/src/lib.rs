//! `ui!` — 声明式 UI 宏（proc-macro）。
//!
//! 语法：
//! ```ignore
//! ui! {
//!     WidgetName(arg1, prop: value) {
//!         ChildName(arg),
//!         AnotherChild { SubChild }
//!     }
//! }
//! ```
//!
//! 展开为 UI crate 的 `widget::WidgetNode` 树，使用构建器链式调用：
//! - `Name("text")` → `Name::new("text")`
//! - `prop: value` → `.prop(value)`
//! - `{ children }` → `vec![...]`

use proc_macro::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    token::{Brace, Comma, Paren},
    Expr, Ident, Token,
};

// ════════════════════════════════════════════════════════════════════════════
// AST 节点
// ════════════════════════════════════════════════════════════════════════════

/// 单个属性参数：`ident: expr` 或单独的 `expr`
struct Arg {
    /// 可选的属性名（命名参数）
    name: Option<Ident>,
    /// 属性值
    value: Expr,
}

impl Parse for Arg {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // 尝试解析 `ident: expr`
        let fork = input.fork();
        if let Ok(name) = fork.parse::<Ident>() {
            if fork.peek(Token![:]) {
                // 命名参数: ident: expr
                input.parse::<Ident>()?;
                input.parse::<Token![:]>()?;
                let value = input.parse::<Expr>()?;
                return Ok(Arg {
                    name: Some(name),
                    value,
                });
            }
        }
        // 位置参数: expr
        let value = input.parse::<Expr>()?;
        Ok(Arg { name: None, value })
    }
}

/// 一个 widget 节点：`Name(args) { children }` 或 `Name(args)` 或 `Name { children }`
struct WidgetNode {
    /// Widget 类型名（如 Container、Label）
    name: Ident,
    /// 构造函数参数（括号内的内容）
    args: Vec<Arg>,
    /// 子节点（花括号内的内容）
    children: Vec<WidgetNode>,
}

impl Parse for WidgetNode {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let mut args = Vec::new();
        let mut children = Vec::new();

        // 解析可选的 `(args)`
        if input.peek(Paren) {
            let content;
            parenthesized!(content in input);
            while !content.is_empty() {
                args.push(content.parse()?);
                if content.is_empty() {
                    break;
                }
                content.parse::<Comma>()?;
            }
        }

        // 解析可选的 `{ children }`
        if input.peek(Brace) {
            let content;
            braced!(content in input);
            while !content.is_empty() {
                children.push(content.parse()?);
                if content.is_empty() {
                    break;
                }
                // 逗号是可选的
                if content.peek(Comma) {
                    content.parse::<Comma>()?;
                }
            }
        }

        Ok(WidgetNode {
            name,
            args,
            children,
        })
    }
}

/// 顶层输入：单个 widget 节点或 `{ widget }` 包裹的节点
struct UiInput {
    root: WidgetNode,
}

impl Parse for UiInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // 支持 `ui! { Widget(...) { ... } }` 也支持 `ui!(Widget(...))`
        let root = input.parse::<WidgetNode>()?;
        Ok(UiInput { root })
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 代码生成
// ════════════════════════════════════════════════════════════════════════════

fn crate_path(found: FoundCrate, self_path: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    match found {
        FoundCrate::Itself => self_path,
        FoundCrate::Name(name) => {
            let ident = proc_macro2::Ident::new(&name, proc_macro2::Span::call_site());
            quote! { ::#ident }
        }
    }
}

fn ui_crate_path() -> proc_macro2::TokenStream {
    if let Ok(found) = crate_name("uix-ui") {
        return crate_path(found, quote! { ::uix_ui });
    }

    if let Ok(found) = crate_name("uix") {
        let root = crate_path(found, quote! { ::uix });
        return quote! { #root::ui };
    }

    quote! { ::uix::ui }
}

impl WidgetNode {
    /// 生成该 widget 节点的 Rust 代码。
    fn expand(&self, ui_path: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        let new_call = self.expand_constructor(ui_path);

        if self.children.is_empty() {
            // Leaf widget
            quote! {
                #ui_path::widget::WidgetNode::leaf(::std::boxed::Box::new(#new_call))
            }
        } else {
            // Parent widget with children
            let child_nodes: Vec<_> = self.children.iter().map(|c| c.expand(ui_path)).collect();
            quote! {
                #ui_path::widget::WidgetNode::new(
                    ::std::boxed::Box::new(#new_call),
                    ::std::vec![#(#child_nodes),*],
                )
            }
        }
    }

    /// 生成构造函数调用链：`Name::new(arg0).prop1(val1).prop2(val2)`
    fn expand_constructor(&self, ui_path: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        let name = &self.name;
        let mut positional_args = Vec::new();
        let mut named_args = Vec::new();

        for arg in &self.args {
            match &arg.name {
                Some(prop_name) => {
                    let val = &arg.value;
                    named_args.push(quote! { .#prop_name(#val) });
                }
                None => {
                    let val = &arg.value;
                    positional_args.push(val);
                }
            }
        }

        if positional_args.is_empty() {
            quote! {
                #ui_path::#name::new() #(#named_args)*
            }
        } else {
            quote! {
                #ui_path::#name::new(#(#positional_args),*) #(#named_args)*
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 入口
// ════════════════════════════════════════════════════════════════════════════

/// 声明式 UI 树构建宏。
///
/// # 语法
///
/// ```ignore
/// ui! {
///     Container(bg: black, dir: Row) {
///         Label("Hello", color: white, font_size: 14),
///         Button("Click"),
///         Space(height: 12),
///     }
/// }
/// ```
///
/// 等价于：
///
/// ```ignore
/// WidgetNode::new(
///     Box::new(Container::new().bg(black).dir(Row)),
///     vec![
///         WidgetNode::leaf(Box::new(Label::new("Hello").color(white).font_size(14))),
///         WidgetNode::leaf(Box::new(Button::new("Click"))),
///         WidgetNode::leaf(Box::new(Space::new().height(12))),
///     ],
/// )
/// ```
#[proc_macro]
pub fn ui(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as UiInput);
    let ui_path = ui_crate_path();
    let expanded = input.root.expand(&ui_path);
    TokenStream::from(expanded)
}

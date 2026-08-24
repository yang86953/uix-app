// 把顶层 <Visual> 声明编译为与 Rust 内核同模块的静态视觉常量。

use proc_macro2::{Ident, TokenStream};
use quote::quote;

use super::{
    Declaration, Diagnostic, Document, Expression, ExpressionKind, VisualValue,
    generate_expression, mark_source_tokens, rust_identifier, with_visual_source_marker,
};

// 生成文档内全部 Visual 常量，确保构造、测量、绘制与 UIX 组合共享同一份值。
pub(crate) fn generate_visual_items(document: &Document) -> Result<TokenStream, Diagnostic> {
    let constants = document
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Declaration::Visual(visual) => Some(visual),
            _ => None,
        })
        .map(|visual| {
            with_visual_source_marker(&visual.name, || {
                let name: Ident = syn::parse_str(&visual.name).expect("Visual 名称已在解析期验证");
                let rust_type: Ident =
                    syn::parse_str(&visual.rust_type).expect("Visual 类型已在解析期验证");
                let fields = visual
                    .fields
                    .iter()
                    .map(|field| {
                        let name = rust_identifier(&field.rust_name, field.span)?;
                        let value = match &field.value {
                            VisualValue::Literal(value) => {
                                let value = syn::LitStr::new(value, proc_macro2::Span::call_site());
                                quote! { #value }
                            }
                            VisualValue::Expression(expression) => {
                                generate_visual_expression(expression)?
                            }
                        };
                        Ok(quote! { #name: #value })
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?;
                Ok(quote! {
                    // 由同目录 UIX 唯一声明的静态视觉事实，供 Rust 内核全部阶段共享。
                    pub(crate) const #name: #rust_type = #rust_type {
                        #(#fields,)*
                    };
                })
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(quote! { #(#constants)* })
}

// Visual 顶层数组保持固定数组字面量，不沿用界面 state 的 Vec 语义。
fn generate_visual_expression(expression: &Expression) -> Result<TokenStream, Diagnostic> {
    if let ExpressionKind::Array(items) = &expression.kind {
        let items = items
            .iter()
            .map(generate_visual_expression)
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(mark_source_tokens(
            quote! { [#(#items),*] },
            expression.span,
        ));
    }
    generate_expression(expression, None)
}

// 引入过程宏令牌与 Rust 标签生命周期。
use proc_macro2::{Ident, Span, TokenStream};
// 引入 Rust 标签生命周期语法节点。
use syn::Lifetime;
// 引入确定性令牌拼接。
use quote::quote;

// 引入独立 action 语句 AST、表达式生成与诊断。
use super::{
    ActionBlock, ActionStatement, Diagnostic, LoweredActionBlock,
    generate_expression_without_source_marker,
};

// 把一个内联 action 生成为独占标签块，保证 return 不越过调用边界。
pub(super) fn generate_lowered_action(
    action: &LoweredActionBlock,
) -> Result<TokenStream, Diagnostic> {
    let label = Lifetime::new(&format!("'{}", action.label), Span::mixed_site());
    let statements = generate_statements(&action.block, &label)?;
    Ok(quote! { #label: { #(#statements)* } })
}

fn generate_statements(
    block: &ActionBlock,
    label: &Lifetime,
) -> Result<Vec<TokenStream>, Diagnostic> {
    block
        .statements
        .iter()
        .map(|statement| generate_statement(statement, label))
        .collect()
}

fn generate_statement(
    statement: &ActionStatement,
    label: &Lifetime,
) -> Result<TokenStream, Diagnostic> {
    match statement {
        ActionStatement::Let {
            name,
            initializer,
            mutable,
            span,
        } => {
            let name = syn::parse_str::<Ident>(name).map_err(|_| {
                Diagnostic::new(
                    *span,
                    "action 局部不能映射为 Rust 标识符",
                    "改用非 Rust 关键字的小写局部名称",
                )
            })?;
            let initializer = generate_expression_without_source_marker(initializer, None)?;
            if *mutable {
                Ok(quote! { let mut #name = #initializer; })
            } else {
                Ok(quote! { let #name = #initializer; })
            }
        }
        ActionStatement::Assign { name, value, span } => {
            let name = syn::parse_str::<Ident>(name).map_err(|_| {
                Diagnostic::new(
                    *span,
                    "action 赋值目标不能映射为 Rust 标识符",
                    "改用非 Rust 关键字的小写局部名称",
                )
            })?;
            let value = generate_expression_without_source_marker(value, None)?;
            Ok(quote! { #name = #value; })
        }
        ActionStatement::Expression { expression, .. } => {
            let expression = generate_expression_without_source_marker(expression, None)?;
            Ok(quote! { #expression; })
        }
        ActionStatement::If {
            condition,
            then_block,
            else_block,
            ..
        } => {
            let condition = generate_expression_without_source_marker(condition, None)?;
            let then_statements = generate_statements(then_block, label)?;
            if let Some(else_block) = else_block {
                let else_statements = generate_statements(else_block, label)?;
                Ok(quote! {
                    if #condition { #(#then_statements)* } else { #(#else_statements)* }
                })
            } else {
                Ok(quote! { if #condition { #(#then_statements)* } })
            }
        }
        ActionStatement::Return { value, .. } => {
            if let Some(value) = value {
                let value = generate_expression_without_source_marker(value, None)?;
                Ok(quote! { break #label #value; })
            } else {
                Ok(quote! { break #label (); })
            }
        }
    }
}

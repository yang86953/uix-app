// 统一生成 uix_items! 所需的模块级 Record 类型与 Visual 静态项。

use proc_macro2::TokenStream;
use quote::quote;

use super::{Diagnostic, Document};

// 保持声明类别的生成器独立，再按稳定顺序合并为单一模块项目标。
pub(crate) fn generate_document_items(document: &Document) -> Result<TokenStream, Diagnostic> {
    let records = super::record_codegen::generate_record_items(document)?;
    let visuals = super::visual_codegen::generate_visual_items(document)?;
    Ok(quote! {
        #records
        #visuals
    })
}

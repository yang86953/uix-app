// 引入过程宏标识符与令牌流。
use proc_macro2::{Ident, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入组件类型、文档声明与诊断。
use super::{
    Declaration, Diagnostic, Document, WidgetValueType, rust_identifier, with_record_source_marker,
};

// 生成文档内全部 record 的模块级结构体声明，供 uix_items! 与 Rust 侧引用。
pub(crate) fn generate_record_items(document: &Document) -> Result<TokenStream, Diagnostic> {
    // 生成每个 record 的结构体令牌。
    let structs = document
        // 遍历声明。
        .declarations
        // 借用声明序列。
        .iter()
        // 只保留 record 声明。
        .filter_map(|declaration| match declaration {
            // 提取 record。
            Declaration::Record(record) => Some(record),
            // 其余声明不生成结构体。
            _ => None,
        })
        // 转换每个 record。
        .map(|record| {
            with_record_source_marker(&record.name, || {
                // 验证 record 名可映射为 Rust 标识符。
                let name: Ident = syn::parse_str(&record.name).expect("record 名已在解析期验证");
                // 生成全部字段令牌。
                let fields = record
                    // 遍历声明字段。
                    .fields
                    // 借用字段序列。
                    .iter()
                    // 转换每个字段。
                    .map(|field| {
                        // 验证字段名。
                        let field_name = rust_identifier(&field.name, field.span)?;
                        // 生成字段类型令牌。
                        let field_type = value_type_tokens(field.kind.clone());
                        // 返回公开字段声明。
                        Ok(quote! { pub #field_name: #field_type })
                    })
                    // 收集或返回首个诊断。
                    .collect::<Result<Vec<_>, Diagnostic>>()?;
                // 返回模块级结构体声明。
                Ok(quote! {
                    // 语言面 <Record> 声明的模块级业务模型，供 uix! 与 Rust 侧共同引用。
                    #[derive(::std::clone::Clone)]
                    pub struct #name {
                        #(#fields,)*
                    }
                })
            })
        })
        // 收集或返回首个诊断。
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    // 返回全部结构体声明。
    Ok(quote! { #(#structs)* })
}

// 把语言类型映射为 Rust 类型令牌。
pub(crate) fn value_type_tokens(value_type: WidgetValueType) -> TokenStream {
    // 按白名单类型生成令牌。
    match value_type {
        // String 对应拥有所有权的字符串。
        WidgetValueType::String => quote! { ::std::string::String },
        // number 对应规范约定的 f64。
        WidgetValueType::Number => quote! { f64 },
        // bool 对应 Rust 布尔类型。
        WidgetValueType::Bool => quote! { bool },
        // u32 对应无符号计数类型。
        WidgetValueType::U32 => quote! { u32 },
        // usize 对应索引类型。
        WidgetValueType::USize => quote! { usize },
        // f32 对应单精度浮点类型。
        WidgetValueType::F32 => quote! { f32 },
        // i32 对应有符号整数类型。
        WidgetValueType::I32 => quote! { i32 },
        // Date 对应公开日期语义类型。
        WidgetValueType::Date => quote! { ::uix::prelude::Date },
        // Time 对应公开时间语义类型。
        WidgetValueType::Time => quote! { ::uix::prelude::Time },
        // Color 对应公开颜色语义类型。
        WidgetValueType::Color => quote! { ::uix::prelude::Color },
        // Point 对应公开坐标语义类型。
        WidgetValueType::Point => quote! { ::uix::prelude::Point },
        // CascaderValue 对应公开级联路径类型。
        WidgetValueType::CascaderValue => quote! { ::uix::prelude::CascaderValue },
        // 多选集合对应字符串 HashSet。
        WidgetValueType::HashSetOfString => {
            quote! { ::std::collections::HashSet<::std::string::String> }
        }
        // 字符串向量对应 Vec<String>。
        WidgetValueType::VecOfString => quote! { ::std::vec::Vec<::std::string::String> },
        // 数值向量对应 Vec<f64>。
        WidgetValueType::VecOfNumber => quote! { ::std::vec::Vec<f64> },
        // record 向量引用文档生成的模块级结构体。
        WidgetValueType::VecOfRecord(name) => {
            // 验证名称可映射为 Rust 标识符。
            let ident = syn::parse_str::<Ident>(&name).expect("record 名已在解析期验证");
            // 返回 record 元素向量类型。
            quote! { ::std::vec::Vec<#ident> }
        }
        // 上传队列对应 Vec<UploadFile>。
        WidgetValueType::VecOfUploadFile => {
            quote! { ::std::vec::Vec<::uix::prelude::UploadFile> }
        }
        // 可空字符串对应 Option<String>。
        WidgetValueType::OptionalString => {
            quote! { ::std::option::Option<::std::string::String> }
        }
        // record 名直接引用模块级结构体。
        WidgetValueType::Record(name) => {
            // 验证名称可映射为 Rust 标识符。
            let ident = syn::parse_str::<Ident>(&name).expect("record 名已在解析期验证");
            // 返回模块级类型标识符。
            quote! { #ident }
        }
    }
}

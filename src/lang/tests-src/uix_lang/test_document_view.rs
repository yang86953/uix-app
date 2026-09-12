//! `uix_lang` 各 codegen 测试共享的稳定令牌视图生成入口。
//! 自 uix_lang/mod.rs 移入，经 #[path] 挂载并由 mod.rs re-export 保持
//! 原命名空间（use super::generate_test_document_view 不变），不进发布包。

use super::{Diagnostic, generate_document_view, parse_document};

#[cfg(test)]
pub(crate) fn generate_test_document_view(source: &str) -> Result<String, Diagnostic> {
    // 先执行完整文档解析与声明校验。
    let document = parse_document(source)?;
    // 再经过组件展开生成稳定令牌文本。
    generate_document_view(&document).map(|tokens| tokens.to_string())
}

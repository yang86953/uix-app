//! `uix_lang` 各 codegen 测试共享的稳定令牌视图生成入口。
//! 自 uix_lang/mod.rs 移入，经 #[path] 挂载并由 mod.rs re-export 保持
//! 原命名空间（use super::generate_test_document_view 不变），不进发布包。

use super::{Diagnostic, generate_document_view, parse_document};

#[cfg(test)]
pub(crate) fn generate_test_document_view(source: &str) -> Result<String, Diagnostic> {
    // 这些历史组件生成合同属于两包集成测试；显式读取开发工作区库声明，
    // 不把官方控件重新放回框架的生产默认目录。
    let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../uix-widgets");
    crate::lang::compiler::components::with_project(&project, || {
        let document = parse_document(source)?;
        generate_document_view(&document).map(|tokens| tokens.to_string())
    })
    .expect("组件生成测试需要同级 uix-widgets 的实际库声明")
}

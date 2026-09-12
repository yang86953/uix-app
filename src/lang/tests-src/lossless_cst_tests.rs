//! `uix-lang-compiler/lossless_cst.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::{CstTokenKind, LosslessCst};
use crate::source_graph::SourceId;

#[test]
fn lossless_cst_reconstructs_comments_expressions_and_utf8_exactly() {
    let source = "// 注释 <fake>\n<Column><Text title=\"a > b\">你好</Text></Column>";
    let cst = LosslessCst::from_source(SourceId::from_source_name("demo.uix"), source);
    assert_eq!(cst.reconstruct(), source);
    assert!(
        cst.tokens()
            .iter()
            .any(|token| token.kind == CstTokenKind::LineComment)
    );
    assert!(cst.tokens().iter().any(|token| {
        token.kind == CstTokenKind::OpeningTag && token.tag_name.as_deref() == Some("Text")
    }));
}

//! 自 src/ui/theme/i18n.rs 迁出的文档测试样例；原检查语义保留，仅源仓 RUSTDOCFLAGS
//! --cfg uix_repo_doc_contract 下的 cargo test --doc 收集，不进发布包。


/// 迁出的文档样例 1（原 src/ui/theme/i18n.rs:5）：
/// ```ignore
/// register_translations(&[("common.save", "保存"), ("common.cancel", "取消")]);
/// embed(label(t!("common.save")));
/// ```
pub struct DocExample1;

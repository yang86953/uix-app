//! 自 src/ui/widgets/combinators/label.rs 迁出的文档测试样例；原检查语义保留，仅源仓 RUSTDOCFLAGS
//! --cfg uix_repo_doc_contract 下的 cargo test --doc 收集，不进发布包。


/// 迁出的文档样例 1（原 src/ui/widgets/combinators/label.rs:54）：
/// ```ignore
/// label("Hello");
/// label(move || format!("计数: {}", count.get()));
/// count.map_text(|n| format!("计数: {n}")); // 等价，少手写 clone
/// ```
pub struct DocExample1;

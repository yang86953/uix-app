//! 自 src/draw/geometry/path.rs 迁出的文档测试样例；原检查语义保留，仅源仓 RUSTDOCFLAGS
//! --cfg uix_repo_doc_contract 下的 cargo test --doc 收集，不进发布包。


/// 迁出的文档样例 1（原 src/draw/geometry/path.rs:170）：
/// ```ignore
/// let mut pb = PathBuilder::new();
/// pb.move_to(10.0, 10.0);
/// pb.line_to(100.0, 10.0);
/// pb.line_to(100.0, 100.0);
/// pb.close();
/// let path = pb.build();
/// ```
pub struct DocExample1;

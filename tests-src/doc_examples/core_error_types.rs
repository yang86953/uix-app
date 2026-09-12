//! 自 src/core/error/types.rs 迁出的文档测试样例；原检查语义保留，仅源仓 RUSTDOCFLAGS
//! --cfg uix_repo_doc_contract 下的 cargo test --doc 收集，不进发布包。


/// 迁出的文档样例 1（原 src/core/error/types.rs:19）：
/// ```ignore
/// let err = Error::new(Errc::NotFound, "资源未找到");
/// let err = Error::fatal(Errc::OutOfRange, "数组越界");
/// let err = Error::invalid_arg("参数 id 不能为空");
/// ```
pub struct DocExample1;

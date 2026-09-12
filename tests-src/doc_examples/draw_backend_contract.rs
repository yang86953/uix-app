//! 自 src/draw/backend/contract.rs 迁出的文档测试样例；原检查语义保留，仅源仓 RUSTDOCFLAGS
//! --cfg uix_repo_doc_contract 下的 cargo test --doc 收集，不进发布包。


/// 迁出的文档样例 1（原 src/draw/backend/contract.rs:197）：
/// ```compile_fail
/// use uix_app::draw::backend::RenderBackend;
///
/// fn needs_send<T: Send>(_value: T) {}
///
/// fn backend_cannot_cross_threads(backend: Box<dyn RenderBackend>) {
///     needs_send(backend);
/// }
/// ```
pub struct DocExample1;

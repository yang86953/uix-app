//! 自 src/platform/presentation/contracts/mod.rs 迁出的文档测试样例；原检查语义保留，仅源仓 RUSTDOCFLAGS
//! --cfg uix_repo_doc_contract 下的 cargo test --doc 收集，不进发布包。


/// 迁出的文档样例 1（原 src/platform/presentation/contracts/mod.rs:221）：
/// ```compile_fail
/// use uix_app::platform::presentation::NativeSurfaceHandle;
///
/// fn needs_send<T: Send>(_value: T) {}
///
/// let handle = unsafe { NativeSurfaceHandle::from_raw(std::ptr::null_mut()) };
/// needs_send(handle);
/// ```
pub struct DocExample1;

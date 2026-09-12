//! 自 src/diagnostics/mod.rs 迁出的文档测试样例；原检查语义保留，仅源仓 RUSTDOCFLAGS
//! --cfg uix_repo_doc_contract 下的 cargo test --doc 收集，不进发布包。


/// 迁出的文档样例 1（原 src/diagnostics/mod.rs:17）：
/// ```compile_fail
/// use uix_app::diagnostics::reporting::ReportingModule;
/// ```
pub struct DocExample1;


/// 迁出的文档样例 2（原 src/diagnostics/mod.rs:21）：
/// ```compile_fail
/// use uix_app::diagnostics::recovery::RecoveryModule;
/// ```
pub struct DocExample2;


/// 迁出的文档样例 3（原 src/diagnostics/mod.rs:25）：
/// ```compile_fail
/// use uix_app::diagnostics::pending::PendingFailureQueue;
/// ```
pub struct DocExample3;


/// 迁出的文档样例 4（原 src/diagnostics/mod.rs:29）：
/// ```compile_fail
/// use uix_app::diagnostics::report::ReportOrigin;
/// ```
pub struct DocExample4;


/// 迁出的文档样例 5（原 src/diagnostics/mod.rs:33）：
/// ```compile_fail
/// use uix_app::diagnostics::config::DiagnosticsConfig;
/// ```
pub struct DocExample5;


/// 迁出的文档样例 6（原 src/diagnostics/mod.rs:37）：
/// ```compile_fail
/// use uix_app::diagnostics::crash::CrashReport;
/// ```
pub struct DocExample6;

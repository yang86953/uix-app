//! 自 src/core/mod.rs 迁出的文档测试样例；原检查语义保留，仅源仓 RUSTDOCFLAGS
//! --cfg uix_repo_doc_contract 下的 cargo test --doc 收集，不进发布包。


/// 迁出的文档样例 1（原 src/core/mod.rs:21）：
/// ```compile_fail
/// // 旧平铺路径不得复活：WidgetId 归 identity 模块。
/// use uix_app::core::widget_id::WidgetId;
/// ```
pub struct DocExample1;


/// 迁出的文档样例 2（原 src/core/mod.rs:26）：
/// ```compile_fail
/// // 旧平铺路径不得复活：WindowId 归 identity 模块。
/// use uix_app::core::window_id::WindowId;
/// ```
pub struct DocExample2;

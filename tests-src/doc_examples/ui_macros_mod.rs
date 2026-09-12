//! 自 src/ui/macros/mod.rs 迁出的文档测试样例；原检查语义保留，仅源仓 RUSTDOCFLAGS
//! --cfg uix_repo_doc_contract 下的 cargo test --doc 收集，不进发布包。


/// 迁出的文档样例 1（原 src/ui/macros/mod.rs:8）：
/// ```ignore
/// pub struct Button { text: String, ... }
///
/// impl_widget!(Button; Layout, Render, Event, Lifecycle; tab_index => 1);
///
/// impl WidgetLayout for Button {
///     fn measure(&self, constraints: Constraints) -> Size { ... }
/// }
/// impl WidgetRender for Button {
///     fn render(&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) { ... }
/// }
/// ```
pub struct DocExample1;

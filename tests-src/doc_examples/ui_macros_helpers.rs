//! 自 src/ui/macros/helpers.rs 迁出的文档测试样例；原检查语义保留，仅源仓 RUSTDOCFLAGS
//! --cfg uix_repo_doc_contract 下的 cargo test --doc 收集，不进发布包。


/// 迁出的文档样例 1（原 src/ui/macros/helpers.rs:41）：
/// ```ignore
/// column(views![
///     label("Hello"),
///     button("+1").primary().on_click(&count, |c| c.set(c.get() + 1)),
/// ])
/// ```
pub struct DocExample1;


/// 迁出的文档样例 2（原 src/ui/macros/helpers.rs:67）：
/// ```ignore
/// keyframe! {
///     #[derive(Debug, PartialEq)]
///     pub struct EnterFrame {
///         pub opacity: f32,
///         pub offset_y: f32,
///     }
/// }
/// ```
pub struct DocExample2;


/// 迁出的文档样例 3（原 src/ui/macros/helpers.rs:129）：
/// ```ignore
/// .root(with_cloned!(active, count, theme; {
///     shell(active, &count, &theme)
/// }))
/// .on_start(with_cloned!(theme, ticks; |handle| {
///     theme.set_handle(handle.clone());
///     // …
/// }))
/// ```
pub struct DocExample3;

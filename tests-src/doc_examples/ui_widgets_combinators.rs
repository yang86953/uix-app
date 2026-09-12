//! 自 src/ui/widgets/combinators.rs 迁出的文档测试样例；原检查语义保留，仅源仓 RUSTDOCFLAGS
//! --cfg uix_repo_doc_contract 下的 cargo test --doc 收集，不进发布包。


/// 迁出的文档样例 1（原 src/ui/widgets/combinators.rs:8）：
/// ```ignore
/// use crate::ui::view::*;
///
/// let ui = column((
///     label("Hello").font_size(24.0).color(Color::blue()),
///     button("Click").primary().on_click_fn(|| println!("clicked")),
/// )).padding(16.0);
/// ```
pub struct DocExample1;


/// 迁出的文档样例 2（原 src/ui/widgets/combinators.rs:64）：
/// ```ignore
/// column(show(visible, label("详情")))
/// ```
pub struct DocExample2;


/// 迁出的文档样例 3（原 src/ui/widgets/combinators.rs:84）：
/// ```ignore
/// column((label("标题"), optional_banner))
/// ```
pub struct DocExample3;


/// 迁出的文档样例 4（原 src/ui/widgets/combinators.rs:830）：
/// ```ignore
/// button("保存").primary().on_click(&state, |s| save(s));
/// button("关闭").on_click_fn(|| close());
/// ```
pub struct DocExample4;


/// 迁出的文档样例 5（原 src/ui/widgets/combinators.rs:862）：
/// ```ignore
/// input().placeholder("请输入用户名").on_change(|v| println!("输入: {}", v))
/// ```
pub struct DocExample5;


/// 迁出的文档样例 6（原 src/ui/widgets/combinators.rs:963）：
/// ```ignore
/// input().placeholder("搜索...").on_change(|v| search(v))
/// ```
pub struct DocExample6;

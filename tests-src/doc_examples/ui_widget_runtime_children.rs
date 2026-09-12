//! 自 src/ui/widget_runtime/children.rs 迁出的文档测试样例；原检查语义保留，仅源仓 RUSTDOCFLAGS
//! --cfg uix_repo_doc_contract 下的 cargo test --doc 收集，不进发布包。


/// 迁出的文档样例 1（原 src/ui/widget_runtime/children.rs:12）：
/// ```ignore
/// children: WidgetChildren,
/// ```
pub struct DocExample1;


/// 迁出的文档样例 2（原 src/ui/widget_runtime/children.rs:17）：
/// ```ignore
/// build => (&self) -> Vec<Box<dyn Widget>> { self.children.take() }
/// ```
pub struct DocExample2;


/// 迁出的文档样例 3（原 src/ui/widget_runtime/children.rs:22）：
/// ```ignore
/// pub fn child(self, w: impl Widget + 'static) -> Self { self.children.add(self, w) }
/// pub fn children(self, widgets: Vec<Box<dyn Widget>>) -> Self { self.children.set_all(self, widgets) }
/// ```
pub struct DocExample3;

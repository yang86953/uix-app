//! `src/ui/widget_runtime/widget_handle.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl WidgetHandle） ——

impl WidgetHandle {
    #[cfg(test)]
    #[doc(hidden)]
    pub fn new(id: WidgetId, tree: &Rc<RefCell<WidgetTree>>) -> Self {
        Self {
            id,
            tree: Some(Rc::downgrade(tree)),
            app_state: None,
        }
    }
}

//! `src/app/application/application/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl App） ——

impl App {
    // 测试目标保留根窗口句柄快捷入口，供外部 GUI 测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn app_handle(&self) -> AppHandle {
        self.app_handle_for_window(WindowId::ROOT)
    }
}

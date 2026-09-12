//! `src/ui/widgets/input/transfer/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Transfer） ——

impl Transfer {
    // 测试目标保留传输组件 frame 注入入口，供交互几何测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn set_frame_for_test(&self, frame: Rect) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
    }
}

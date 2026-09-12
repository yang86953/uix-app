//! `src/ui/widgets/display/skeleton/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Skeleton） ——

impl Skeleton {
    // 测试目标保留骨架头像区域观测入口，供占位布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn avatar_rect_for_test(&self, frame: Rect) -> Rect {
        self.avatar_rect(Self::normalized_frame(frame))
    }

    // 测试目标保留骨架段落区域观测入口，供占位布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn paragraph_rects_for_test(&self, frame: Rect) -> Vec<Rect> {
        let mut rects = Vec::with_capacity(self.paragraph_lines.max(2));
        self.for_each_paragraph_rect(Self::normalized_frame(frame), |rect| rects.push(rect));
        rects
    }

    // 测试目标保留骨架动画 phase 观测入口，供占位动画测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn phase_for_test(&self) -> f32 {
        self.phase.get()
    }
}

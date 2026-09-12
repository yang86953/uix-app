//! `src/draw/painting/recorder/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl CommandRecorder） ——

impl CommandRecorder {
    // 测试目标保留 scratch surface 尺寸观测入口，供 recorder 生命周期测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn scratch_surface_size(&self) -> (i32, i32) {
        (
            self.canvas.scratch.surface().width(),
            self.canvas.scratch.surface().height(),
        )
    }

    // 测试目标保留 offscreen scratch 尺寸观测入口，供 recorder 生命周期测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn offscreen_scratch_surface_size(
        &self,
        handle: &ImageHandle,
    ) -> Option<(i32, i32)> {
        let picture = self.offscreens.get(handle)?;
        Some((
            picture.canvas.scratch.surface().width(),
            picture.canvas.scratch.surface().height(),
        ))
    }
}

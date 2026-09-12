//! `src/draw/raster/software_rasterizer.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl SoftwareRasterizer） ——

impl SoftwareRasterizer {
    #[cfg(test)]
    pub(crate) fn transient_stack_capacities(&self) -> (usize, usize, usize) {
        (
            self.clip_stack.capacity(),
            self.clip_mask_stack.capacity(),
            self.state_stack.capacity(),
        )
    }

    #[cfg(test)]
    pub(crate) fn snapshot_slot_count(&self) -> usize {
        self.state_stack.len()
    }

    #[cfg(test)]
    pub(crate) fn snapshot_clip_stack_allocation(
        &self,
        index: usize,
    ) -> Option<(*const Rect, usize)> {
        self.state_stack
            .get(index)
            .map(|snapshot| (snapshot.clip_stack.as_ptr(), snapshot.clip_stack.capacity()))
    }
}

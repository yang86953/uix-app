//! CPU 像素离屏池 — 供 CPU / PresentUpload 与 API-neutral recorder 的 Picture 缓存。
//!
//! GPU 后端使用各自的 RT/FBO，不得挂接本池。

// 引入 typed error，避免 Picture 像素分配失败被降成普通缺失。
use crate::core::Error;
use crate::draw::Canvas2D;
use crate::draw::backend::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::backend::slot_pool::SlotPool;
use crate::draw::geometry::types::ImageHandle;
use crate::draw::raster::pixel_surface::PixelSurface;

#[derive(Default)]
pub(crate) struct CpuOffscreenPool {
    pool: SlotPool<CpuCanvas2D>,
}

impl CpuOffscreenPool {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn clear(&mut self) {
        self.pool.clear();
    }

    // 以检查式结果创建 CPU Picture；非正尺寸仍是无资源的正常结果。
    pub(crate) fn try_create(
        // 借用离屏池的唯一可变 owner。
        &mut self,
        // 接收 Picture 的逻辑宽度。
        width: i32,
        // 接收 Picture 的逻辑高度。
        height: i32,
        // 区分正常无资源与 typed 分配失败。
    ) -> Result<Option<ImageHandle>, Error> {
        if width <= 0 || height <= 0 {
            // 非正尺寸不创建资源，也不伪造分配错误。
            return Ok(None);
        }
        // 保留 PixelSurface 的 GraphicsOutOfMemory 分类供恢复链消费。
        let surface = PixelSurface::try_new(width, height)?;
        // 只有像素表面和槽位都建立后才发布新 handle。
        let id = self.pool.insert(CpuCanvas2D::new(surface));
        Ok(Some(ImageHandle(id)))
    }

    pub(crate) fn destroy(&mut self, handle: ImageHandle) {
        self.pool.remove(handle.0);
    }

    pub(crate) fn canvas_mut(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.pool
            .get_mut(handle.0)
            .map(|c| c as &mut dyn Canvas2D)
    }

    pub(crate) fn copy_pixels(&self, handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        let canvas = self.pool.get(handle.0)?;
        let surf = canvas.surface();
        Some((surf.pixels().to_vec(), surf.width()))
    }

    /// 将一个离屏表面直接绘制到另一个离屏表面，避免嵌套 Picture 的临时像素拷贝。
    pub(crate) fn blit_into(
        &mut self,
        source: &ImageHandle,
        destination: &ImageHandle,
        src_rect: crate::core::Rect,
        dst_rect: crate::core::Rect,
    ) -> bool {
        if source.0 == destination.0 {
            return false;
        }

        let (source_slot, destination_slot) = self.pool.get_two_mut(source.0, destination.0);
        let Some(source_canvas) = source_slot else {
            return false;
        };
        let Some(destination_canvas) = destination_slot else {
            return false;
        };
        let source_surface = source_canvas.surface();
        destination_canvas.blit_image(
            source_surface.pixels(),
            source_surface.width(),
            src_rect,
            dst_rect,
        );
        true
    }

    pub(crate) fn get(&self, handle: &ImageHandle) -> Option<&CpuCanvas2D> {
        self.pool.get(handle.0)
    }

    pub(crate) fn get_mut(&mut self, handle: &ImageHandle) -> Option<&mut CpuCanvas2D> {
        self.pool.get_mut(handle.0)
    }

    pub(crate) fn memory_usage(&self) -> usize {
        self.pool
            .iter_values()
            .map(|c| c.surface().memory_usage())
            .fold(0usize, usize::saturating_add)
    }

    /// Compact the slot array by truncating trailing `None` entries.
    /// Also removes free IDs that now point beyond the compacted length.
    pub(crate) fn compact(&mut self) {
        self.pool.compact();
    }

    // 测试目标保留槽位数量观测入口，供离屏池压缩测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn slot_len(&self) -> usize {
        self.pool.len()
    }
}

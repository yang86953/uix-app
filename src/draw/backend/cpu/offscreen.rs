//! CPU 像素离屏池 — 供 CPU / PresentUpload 与 API-neutral recorder 的 Picture 缓存。
//!
//! GPU 后端使用各自的 RT/FBO，不得挂接本池。

// 引入 typed error，避免 Picture 像素分配失败被降成普通缺失。
use crate::core::Error;
use crate::draw::Canvas2D;
use crate::draw::backend::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::geometry::types::ImageHandle;
use crate::draw::raster::pixel_surface::PixelSurface;

#[derive(Default)]
pub(crate) struct CpuOffscreenPool {
    offscreens: Vec<Option<CpuCanvas2D>>,
    free_ids: Vec<u32>,
    next_id: u32,
}

impl CpuOffscreenPool {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn clear(&mut self) {
        self.offscreens.clear();
        self.free_ids.clear();
        self.next_id = 0;
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
        let id = if let Some(id) = self.free_ids.pop() {
            id
        } else {
            let id = self.next_id;
            self.next_id = self.next_id.saturating_add(1);
            id
        };
        let idx = id as usize;
        while self.offscreens.len() <= idx {
            self.offscreens.push(None);
        }
        self.offscreens[idx] = Some(CpuCanvas2D::new(surface));
        // 只有像素表面和槽位都建立后才发布新 handle。
        Ok(Some(ImageHandle(id)))
    }

    pub(crate) fn destroy(&mut self, handle: ImageHandle) {
        let idx = handle.0 as usize;
        if idx < self.offscreens.len() && self.offscreens[idx].take().is_some() {
            self.free_ids.push(handle.0);
        }
    }

    pub(crate) fn canvas_mut(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        let idx = handle.0 as usize;
        self.offscreens
            .get_mut(idx)?
            .as_mut()
            .map(|c| c as &mut dyn Canvas2D)
    }

    pub(crate) fn copy_pixels(&self, handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        let idx = handle.0 as usize;
        let canvas = self.offscreens.get(idx)?.as_ref()?;
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
        let source_idx = source.0 as usize;
        let destination_idx = destination.0 as usize;
        if source_idx == destination_idx {
            return false;
        }

        let (source_slot, destination_slot) = if source_idx < destination_idx {
            let (before, after) = self.offscreens.split_at_mut(destination_idx);
            (before.get_mut(source_idx), after.first_mut())
        } else {
            let (before, after) = self.offscreens.split_at_mut(source_idx);
            (after.first_mut(), before.get_mut(destination_idx))
        };
        let Some(source_canvas) = source_slot.and_then(|slot| slot.as_ref()) else {
            return false;
        };
        let Some(destination_canvas) = destination_slot.and_then(|slot| slot.as_mut()) else {
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
        let idx = handle.0 as usize;
        self.offscreens.get(idx)?.as_ref()
    }

    pub(crate) fn get_mut(&mut self, handle: &ImageHandle) -> Option<&mut CpuCanvas2D> {
        let idx = handle.0 as usize;
        self.offscreens.get_mut(idx)?.as_mut()
    }

    pub(crate) fn memory_usage(&self) -> usize {
        self.offscreens
            .iter()
            .filter_map(|o| {
                o.as_ref().map(|c| {
                    let s = c.surface();
                    s.memory_usage()
                })
            })
            .fold(0usize, usize::saturating_add)
    }

    /// Compact the slot array by truncating trailing `None` entries.
    /// Also removes free IDs that now point beyond the compacted length.
    pub(crate) fn compact(&mut self) {
        while self.offscreens.last().is_some_and(|s| s.is_none()) {
            self.offscreens.pop();
        }
        // Recompute next_id as max(used_id) + 1 to avoid gaps.
        // Retain free_ids that still point within bounds.
        self.free_ids
            .retain(|id| (*id as usize) < self.offscreens.len());
        self.next_id = self.offscreens.len() as u32;
    }

    // 测试目标保留槽位数量观测入口，供离屏池压缩测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn slot_len(&self) -> usize {
        self.offscreens.len()
    }
}

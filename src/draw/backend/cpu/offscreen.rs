//! CPU 像素离屏池 — 供 CPU / PresentUpload 与 API-neutral recorder 的 Picture 缓存。
//!
//! GPU 后端使用各自的 RT/FBO，不得挂接本池。

use crate::draw::backend::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::geometry::types::ImageHandle;
use crate::draw::Canvas2D;

#[derive(Default)]
pub struct CpuOffscreenPool {
    offscreens: Vec<Option<CpuCanvas2D>>,
    free_ids: Vec<u32>,
    next_id: u32,
}

impl CpuOffscreenPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.offscreens.clear();
        self.free_ids.clear();
        self.next_id = 0;
    }

    pub fn create(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let surface = PixelSurface::try_new(width, height).ok()?;
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
        Some(ImageHandle(id))
    }

    pub fn destroy(&mut self, handle: ImageHandle) {
        let idx = handle.0 as usize;
        if idx < self.offscreens.len() && self.offscreens[idx].take().is_some() {
            self.free_ids.push(handle.0);
        }
    }

    pub fn canvas_mut(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        let idx = handle.0 as usize;
        self.offscreens
            .get_mut(idx)?
            .as_mut()
            .map(|c| c as &mut dyn Canvas2D)
    }

    pub fn copy_pixels(&self, handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        let idx = handle.0 as usize;
        let canvas = self.offscreens.get(idx)?.as_ref()?;
        let surf = canvas.surface();
        Some((surf.pixels().to_vec(), surf.width()))
    }

    pub fn get(&self, handle: &ImageHandle) -> Option<&CpuCanvas2D> {
        let idx = handle.0 as usize;
        self.offscreens.get(idx)?.as_ref()
    }

    pub fn get_mut(&mut self, handle: &ImageHandle) -> Option<&mut CpuCanvas2D> {
        let idx = handle.0 as usize;
        self.offscreens.get_mut(idx)?.as_mut()
    }

    pub fn memory_usage(&self) -> usize {
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
    pub fn compact(&mut self) {
        while self.offscreens.last().is_some_and(|s| s.is_none()) {
            self.offscreens.pop();
        }
        // Recompute next_id as max(used_id) + 1 to avoid gaps.
        // Retain free_ids that still point within bounds.
        self.free_ids
            .retain(|id| (*id as usize) < self.offscreens.len());
        self.next_id = self.offscreens.len() as u32;
    }

    #[cfg(test)]
    pub(crate) fn slot_len(&self) -> usize {
        self.offscreens.len()
    }
}

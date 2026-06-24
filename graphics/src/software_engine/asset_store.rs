use super::{ImageData, ImageSlot, OffscreenData, OffscreenSlot};
use uix_diag::{Errc, Error};
use crate::bitmap_font::BitmapFont;
use crate::types::HandleKind;
use crate::ImageHandle;

/// Owns graphical resources (images, offscreen buffers, bitmap font).
///
/// Fonts are now managed by the `TextBackend` trait — see `FontdueBackend`.
pub struct AssetStore {
    bitmap_font: BitmapFont,
    image_slots: Vec<ImageSlot>,
    image_free: Vec<u32>,
    offscreen_slots: Vec<OffscreenSlot>,
    offscreen_free: Vec<u32>,
}

impl Default for AssetStore {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetStore {
    pub fn new() -> Self {
        Self {
            bitmap_font: BitmapFont::new(),
            image_slots: Vec::new(),
            image_free: Vec::new(),
            offscreen_slots: Vec::new(),
            offscreen_free: Vec::new(),
        }
    }

    pub fn shutdown(&mut self) {
        self.image_slots.clear();
        self.image_free.clear();
        self.offscreen_slots.clear();
        self.offscreen_free.clear();
    }

    pub fn bitmap_font(&self) -> &BitmapFont {
        &self.bitmap_font
    }

    // ── 图片 ──

    /// 分配一个新的图片插槽。返回可变句柄引用。
    pub fn alloc_image_slot(
        &mut self,
        pixels: Vec<u32>,
        w: i32,
        h: i32,
    ) -> &mut ImageHandle {
        let idx = if let Some(free) = self.image_free.pop() {
            // 复用空闲插槽
            self.image_slots[free as usize] = ImageSlot {
                handle: ImageHandle::new(free, HandleKind::Image),
                data: ImageData { pixels, w, h },
            };
            free
        } else {
            let idx = self.image_slots.len() as u32;
            self.image_slots.push(ImageSlot {
                handle: ImageHandle::new(idx, HandleKind::Image),
                data: ImageData { pixels, w, h },
            });
            idx
        };
        &mut self.image_slots[idx as usize].handle
    }

    /// 兼容旧 API 的 load_image（委托给 alloc_image_slot）。
    pub fn load_image(&mut self, pixels: Vec<u32>, w: i32, h: i32) -> &mut ImageHandle {
        self.alloc_image_slot(pixels, w, h)
    }

    pub(crate) fn find_image(&self, handle: &ImageHandle) -> Option<(&[u32], i32, i32)> {
        if handle.kind != HandleKind::Image {
            return None;
        }
        let idx = handle.index as usize;
        self.image_slots
            .get(idx)
            .map(|s| (s.data.pixels.as_slice(), s.data.w, s.data.h))
    }

    /// 移除图片（释放内存，插槽可复用）。
    pub fn remove_image(&mut self, handle: &ImageHandle) {
        if handle.kind != HandleKind::Image {
            return;
        }
        let idx = handle.index as usize;
        if idx < self.image_slots.len() {
            self.image_slots[idx].data.pixels.clear();
            self.image_slots[idx].data.pixels.shrink_to_fit();
            self.image_free.push(handle.index);
        }
    }

    pub(crate) fn find_any_size(&self, handle: &ImageHandle) -> Option<(i32, i32)> {
        match handle.kind {
            HandleKind::Image => {
                let idx = handle.index as usize;
                self.image_slots.get(idx).map(|s| (s.data.w, s.data.h))
            }
            HandleKind::Offscreen => {
                let idx = handle.index as usize;
                self.offscreen_slots.get(idx).map(|s| (s.data.w, s.data.h))
            }
        }
    }

    // ── 离屏缓冲 ──

    pub fn create_offscreen(&mut self, w: i32, h: i32) -> Result<&mut ImageHandle, Error> {
        if w <= 0 || h <= 0 {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("invalid offscreen size: {}x{}", w, h),
            ));
        }
        // 用 0 初始化（透明），渲染层会正确覆盖。避免无意义的预填充。
        let idx = self.offscreen_slots.len() as u32;
        let pixels = vec![0x00000000; (w * h) as usize];
        self.offscreen_slots.push(OffscreenSlot {
            handle: ImageHandle::new(idx, HandleKind::Offscreen),
            data: OffscreenData { pixels, w, h },
        });
        Ok(&mut self.offscreen_slots[idx as usize].handle)
    }

    pub(crate) fn find_offscreen(&self, handle: &ImageHandle) -> Option<(&[u32], i32, i32)> {
        if handle.kind != HandleKind::Offscreen {
            return None;
        }
        let idx = handle.index as usize;
        self.offscreen_slots
            .get(idx)
            .map(|s| (s.data.pixels.as_slice(), s.data.w, s.data.h))
    }

    pub(crate) fn offscreen_slot_index(&self, handle: &ImageHandle) -> Option<usize> {
        if handle.kind != HandleKind::Offscreen {
            return None;
        }
        Some(handle.index as usize)
    }

    /// 移除离屏缓冲（释放内存，插槽可复用）。
    pub fn remove_offscreen(&mut self, handle: &ImageHandle) {
        if handle.kind != HandleKind::Offscreen {
            return;
        }
        let idx = handle.index as usize;
        if idx < self.offscreen_slots.len() {
            self.offscreen_slots[idx].data.pixels.clear();
            self.offscreen_slots[idx].data.pixels.shrink_to_fit();
            self.offscreen_free.push(handle.index);
        }
    }

    /// 返回图片插槽的总字节数。
    pub fn image_bytes(&self) -> usize {
        self.image_slots.iter().map(|s| s.data.pixels.capacity() * 4).sum()
    }
    /// 返回离屏插槽的总字节数。
    pub fn offscreen_bytes(&self) -> usize {
        self.offscreen_slots.iter().map(|s| s.data.pixels.capacity() * 4).sum()
    }
    /// 返回离屏插槽数量。
    pub fn offscreen_count(&self) -> usize {
        self.offscreen_slots.len()
    }
    /// 返回图片插槽数量。
    pub fn image_count(&self) -> usize {
        self.image_slots.len()
    }

    /// 交换离屏槽与外部像素缓冲（零拷贝，代替旧 take/return 模式）。
    pub fn swap_offscreen_pixels(
        &mut self,
        idx: usize,
        pixels: &mut Vec<u32>,
        w: &mut i32,
        h: &mut i32,
    ) {
        let slot = &mut self.offscreen_slots[idx];
        std::mem::swap(pixels, &mut slot.data.pixels);
        std::mem::swap(w, &mut slot.data.w);
        std::mem::swap(h, &mut slot.data.h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_handle(store: &mut AssetStore) -> ImageHandle {
        let h = store.load_image(vec![0xFFFF0000u32; 25], 5, 5);
        ImageHandle::new(h.index, h.kind)
    }

    #[test]
    fn load_image_increments_index() {
        let mut store = AssetStore::new();
        let h1 = make_handle(&mut store);
        let h2 = make_handle(&mut store);
        assert_eq!(h1.index, 0);
        assert_eq!(h2.index, 1);
    }

    #[test]
    fn find_image_none_for_unknown() {
        let store = AssetStore::new();
        let h = ImageHandle::new(999, HandleKind::Image);
        assert!(store.find_image(&h).is_none());
    }

    #[test]
    fn find_image_returns_data_after_load() {
        let mut store = AssetStore::new();
        let h = make_handle(&mut store);
        let found = store.find_image(&h);
        assert!(found.is_some());
        let (data, w, h_val) = found.unwrap();
        assert_eq!(w, 5);
        assert_eq!(h_val, 5);
        assert_eq!(data.len(), 25);
    }

    #[test]
    fn find_image_returns_none_for_wrong_kind() {
        let mut store = AssetStore::new();
        let _ = store.create_offscreen(10, 10);
        let h = ImageHandle::new(0, HandleKind::Offscreen);
        assert!(store.find_image(&h).is_none());
    }

    #[test]
    fn create_offscreen_rejects_invalid_size() {
        let mut store = AssetStore::new();
        assert!(store.create_offscreen(0, 100).is_err());
        assert!(store.create_offscreen(100, -1).is_err());
    }

    #[test]
    fn create_offscreen_initial_black() {
        let mut store = AssetStore::new();
        let _h = store.create_offscreen(2, 2).expect("offscreen ok");
        let h = ImageHandle::new(0, HandleKind::Offscreen);
        let (data, _w, _h) = store.find_offscreen(&h).unwrap();
        assert!(data.iter().all(|&p| p == 0x00000000));
    }

    #[test]
    fn offscreen_swap_cycle() {
        let mut store = AssetStore::new();
        let _ = store.create_offscreen(2, 2).expect("offscreen ok");
        // 模拟 RenderTarget 的数据
        let mut pixels = vec![0xFFFF0000u32; 16]; // 4x4 全红
        let mut w = 4i32;
        let mut h = 4i32;
        // swap offscreen ↔ pixels
        store.swap_offscreen_pixels(0, &mut pixels, &mut w, &mut h);
        // pixels 现在有 offscreen 数据（全黑），且 w/h 变成 2x2
        assert!(pixels.iter().all(|&p| p == 0x00000000));
        assert_eq!((w, h), (2, 2));
        // offscreen 槽有原数据（全红 4x4）
        let off_h = ImageHandle::new(0, HandleKind::Offscreen);
        let (data, dw, dh) = store.find_offscreen(&off_h).unwrap();
        assert!(data.iter().all(|&p| p == 0xFFFF0000));
        assert_eq!((dw, dh), (4, 4));
        // swap 回来
        store.swap_offscreen_pixels(0, &mut pixels, &mut w, &mut h);
        assert_eq!(pixels[0], 0xFFFF0000);
        assert_eq!((w, h), (4, 4));
    }

    #[test]
    fn shutdown_clears_all() {
        let mut store = AssetStore::new();
        let _ = make_handle(&mut store);
        let _ = store.create_offscreen(32, 32);
        store.shutdown();
        let h = ImageHandle::new(0, HandleKind::Offscreen);
        assert!(store.find_offscreen(&h).is_none());
    }

    #[test]
    fn bitmap_font_default_exists() {
        let store = AssetStore::new();
        let _bf = store.bitmap_font();
    }

    #[test]
    fn find_any_size_image() {
        let mut store = AssetStore::new();
        let h = make_handle(&mut store);
        let size = store.find_any_size(&h);
        assert_eq!(size, Some((5, 5)));
    }

    #[test]
    fn offscreen_slot_index() {
        let mut store = AssetStore::new();
        let _ = store.create_offscreen(10, 10).expect("ok");
        let h = ImageHandle::new(0, HandleKind::Offscreen);
        assert_eq!(store.offscreen_slot_index(&h), Some(0));
    }
}

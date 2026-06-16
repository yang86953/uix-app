use super::{ImageData, ImageSlot, OffscreenData, OffscreenSlot};
use crate::diag::{Errc, Error};
use crate::graphics::bitmap_font::BitmapFont;
use crate::graphics::types::HandleKind;
use crate::graphics::ImageHandle;

/// Owns graphical resources (images, offscreen buffers, bitmap font).
///
/// Fonts are now managed by the `TextBackend` trait — see `FontdueBackend`.
pub struct AssetStore {
    bitmap_font: BitmapFont,
    image_slots: Vec<ImageSlot>,
    offscreen_slots: Vec<OffscreenSlot>,
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
            offscreen_slots: Vec::new(),
        }
    }

    pub fn shutdown(&mut self) {
        self.image_slots.clear();
        self.offscreen_slots.clear();
    }

    pub fn bitmap_font(&self) -> &BitmapFont {
        &self.bitmap_font
    }

    // ── 图片 ──

    pub fn load_image(&mut self, pixels: Vec<u32>, w: i32, h: i32) -> &mut ImageHandle {
        let idx = self.image_slots.len() as u32;
        self.image_slots.push(ImageSlot {
            handle: ImageHandle::new(idx, HandleKind::Image),
            data: ImageData { pixels, w, h },
        });
        &mut self.image_slots[idx as usize].handle
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
        let idx = self.offscreen_slots.len() as u32;
        let pixels = vec![0xFF000000; (w * h) as usize];
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

    pub fn take_offscreen_pixels(&mut self, idx: usize) -> (Vec<u32>, i32, i32) {
        let slot = &mut self.offscreen_slots[idx];
        let w = slot.data.w;
        let h = slot.data.h;
        let pixels = std::mem::take(&mut slot.data.pixels);
        (pixels, w, h)
    }

    pub fn return_offscreen_pixels(&mut self, idx: usize, pixels: Vec<u32>) {
        self.offscreen_slots[idx].data.pixels = pixels;
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
        let _ = store.create_offscreen(2, 2).expect("offscreen ok");
        let (data, _w, _h) = store.take_offscreen_pixels(0);
        assert!(data.iter().all(|&p| p == 0xFF000000));
    }

    #[test]
    fn offscreen_take_and_return_cycle() {
        let mut store = AssetStore::new();
        let _ = store.create_offscreen(2, 2).expect("offscreen ok");
        let (pixels, w, h) = store.take_offscreen_pixels(0);
        assert_eq!((w, h), (2, 2));
        assert_eq!(pixels.len(), 4);
        let new_pixels = vec![0xFFFFFFFF; 4];
        store.return_offscreen_pixels(0, new_pixels);
        let (returned, _, _) = store.take_offscreen_pixels(0);
        assert_eq!(returned[0], 0xFFFFFFFF);
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

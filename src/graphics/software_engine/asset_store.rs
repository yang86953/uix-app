use super::{FontData, FontSlot, ImageData, ImageSlot, OffscreenData, OffscreenSlot};
use crate::diag::{Errc, Error};
use crate::graphics::bitmap_font::BitmapFont;
use crate::graphics::{FontHandle, HandleKind, ImageHandle};

/// Owns all graphical resources (fonts, images, offscreen buffers).
pub struct AssetStore {
    bitmap_font: BitmapFont,
    font_slots: Vec<FontSlot>,
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
            font_slots: Vec::new(),
            image_slots: Vec::new(),
            offscreen_slots: Vec::new(),
        }
    }

    pub fn shutdown(&mut self) {
        self.font_slots.clear();
        self.image_slots.clear();
        self.offscreen_slots.clear();
    }

    // ── 字体 ──

    pub fn load_font(&mut self, bytes: Vec<u8>, _size: f32) -> Result<&mut FontHandle, Error> {
        let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
            .map_err(|e| Error::new(Errc::FormatError, format!("invalid font: {}", e)))?;
        let idx = self.font_slots.len() as u32;
        self.font_slots.push(FontSlot {
            handle: FontHandle::new(idx),
            data: FontData { font },
        });
        Ok(&mut self.font_slots[idx as usize].handle)
    }

    pub(crate) fn find_font(&self, handle: &FontHandle) -> Option<&FontData> {
        let idx = handle.0 as usize;
        self.font_slots.get(idx).map(|s| &s.data)
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

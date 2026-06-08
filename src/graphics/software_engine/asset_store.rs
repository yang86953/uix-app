use super::{FontData, FontSlot, ImageData, ImageSlot, OffscreenData, OffscreenSlot};
use crate::diag::{Errc, Error};
use crate::graphics::{FontHandle, ImageHandle};
use crate::graphics::bitmap_font::BitmapFont;

/// Owns all graphical resources (fonts, images, offscreen buffers).
pub struct AssetStore {
    bitmap_font: BitmapFont,
    font_slots: Vec<Box<FontSlot>>,
    image_slots: Vec<Box<ImageSlot>>,
    offscreen_slots: Vec<Box<OffscreenSlot>>,
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

    pub fn load_font(&mut self, bytes: Vec<u8>, size: f32) -> Result<&mut FontHandle, Error> {
        let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
            .map_err(|e| Error::new(Errc::FormatError, format!("invalid font: {}", e)))?;
        let slot = Box::new(FontSlot {
            handle: Box::new(FontHandle::default()),
            data: FontData { font, size },
        });
        self.font_slots.push(slot);
        Ok(self.font_slots.last_mut().unwrap().handle.as_mut())
    }

    pub(crate) fn find_font(&self, handle: &FontHandle) -> Option<&FontData> {
        let ptr = handle as *const FontHandle;
        self.font_slots.iter()
            .find(|s| s.handle.as_ref() as *const FontHandle == ptr)
            .map(|s| &s.data)
    }

    pub fn bitmap_font(&self) -> &BitmapFont {
        &self.bitmap_font
    }

    // ── 图片 ──

    pub fn load_image(&mut self, pixels: Vec<u32>, w: i32, h: i32) -> &mut ImageHandle {
        let slot = Box::new(ImageSlot {
            handle: Box::new(ImageHandle),
            data: ImageData { pixels, w, h },
        });
        self.image_slots.push(slot);
        self.image_slots.last_mut().unwrap().handle.as_mut()
    }

    pub fn find_image(&self, handle: &ImageHandle) -> Option<(&[u32], i32, i32)> {
        let ptr = handle as *const ImageHandle;
        self.image_slots.iter()
            .find(|s| s.handle.as_ref() as *const ImageHandle == ptr)
            .map(|s| (s.data.pixels.as_slice(), s.data.w, s.data.h))
    }

    pub fn find_any_size(&self, handle: &ImageHandle) -> Option<(i32, i32)> {
        let ptr = handle as *const ImageHandle;
        if let Some(s) = self.image_slots.iter()
            .find(|s| s.handle.as_ref() as *const ImageHandle == ptr)
        {
            return Some((s.data.w, s.data.h));
        }
        self.offscreen_slots.iter()
            .find(|s| s.handle.as_ref() as *const ImageHandle == ptr)
            .map(|s| (s.data.w, s.data.h))
    }

    // ── 离屏缓冲 ──

    pub fn create_offscreen(&mut self, w: i32, h: i32) -> Result<&mut ImageHandle, Error> {
        if w <= 0 || h <= 0 {
            return Err(Error::new(Errc::InvalidArgument,
                format!("invalid offscreen size: {}x{}", w, h)));
        }
        let pixels = vec![0xFF000000; (w * h) as usize];
        let slot = Box::new(OffscreenSlot {
            handle: Box::new(ImageHandle),
            data: OffscreenData { pixels, w, h },
        });
        self.offscreen_slots.push(slot);
        Ok(self.offscreen_slots.last_mut().unwrap().handle.as_mut())
    }

    pub fn find_offscreen(&self, handle: &ImageHandle) -> Option<(&[u32], i32, i32)> {
        let ptr = handle as *const ImageHandle;
        self.offscreen_slots.iter()
            .find(|s| s.handle.as_ref() as *const ImageHandle == ptr)
            .map(|s| (s.data.pixels.as_slice(), s.data.w, s.data.h))
    }

    pub fn offscreen_slot_index(&self, handle: &ImageHandle) -> Option<usize> {
        let ptr = handle as *const ImageHandle;
        self.offscreen_slots.iter()
            .position(|s| s.handle.as_ref() as *const ImageHandle == ptr)
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

//! 图片加载与缓存 — 独立于渲染引擎的位图资源管理。
//!
//! 职责：解码 PNG/JPEG 等格式、路径缓存、按句柄提供像素数据。
//! 渲染通过 `PaintContext::draw_image` 调用 `Canvas2D::blit_image`。

pub(crate) mod decode;

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::core::error::Error;
use crate::core::Rect;
use crate::draw::traits::Canvas2D;

pub use decode::decode_to_pixels;

/// 解码位图句柄（区别于离屏缓冲 `ImageHandle`）。
/// 低 32 位为槽位索引，高 32 位为 generation，防止 unload/reuse 后旧句柄静默指向新图片。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BitmapHandle(pub u64);

impl BitmapHandle {
    pub const INVALID: Self = Self(u64::MAX);

    pub fn is_valid(self) -> bool {
        self.0 != u64::MAX
    }

    fn slot_index(self) -> usize {
        (self.0 & 0xFFFF_FFFF) as usize
    }

    fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }

    fn pack(index: u32, generation: u32) -> Self {
        Self((generation as u64) << 32 | index as u64)
    }
}

/// 引擎内单个解码位图槽位。
#[derive(Clone, Default)]
pub struct ImageSlot {
    width: i32,
    height: i32,
    pixels: Vec<u32>,
    path: Option<String>,
    valid: bool,
    generation: u32,
}

impl ImageSlot {
    fn from_decoded(w: i32, h: i32, pixels: Vec<u32>, path: Option<String>) -> Self {
        Self {
            width: w,
            height: h,
            pixels,
            path,
            valid: true,
            generation: 0,
        }
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }
}

/// 图片服务 — 解码缓存与路径索引。
pub struct ImageService {
    slots: RefCell<Vec<ImageSlot>>,
    pub(crate) path_cache: RefCell<HashMap<String, BitmapHandle>>,
}

impl Default for ImageService {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageService {
    pub fn new() -> Self {
        Self {
            slots: RefCell::new(Vec::new()),
            path_cache: RefCell::new(HashMap::new()),
        }
    }

    /// 从原始字节加载图片，返回新句柄。
    pub fn load_from_bytes(&self, data: &[u8]) -> Result<BitmapHandle, Error> {
        let (w, h, pixels) = decode_to_pixels(data)?;
        Ok(self.insert_slot(ImageSlot::from_decoded(w, h, pixels, None)))
    }

    /// 从文件路径加载图片（带路径缓存）。
    pub fn load_from_path(&self, path: impl AsRef<Path>) -> Result<BitmapHandle, Error> {
        let path_str = path.as_ref().to_string_lossy().into_owned();
        if let Some(handle) = self.path_cache.borrow().get(&path_str).copied() {
            if self.is_valid(handle) {
                return Ok(handle);
            }
        }
        let data = fs::read(&path_str)
            .map_err(|e| Error::io_error(format!("读取图片文件 '{path_str}' 失败: {e}")))?;
        let (w, h, pixels) = decode_to_pixels(&data)?;
        let handle = self.insert_slot(ImageSlot::from_decoded(
            w,
            h,
            pixels,
            Some(path_str.clone()),
        ));
        self.path_cache.borrow_mut().insert(path_str, handle);
        Ok(handle)
    }

    /// 懒加载：路径为空或加载失败返回 `None`。
    pub fn ensure_loaded(&self, path: &str) -> Option<BitmapHandle> {
        if path.is_empty() {
            return None;
        }
        self.load_from_path(path).ok()
    }

    /// 检查句柄是否有效（generation 也须匹配）。
    pub fn is_valid(&self, handle: BitmapHandle) -> bool {
        let idx = handle.slot_index();
        self.slots.borrow().get(idx).is_some_and(|s| s.valid && s.generation == handle.generation())
    }

    /// 读取槽位像素（只读借用）。
    pub fn with_slot<R>(&self, handle: BitmapHandle, f: impl FnOnce(&ImageSlot) -> R) -> Option<R> {
        let slots = self.slots.borrow();
        let idx = handle.slot_index();
        slots.get(idx).filter(|s| s.valid && s.generation == handle.generation()).map(f)
    }

    /// 卸载位图并清除路径缓存引用。
    pub fn unload(&self, handle: BitmapHandle) {
        let mut slots = self.slots.borrow_mut();
        let idx = handle.slot_index();
        if let Some(slot) = slots.get_mut(idx)
            .filter(|s| s.generation == handle.generation())
        {
            if let Some(ref path) = slot.path {
                self.path_cache.borrow_mut().remove(path);
            }
            *slot = ImageSlot::default();
        }
    }

    /// 近似内存占用（字节）。
    pub fn memory_usage(&self) -> usize {
        self.slots
            .borrow()
            .iter()
            .filter(|s| s.valid)
            .map(|s| s.pixels.len() * 4 + std::mem::size_of::<ImageSlot>())
            .sum()
    }

    fn insert_slot(&self, mut slot: ImageSlot) -> BitmapHandle {
        let mut slots = self.slots.borrow_mut();
        // 复用已释放槽位，递增 generation 使旧句柄失效
        for (i, s) in slots.iter_mut().enumerate() {
            if !s.valid {
                slot.generation = s.generation.wrapping_add(1);
                *s = slot;
                return BitmapHandle::pack(i as u32, s.generation);
            }
        }
        let generation = 0;
        slot.generation = generation;
        let idx = slots.len();
        slots.push(slot);
        BitmapHandle::pack(idx as u32, generation)
    }
}

/// 将解码位图 blit 到 canvas（`fit=true` 时保持宽高比居中）。
pub fn blit_handle(
    service: &ImageService,
    canvas: &mut dyn Canvas2D,
    handle: BitmapHandle,
    bounds: Rect,
    fit: bool,
) {
    service.with_slot(handle, |slot| {
        let w = slot.width();
        let h = slot.height();
        let src = Rect::new(0.0, 0.0, w as f32, h as f32);
        let dst = if fit {
            fit_dst_rect(w, h, bounds)
        } else {
            bounds
        };
        canvas.blit_image(slot.pixels(), w, src, dst);
    });
}

/// 将位图绘制到目标矩形（保持宽高比，居中 fit）。
pub fn fit_dst_rect(src_w: i32, src_h: i32, bounds: Rect) -> Rect {
    if src_w <= 0 || src_h <= 0 || bounds.w <= 0.0 || bounds.h <= 0.0 {
        return bounds;
    }
    let src_aspect = src_w as f32 / src_h as f32;
    let dst_aspect = bounds.w / bounds.h;
    if src_aspect > dst_aspect {
        let h = bounds.w / src_aspect;
        Rect::new(bounds.x, bounds.y + (bounds.h - h) * 0.5, bounds.w, h)
    } else {
        let w = bounds.h * src_aspect;
        Rect::new(bounds.x + (bounds.w - w) * 0.5, bounds.y, w, bounds.h)
    }
}


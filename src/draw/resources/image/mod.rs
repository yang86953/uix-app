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
use crate::draw::Canvas2D;

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

type RoundedRectCache = HashMap<(BitmapHandle, u32, u32, u32, bool), BitmapHandle>;

/// 图片服务 — 解码缓存与路径索引。
pub struct ImageService {
    slots: RefCell<Vec<ImageSlot>>,
    slot_generations: RefCell<Vec<u32>>,
    pub(crate) path_cache: RefCell<HashMap<String, BitmapHandle>>,
    square_cache: RefCell<HashMap<BitmapHandle, BitmapHandle>>,
    circular_cache: RefCell<HashMap<(BitmapHandle, u32), BitmapHandle>>,
    rounded_square_cache: RefCell<HashMap<(BitmapHandle, u32, u32), BitmapHandle>>,
    rounded_rect_cache: RefCell<RoundedRectCache>,
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
            slot_generations: RefCell::new(Vec::new()),
            path_cache: RefCell::new(HashMap::new()),
            square_cache: RefCell::new(HashMap::new()),
            circular_cache: RefCell::new(HashMap::new()),
            rounded_square_cache: RefCell::new(HashMap::new()),
            rounded_rect_cache: RefCell::new(HashMap::new()),
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
        self.slots
            .borrow()
            .get(idx)
            .is_some_and(|s| s.valid && s.generation == handle.generation())
    }

    /// 读取槽位像素（只读借用）。
    pub fn with_slot<R>(&self, handle: BitmapHandle, f: impl FnOnce(&ImageSlot) -> R) -> Option<R> {
        let slots = self.slots.borrow();
        let idx = handle.slot_index();
        slots
            .get(idx)
            .filter(|s| s.valid && s.generation == handle.generation())
            .map(f)
    }

    /// 返回源位图的居中正方形裁切，结果按源句柄缓存。
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn square_crop(&self, handle: BitmapHandle) -> Option<BitmapHandle> {
        let cached = self.square_cache.borrow().get(&handle).copied();
        if let Some(cached) = cached {
            if self.is_valid(cached) {
                return Some(cached);
            }
            self.square_cache.borrow_mut().remove(&handle);
        }

        let (side, pixels) = self.centered_square_pixels(handle)?;
        let cropped = self.insert_slot(ImageSlot::from_decoded(side, side, pixels, None));
        self.square_cache.borrow_mut().insert(handle, cropped);
        Some(cropped)
    }

    /// 返回源位图的居中正方形圆形裁切；透明角已预乘，结果按源句柄缓存。
    pub fn circular_crop(&self, handle: BitmapHandle) -> Option<BitmapHandle> {
        let source_side = self
            .with_slot(handle, |slot| slot.width.min(slot.height).max(0) as u32)
            .filter(|side| *side > 0)?;
        self.circular_crop_sized(handle, source_side)
    }

    /// 按目标物理像素生成居中裁切的圆形派生图，避免低分辨率源图放大后遮罩失真。
    pub(crate) fn circular_crop_sized(
        &self,
        handle: BitmapHandle,
        target_side: u32,
    ) -> Option<BitmapHandle> {
        let target_side = target_side.clamp(1, 4096);
        let key = (handle, target_side);
        let cached = self.circular_cache.borrow().get(&key).copied();
        if let Some(cached) = cached {
            if self.is_valid(cached) {
                return Some(cached);
            }
            self.circular_cache.borrow_mut().remove(&key);
        }

        let (side, mut pixels) = self.resized_center_square_pixels(handle, target_side)?;

        let radius = side as f32 * 0.5;
        let center = radius;
        for y in 0..side as usize {
            for x in 0..side as usize {
                let dx = x as f32 + 0.5 - center;
                let dy = y as f32 + 0.5 - center;
                let distance = (dx * dx + dy * dy).sqrt();
                let coverage = (radius + 0.5 - distance).clamp(0.0, 1.0);
                let pixel = &mut pixels[y * side as usize + x];
                *pixel = apply_pixel_coverage(*pixel, coverage);
            }
        }

        let cropped = self.insert_slot(ImageSlot::from_decoded(side, side, pixels, None));
        self.circular_cache.borrow_mut().insert(key, cropped);
        Some(cropped)
    }

    /// 按目标物理像素生成带圆角遮罩的居中正方形派生图。
    pub(crate) fn rounded_square_crop_sized(
        &self,
        handle: BitmapHandle,
        target_side: u32,
        corner_radius: f32,
    ) -> Option<BitmapHandle> {
        let target_side = target_side.clamp(1, 4096);
        let corner_radius = if corner_radius.is_finite() {
            corner_radius.clamp(0.0, target_side as f32 * 0.5)
        } else {
            0.0
        };
        let radius_key = (corner_radius * 64.0).round() as u32;
        let key = (handle, target_side, radius_key);
        let cached = self.rounded_square_cache.borrow().get(&key).copied();
        if let Some(cached) = cached {
            if self.is_valid(cached) {
                return Some(cached);
            }
            self.rounded_square_cache.borrow_mut().remove(&key);
        }

        let (side, mut pixels) = self.resized_center_square_pixels(handle, target_side)?;
        let radius = radius_key as f32 / 64.0;
        if radius > 0.0 {
            apply_rounded_rect_mask(&mut pixels, side as usize, side as usize, radius);
        }

        let cropped = self.insert_slot(ImageSlot::from_decoded(side, side, pixels, None));
        self.rounded_square_cache.borrow_mut().insert(key, cropped);
        Some(cropped)
    }

    /// 按目标物理像素生成带圆角遮罩的矩形派生图，并保留 fit / stretch 契约。
    pub(crate) fn rounded_rect_sized(
        &self,
        handle: BitmapHandle,
        target_width: u32,
        target_height: u32,
        corner_radius: f32,
        fit: bool,
    ) -> Option<BitmapHandle> {
        let target_width = target_width.clamp(1, 4096);
        let target_height = target_height.clamp(1, 4096);
        let corner_radius = if corner_radius.is_finite() {
            corner_radius.clamp(0.0, target_width.min(target_height) as f32 * 0.5)
        } else {
            0.0
        };
        let radius_key = (corner_radius * 64.0).round() as u32;
        let key = (handle, target_width, target_height, radius_key, fit);
        let cached = self.rounded_rect_cache.borrow().get(&key).copied();
        if let Some(cached) = cached {
            if self.is_valid(cached) {
                return Some(cached);
            }
            self.rounded_rect_cache.borrow_mut().remove(&key);
        }

        let mut pixels = self.resized_rect_pixels(handle, target_width, target_height, fit)?;
        let radius = radius_key as f32 / 64.0;
        if radius > 0.0 {
            apply_rounded_rect_mask(
                &mut pixels,
                target_width as usize,
                target_height as usize,
                radius,
            );
        }

        let derived = self.insert_slot(ImageSlot::from_decoded(
            target_width as i32,
            target_height as i32,
            pixels,
            None,
        ));
        self.rounded_rect_cache.borrow_mut().insert(key, derived);
        Some(derived)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn centered_square_pixels(&self, handle: BitmapHandle) -> Option<(i32, Vec<u32>)> {
        self.with_slot(handle, |slot| {
            let side = slot.width.min(slot.height).max(0);
            let offset_x = (slot.width - side) / 2;
            let offset_y = (slot.height - side) / 2;
            let side_usize = side as usize;
            let source_width = slot.width as usize;
            let mut pixels = Vec::with_capacity(side_usize.saturating_mul(side_usize));
            for y in 0..side_usize {
                let source_start = (offset_y as usize + y) * source_width + offset_x as usize;
                let source_end = source_start + side_usize;
                pixels.extend_from_slice(&slot.pixels[source_start..source_end]);
            }
            (side, pixels)
        })
        .filter(|(side, _)| *side > 0)
    }

    fn resized_center_square_pixels(
        &self,
        handle: BitmapHandle,
        target_side: u32,
    ) -> Option<(i32, Vec<u32>)> {
        let target_side = i32::try_from(target_side).ok()?.max(1);
        self.with_slot(handle, |slot| {
            let source_side = slot.width.min(slot.height).max(0);
            if source_side <= 0 {
                return None;
            }
            let offset_x = (slot.width - source_side) / 2;
            let offset_y = (slot.height - source_side) / 2;
            let target = target_side as usize;
            let source_side = source_side as usize;
            let source_width = slot.width as usize;
            let mut pixels = Vec::with_capacity(target.saturating_mul(target));
            for y in 0..target {
                let source_y = offset_y as usize + y * source_side / target;
                for x in 0..target {
                    let source_x = offset_x as usize + x * source_side / target;
                    pixels.push(slot.pixels[source_y * source_width + source_x]);
                }
            }
            Some((target_side, pixels))
        })?
    }

    fn resized_rect_pixels(
        &self,
        handle: BitmapHandle,
        target_width: u32,
        target_height: u32,
        fit: bool,
    ) -> Option<Vec<u32>> {
        self.with_slot(handle, |slot| {
            if slot.width <= 0 || slot.height <= 0 {
                return None;
            }
            let target_width = target_width as usize;
            let target_height = target_height as usize;
            let pixel_count = target_width.checked_mul(target_height)?;
            let mut pixels = Vec::new();
            pixels.try_reserve_exact(pixel_count).ok()?;
            pixels.resize(pixel_count, 0);
            let (draw_width, draw_height) = if fit {
                let scale = (target_width as f32 / slot.width as f32)
                    .min(target_height as f32 / slot.height as f32);
                (
                    (slot.width as f32 * scale).round().max(1.0) as usize,
                    (slot.height as f32 * scale).round().max(1.0) as usize,
                )
            } else {
                (target_width, target_height)
            };
            let draw_width = draw_width.min(target_width);
            let draw_height = draw_height.min(target_height);
            let offset_x = (target_width - draw_width) / 2;
            let offset_y = (target_height - draw_height) / 2;
            let source_width = slot.width as usize;
            let source_height = slot.height as usize;
            for y in 0..draw_height {
                let source_y = y * source_height / draw_height;
                for x in 0..draw_width {
                    let source_x = x * source_width / draw_width;
                    pixels[(offset_y + y) * target_width + offset_x + x] =
                        slot.pixels[source_y * source_width + source_x];
                }
            }
            Some(pixels)
        })?
    }

    /// 卸载位图并清除路径缓存引用。
    pub fn unload(&self, handle: BitmapHandle) {
        let circular = {
            let mut cache = self.circular_cache.borrow_mut();
            take_derived_handles(&mut cache, handle, |key| key.0)
        };
        let rounded_square = {
            let mut cache = self.rounded_square_cache.borrow_mut();
            take_derived_handles(&mut cache, handle, |key| key.0)
        };
        let rounded_rect = {
            let mut cache = self.rounded_rect_cache.borrow_mut();
            take_derived_handles(&mut cache, handle, |key| key.0)
        };
        let square = {
            let mut cache = self.square_cache.borrow_mut();
            take_derived_handles(&mut cache, handle, |key| *key)
        };
        self.invalidate_slot(handle);
        for derived in circular
            .into_iter()
            .chain(rounded_square)
            .chain(rounded_rect)
            .chain(square)
        {
            self.invalidate_slot(derived);
        }
        self.compact();
    }

    fn invalidate_slot(&self, handle: BitmapHandle) {
        let mut slots = self.slots.borrow_mut();
        let idx = handle.slot_index();
        if let Some(slot) = slots
            .get_mut(idx)
            .filter(|s| s.generation == handle.generation())
        {
            if let Some(ref path) = slot.path {
                self.path_cache.borrow_mut().remove(path);
            }
            let generation = slot.generation;
            *slot = ImageSlot {
                generation,
                ..ImageSlot::default()
            };
        }
    }

    /// 近似内存占用（字节）。
    pub fn memory_usage(&self) -> usize {
        let slot_usage = self
            .slots
            .borrow()
            .iter()
            .filter(|s| s.valid)
            .map(|s| s.pixels.len() * 4 + std::mem::size_of::<ImageSlot>())
            .sum::<usize>();
        slot_usage + self.slot_generations.borrow().len() * std::mem::size_of::<u32>()
    }

    fn insert_slot(&self, mut slot: ImageSlot) -> BitmapHandle {
        let mut slots = self.slots.borrow_mut();
        let mut generations = self.slot_generations.borrow_mut();
        // Reuse freed slots, bumping generation to invalidate old handles.
        for (i, s) in slots.iter_mut().enumerate() {
            if !s.valid {
                let generation = generations[i].wrapping_add(1);
                generations[i] = generation;
                slot.generation = generation;
                *s = slot;
                return BitmapHandle::pack(i as u32, s.generation);
            }
        }
        let idx = slots.len();
        let generation = if let Some(generation) = generations.get_mut(idx) {
            *generation = generation.wrapping_add(1);
            *generation
        } else {
            debug_assert_eq!(idx, generations.len());
            generations.push(0);
            0
        };
        slot.generation = generation;
        slots.push(slot);
        BitmapHandle::pack(idx as u32, generation)
    }

    /// Compact the slot array by truncating trailing invalid slots.
    /// Generation 历史单独保留，因此截断后的索引复用不会让旧句柄重新生效。
    pub fn compact(&self) {
        let mut slots = self.slots.borrow_mut();
        while slots.last().is_some_and(|s| !s.valid) {
            slots.pop();
        }
    }
}

fn apply_pixel_coverage(pixel: u32, coverage: f32) -> u32 {
    if coverage >= 1.0 {
        return pixel;
    }
    if coverage <= 0.0 {
        return 0;
    }
    let scale = |channel: u32| ((channel as f32 * coverage).round() as u32).min(255);
    let a = scale((pixel >> 24) & 0xFF);
    let r = scale((pixel >> 16) & 0xFF);
    let g = scale((pixel >> 8) & 0xFF);
    let b = scale(pixel & 0xFF);
    (a << 24) | (r << 16) | (g << 8) | b
}

fn apply_rounded_rect_mask(pixels: &mut [u32], width: usize, height: usize, radius: f32) {
    let half_width = width as f32 * 0.5;
    let half_height = height as f32 * 0.5;
    for y in 0..height {
        for x in 0..width {
            let px = (x as f32 + 0.5 - half_width).abs();
            let py = (y as f32 + 0.5 - half_height).abs();
            let qx = px - (half_width - radius);
            let qy = py - (half_height - radius);
            let outside = qx.max(0.0).hypot(qy.max(0.0));
            let inside = qx.max(qy).min(0.0);
            let distance = outside + inside - radius;
            let coverage = (0.5 - distance).clamp(0.0, 1.0);
            let pixel = &mut pixels[y * width + x];
            *pixel = apply_pixel_coverage(*pixel, coverage);
        }
    }
}

fn take_derived_handles<K: Eq + std::hash::Hash>(
    cache: &mut HashMap<K, BitmapHandle>,
    source: BitmapHandle,
    source_of: impl Fn(&K) -> BitmapHandle,
) -> Vec<BitmapHandle> {
    let mut derived = Vec::new();
    cache.retain(|key, value| {
        let from_source = source_of(key) == source;
        if from_source {
            derived.push(*value);
        }
        !from_source && *value != source
    });
    derived
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

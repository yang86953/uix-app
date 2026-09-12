//! 图片加载与缓存 — 独立于渲染引擎的位图资源管理。
//!
//! 职责：解码 PNG/JPEG 等格式、路径缓存、按句柄提供像素数据。
//! 渲染通过 `PaintContext::draw_image` 调用 `Canvas2D::blit_image`。

// 图片编解码 capability 启用时才编译第三方格式解码模块。
#[cfg(feature = "image-codecs")]
// 该模块只负责把压缩图片转换为框架 RGBA 像素。
pub(crate) mod decode;

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
// 引入共享圆角值与圆角 SDF，供背景瓦片盒相对掩码复用。
use crate::draw::geometry::types::Radius;
use std::sync::Arc;
// 图片编解码 capability 启用时才需要读取文件。
#[cfg(feature = "image-codecs")]
// 路径解码入口使用标准文件系统读取压缩数据。
use std::fs;
// 图片编解码 capability 启用时才公开路径加载入口。
#[cfg(feature = "image-codecs")]
// 路径类型仅服务于图片文件解码。
use std::path::Path;
// 异步图片解码通过标准通道把后台结果交回资源所有者线程。
#[cfg(feature = "image-codecs")]
// 只引入非阻塞轮询需要的通道类型。
use std::sync::mpsc::{Receiver, TryRecvError};

// 图片编解码 capability 启用时才构造解码与文件错误。
use crate::core::Rect;
#[cfg(feature = "image-codecs")]
// 解码入口继续返回框架统一错误类型。
use crate::core::error::Error;
use crate::draw::Canvas2D;

// 图片编解码 capability 启用时才暴露压缩数据解码函数。
#[cfg(feature = "image-codecs")]
// 关闭 capability 后不保留绕过 ImageService 的公开解码路径。
pub use decode::decode_to_pixels;

/// 解码位图句柄（区别于离屏缓冲 `ImageHandle`）。
/// 低 32 位为槽位索引，高 32 位为 generation，防止 unload/reuse 后旧句柄静默指向新图片。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BitmapHandle(pub u64);

impl BitmapHandle {
    /// 不指向任何位图槽位的哨兵句柄。
    pub const INVALID: Self = Self(u64::MAX);

    /// 返回该值是否不是无效哨兵；资源是否仍存在应由 [`ImageService::is_valid`] 检查。
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
    pixels: Arc<Vec<u32>>,
    path: Option<String>,
    valid: bool,
    generation: u32,
}

impl ImageSlot {
    fn from_decoded(w: i32, h: i32, pixels: Vec<u32>, path: Option<String>) -> Self {
        Self {
            width: w,
            height: h,
            // 只把 Vec 头移动进共享所有者，不在冷加载阶段复制整张图片。
            pixels: Arc::new(pixels),
            path,
            valid: true,
            generation: 0,
        }
    }

    /// 返回位图的固有像素宽度。
    pub fn width(&self) -> i32 {
        self.width
    }

    /// 返回位图的固有像素高度。
    pub fn height(&self) -> i32 {
        self.height
    }

    /// 借用按行紧密排列的预乘 AARRGGBB 像素。
    pub fn pixels(&self) -> &[u32] {
        self.pixels.as_slice()
    }

    // 为帧录制交付不可变像素所有权；卸载槽位不会使已录制帧悬垂。
    fn shared_pixels(&self) -> Arc<Vec<u32>> {
        Arc::clone(&self.pixels)
    }
}

type RoundedRectCache = HashMap<(BitmapHandle, u32, u32, u32, bool), BitmapHandle>;
// 盒相对圆角背景瓦片派生缓存：句柄、设备宽高、量化盒/瓦几何与四个量化半径。
type RoundedBackgroundKey = (
    BitmapHandle,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
);
// 派生瓦片缓存上限：resize/动画下按 LRU 淘汰并释放槽位，保持资源有界。
pub(crate) const ROUNDED_BACKGROUND_CACHE_CAP: usize = 48;

// 保存后台图片解码任务的 typed 结果。
#[cfg(feature = "image-codecs")]
type AsyncDecodeResult = Result<(i32, i32, Vec<u32>), Error>;

/// 图片服务 — 解码缓存与路径索引。
pub struct ImageService {
    slots: RefCell<Vec<ImageSlot>>,
    slot_generations: RefCell<Vec<u32>>,
    pub(crate) path_cache: RefCell<HashMap<String, BitmapHandle>>,
    square_cache: RefCell<HashMap<BitmapHandle, BitmapHandle>>,
    circular_cache: RefCell<HashMap<(BitmapHandle, u32), BitmapHandle>>,
    rounded_square_cache: RefCell<HashMap<(BitmapHandle, u32, u32), BitmapHandle>>,
    rounded_rect_cache: RefCell<RoundedRectCache>,
    rounded_background_cache: RefCell<(
        HashMap<RoundedBackgroundKey, BitmapHandle>,
        VecDeque<RoundedBackgroundKey>,
    )>,
    // 按规范化路径保存仍在后台读取或解码的单次任务。
    #[cfg(feature = "image-codecs")]
    pending_path_decodes: RefCell<HashMap<String, Receiver<AsyncDecodeResult>>>,
}

impl Default for ImageService {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageService {
    /// 创建没有已登记位图、派生缓存或后台解码任务的图片服务。
    pub fn new() -> Self {
        Self {
            slots: RefCell::new(Vec::new()),
            slot_generations: RefCell::new(Vec::new()),
            path_cache: RefCell::new(HashMap::new()),
            square_cache: RefCell::new(HashMap::new()),
            circular_cache: RefCell::new(HashMap::new()),
            rounded_square_cache: RefCell::new(HashMap::new()),
            rounded_rect_cache: RefCell::new(HashMap::new()),
            rounded_background_cache: RefCell::new((HashMap::new(), VecDeque::new())),
            // 新服务初始没有后台图片任务。
            #[cfg(feature = "image-codecs")]
            pending_path_decodes: RefCell::new(HashMap::new()),
        }
    }

    /// 从原始字节加载图片，返回新句柄。
    // 图片编解码 capability 启用时才公开字节解码入口。
    #[cfg(feature = "image-codecs")]
    // 关闭 capability 后使用方无法绕过依赖门控调用该方法。
    pub fn load_from_bytes(&self, data: &[u8]) -> Result<BitmapHandle, Error> {
        let (w, h, pixels) = decode_to_pixels(data)?;
        Ok(self.insert_slot(ImageSlot::from_decoded(w, h, pixels, None)))
    }

    /// 从文件路径加载图片（带路径缓存）。
    // 图片编解码 capability 启用时才公开路径解码入口。
    #[cfg(feature = "image-codecs")]
    // 关闭 capability 后路径缓存仍可服务已有内部槽位清理。
    pub fn load_from_path(&self, path: impl AsRef<Path>) -> Result<BitmapHandle, Error> {
        // UTF-8 路径命中缓存时保持借用，避免绘制热路径重复分配 String。
        let path_str = path.as_ref().to_string_lossy();
        if let Some(handle) = self.path_cache.borrow().get(path_str.as_ref()).copied() {
            if self.is_valid(handle) {
                return Ok(handle);
            }
        }
        let data = fs::read(path_str.as_ref())
            .map_err(|e| Error::io_error(format!("读取图片文件 '{path_str}' 失败: {e}")))?;
        let (w, h, pixels) = decode_to_pixels(&data)?;
        // 只有缓存未命中且解码成功后才取得路径所有权。
        let path_str = path_str.into_owned();
        let handle = self.insert_slot(ImageSlot::from_decoded(
            w,
            h,
            pixels,
            Some(path_str.clone()),
        ));
        self.path_cache.borrow_mut().insert(path_str, handle);
        Ok(handle)
    }

    /// 非阻塞请求文件图片：首次启动后台解码并返回 `Ok(None)`，完成后返回句柄。
    // 图片编解码 capability 启用时才公开异步路径入口。
    #[cfg(feature = "image-codecs")]
    // 调用方必须在后继帧继续轮询，typed error 由最终轮询返回。
    pub fn poll_load_from_path(
        &self,
        // 接收需要缓存的本地文件路径。
        path: impl AsRef<Path>,
    ) -> Result<Option<BitmapHandle>, Error> {
        // 使用与同步缓存一致的有损字符串身份；命中路径保持借用。
        let path_string = path.as_ref().to_string_lossy();
        // 空路径不能形成稳定的资源请求。
        if path_string.is_empty() {
            // 返回统一无效参数错误。
            return Err(Error::invalid_arg("图片文件路径不能为空"));
        }
        // 优先复用已经完成并仍然有效的路径缓存。
        if let Some(handle) = self.path_cache.borrow().get(path_string.as_ref()).copied() {
            // 代际仍有效时直接交付现有句柄。
            if self.is_valid(handle) {
                // 同步加载可能抢先完成，清理同路径残留后台接收端。
                self.pending_path_decodes
                    .borrow_mut()
                    .remove(path_string.as_ref());
                // 已缓存图片不需要后台任务。
                return Ok(Some(handle));
            }
        }
        // 非阻塞读取现有后台任务的当前结果。
        let polled = {
            // 暂时借用任务表，只在通道轮询期间持有。
            let pending = self.pending_path_decodes.borrow();
            // 查找当前路径对应的唯一任务。
            pending.get(path_string.as_ref()).map(Receiver::try_recv)
        };
        // 已有任务时根据通道状态完成本轮查询。
        if let Some(polled) = polled {
            // 区分完成、仍在执行与异常断开。
            match polled {
                // 后台任务已经返回解码结果。
                Ok(result) => {
                    // 完成任务只消费一次，立即移出任务表。
                    self.pending_path_decodes
                        .borrow_mut()
                        .remove(path_string.as_ref());
                    // 传播后台文件或解码 typed error。
                    let (width, height, pixels) = result?;
                    // 任务完成并需要落入槽位与路径表时才取得 String 所有权。
                    let path_string = path_string.into_owned();
                    // 只在所有解码步骤成功后登记有效资源槽位。
                    let handle = self.insert_slot(ImageSlot::from_decoded(
                        // 保存固有像素宽度。
                        width,
                        // 保存固有像素高度。
                        height,
                        // 移交后台产生的 RGBA 像素。
                        pixels,
                        // 保存路径以支持卸载时清理缓存。
                        Some(path_string.clone()),
                    ));
                    // 与同步加载共享同一条路径缓存。
                    self.path_cache.borrow_mut().insert(path_string, handle);
                    // 向调用方交付可绘制句柄。
                    return Ok(Some(handle));
                }
                // 空通道表示后台线程仍在读取或解码。
                Err(TryRecvError::Empty) => {
                    // 本轮不阻塞 UI 所有者线程。
                    return Ok(None);
                }
                // 发送端异常结束时移除失效任务并返回 typed error。
                Err(TryRecvError::Disconnected) => {
                    // 清理已无法完成的任务身份。
                    self.pending_path_decodes
                        .borrow_mut()
                        .remove(path_string.as_ref());
                    // 通道断开属于资源服务状态错误。
                    return Err(Error::invalid_state(format!(
                        // 保留确切路径方便诊断。
                        "图片后台解码任务异常结束: {path_string}"
                    )));
                }
            }
        }
        // 首次任务必须跨线程与跨轮次保存路径，此时才转成 owned。
        let path_string = path_string.into_owned();
        // 为首次请求建立容量一的单结果通道。
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        // 后台线程拥有独立路径副本。
        let worker_path = path_string.clone();
        // 启动只处理当前文件的一次性后台任务。
        std::thread::Builder::new()
            // 使用稳定名称辅助崩溃和性能诊断。
            .name("uix-image-decode".to_owned())
            // 后台只执行文件读取与纯像素解码，不接触 ImageService 状态。
            .spawn(move || {
                // 把文件系统失败映射为框架 typed error。
                let result = fs::read(&worker_path)
                    // 保留失败路径与系统错误详情。
                    .map_err(|error| {
                        // 使用统一 I/O 错误码。
                        Error::io_error(format!("读取图片文件 '{worker_path}' 失败: {error}"))
                    })
                    // 文件读取成功后执行纯解码。
                    .and_then(|data| decode_to_pixels(&data));
                // 服务或请求方已销毁时允许发送失败并自然结束线程。
                let _ = sender.send(result);
            })
            // 线程创建失败必须同步返回 typed error，不能伪装成加载中。
            .map_err(|error| Error::invalid_state(format!("启动图片后台解码任务失败: {error}")))?;
        // 只有线程创建成功后才登记接收端。
        self.pending_path_decodes
            // 获取任务表可变借用。
            .borrow_mut()
            // 为当前路径登记唯一后台任务。
            .insert(path_string, receiver);
        // 首次请求不等待文件 I/O 或解码完成。
        Ok(None)
    }

    /// 懒加载：路径为空或加载失败返回 `None`。
    // 图片编解码 capability 启用时才公开路径懒加载入口。
    #[cfg(feature = "image-codecs")]
    // 关闭 capability 后路径型组件 API 会同步收缩。
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
    // 测试目标保留居中裁切入口，供图片资源契约测试按需调用。
    #[allow(dead_code)]
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
    pub fn circular_crop_sized(
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
    pub fn rounded_square_crop_sized(
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
    pub fn rounded_rect_sized(
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

    /// 按背景盒坐标生成精确圆角裁剪的瓦片派生图（盒相对掩码）。
    ///
    /// `tile`/`box_rect`/`corner` 同为逻辑坐标且 `corner` 已按盒尺寸归一化：
    /// 每个设备像素逆映射回逻辑空间后，用共享 `rounded_rect_sdf` 相对
    /// 背景盒求值，整层所有相交瓦片获得同一圆角边界（直边部分与盒边
    /// 重合，由外层矩形裁剪负责）。设备宽高按 `device_pixel_ratio` 换算
    /// 并钳制到 4096；钳制后的有效缩放同时作用于掩码求值与抗锯齿带，
    /// 不静默改变逻辑圆角形状。缓存按量化几何键 LRU 有界（resize/动画
    /// 下淘汰最旧并释放槽位）。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn rounded_background_tile(
        &self,
        handle: BitmapHandle,
        tile: Rect,
        box_rect: Rect,
        corner: Radius,
        device_pixel_ratio: f32,
    ) -> Option<BitmapHandle> {
        let dpr = if device_pixel_ratio.is_finite() && device_pixel_ratio > 0.0 {
            device_pixel_ratio
        } else {
            1.0
        };
        let target_width = ((tile.w * dpr).round() as i32).clamp(1, 4096) as u32;
        let target_height = ((tile.h * dpr).round() as i32).clamp(1, 4096) as u32;
        // 钳制后的有效缩放：设备像素 ↔ 逻辑像素的真实比例。
        let scale_x = target_width as f32 / tile.w;
        let scale_y = target_height as f32 / tile.h;
        let ramp_scale = (scale_x + scale_y) * 0.5;
        // 量化几何键：偏移、盒尺寸与半径按 1/64 逻辑像素离散。
        let quantize = |value: f32| (value * 64.0).round() as i32 as u32;
        let key = (
            handle,
            target_width,
            target_height,
            quantize(box_rect.x - tile.x),
            quantize(box_rect.y - tile.y),
            quantize(box_rect.w),
            quantize(box_rect.h),
            quantize(corner.tl),
            quantize(corner.tr),
            quantize(corner.br),
            quantize(corner.bl),
            quantize(ramp_scale * 64.0),
        );
        {
            let mut cache = self.rounded_background_cache.borrow_mut();
            if let Some(cached) = cache.0.get(&key).copied() {
                if self.is_valid(cached) {
                    // 命中即提升为最新使用。
                    if let Some(index) = cache.1.iter().position(|k| *k == key) {
                        cache.1.remove(index);
                    }
                    cache.1.push_back(key);
                    return Some(cached);
                }
                cache.0.remove(&key);
                if let Some(index) = cache.1.iter().position(|k| *k == key) {
                    cache.1.remove(index);
                }
            }
        }
        let mut pixels = self.resized_rect_pixels(handle, target_width, target_height, false)?;
        // 设备像素逆映射回逻辑空间后按背景盒求值共享圆角 SDF。
        for py in 0..target_height as usize {
            for px in 0..target_width as usize {
                let logical_x = tile.x + (px as f32 + 0.5) / scale_x;
                let logical_y = tile.y + (py as f32 + 0.5) / scale_y;
                let sdf = crate::draw::raster::rasterizer::core::rounded_rect_sdf(
                    logical_x, logical_y, &box_rect, &corner,
                );
                let coverage = (0.5 - sdf * ramp_scale).clamp(0.0, 1.0);
                let pixel = &mut pixels[py * target_width as usize + px];
                *pixel = apply_pixel_coverage(*pixel, coverage);
            }
        }
        let derived = self.insert_slot(ImageSlot::from_decoded(
            target_width as i32,
            target_height as i32,
            pixels,
            None,
        ));
        let evicted = {
            let mut cache = self.rounded_background_cache.borrow_mut();
            cache.0.insert(key, derived);
            cache.1.push_back(key);
            // 淘汰最旧条目并同步释放其派生槽位，保证资源有界。
            let mut evicted = Vec::new();
            while cache.1.len() > ROUNDED_BACKGROUND_CACHE_CAP {
                let Some(oldest) = cache.1.pop_front() else {
                    break;
                };
                if let Some(handle) = cache.0.remove(&oldest) {
                    evicted.push(handle);
                }
            }
            evicted
        };
        for handle in evicted {
            self.invalidate_slot(handle);
        }
        Some(derived)
    }

    /// 派生瓦片缓存当前条目数（cfg(test) 观测有界性）。
    #[cfg(test)]
    pub(crate) fn rounded_background_cache_len(&self) -> usize {
        self.rounded_background_cache.borrow().0.len()
    }

    /// 派生瓦片缓存上限（cfg(test) 与断言共享同一常量）。
    #[cfg(test)]
    pub(crate) fn rounded_background_cache_cap(&self) -> usize {
        ROUNDED_BACKGROUND_CACHE_CAP
    }

    // 居中像素提取是图片裁切的兼容辅助入口，当前测试矩阵按需调用。
    #[allow(dead_code)]
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
        let rounded_background = {
            let mut cache = self.rounded_background_cache.borrow_mut();
            take_derived_handles(&mut cache.0, handle, |key| key.0)
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
            .chain(rounded_background)
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
        canvas.blit_image_shared(slot.shared_pixels(), w, src, dst);
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

// 图片异步加载契约测试独立存放，避免资源模块继续增长。

#[cfg(all(test, feature = "image-codecs"))]
#[path = "../../../../tests-src/draw/resources/s4_rounded_tile_tests.rs"]
mod s4_rounded_tile_tests;

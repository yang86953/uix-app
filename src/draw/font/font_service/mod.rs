//! FontService — 独立于渲染器的字体系统。
//!
//! 职责：字体加载、元数据管理、文本布局、字形光栅化（含统一缓存）、
//!       自动多字体回退（主字体→回退链→Bitmap 位图回退）。
//! 渲染引擎只需通过 `draw_glyph_raster` 绘制像素，无需实现字体逻辑。

mod font_cache;
mod font_layout;

pub use font_cache::{FontFace, GlyphCache};

use std::sync::Arc;

use crate::core::Error;
use crate::core::{Point, Size};
use crate::draw::font::text_backend::{self as tb, GlyphRaster, TextLayoutOptions};
use crate::draw::FontHandle;
use crate::draw::TextBackend;

use font_cache::{CachedRaster, FontSlot, GlyphCacheKey};

// ════════════════════════════════════════════════════════════════════════════
// FontService — 字体管理器
// ════════════════════════════════════════════════════════════════════════════

/// 字体服务——渲染器无关的字体子系统。
///
/// 管理所有已加载字体（通过 `text_backend`），提供：
/// - 字体加载/卸载（委托给后端）
/// - 字体元数据注册（family、path）
/// - 统一字形缓存
/// - 多字体回退布局（主字体 → fallback_chain → BitmapFont）
/// - 文本度量、布局、击中测试
pub struct FontService {
    pub(crate) text_backend: Box<dyn TextBackend>,
    /// 字体注册表：索引 = FontHandle.0。
    registry: Vec<FontSlot>,
    pub(crate) primary_family: String,
    pub(crate) user_family_set: bool,
    /// 主字体句柄（由 load_default_system_font 设置）。
    pub loaded_font_handle: FontHandle,
    /// 回退字体链（有序）。BitmapFont 自动作为最终回退。
    fallback_handles: Vec<FontHandle>,
    /// 统一字形缓存。
    glyph_cache: GlyphCache,
}

impl FontService {
    /// 创建新的字体服务实例（ab_glyph 后端）。
    pub fn new() -> Self {
        Self {
            text_backend: Box::new(
                crate::draw::font::text_backends::ab_glyph::AbGlyphBackend::new(),
            ),
            registry: Vec::new(),
            primary_family: "sans-serif".into(),
            user_family_set: false,
            loaded_font_handle: FontHandle::new(0),
            fallback_handles: Vec::new(),
            glyph_cache: GlyphCache::new(),
        }
    }

    /// 替换文本后端（例如替换为 FreeType 后端）。
    /// 必须在 `load_default_system_font` 之前调用。
    pub fn with_text_backend(mut self, backend: Box<dyn TextBackend>) -> Self {
        self.text_backend = backend;
        self
    }

    // ── 字体元数据查询 ──

    /// 返回字体族名称（若注册表中有记录）。
    pub fn font_family(&self, handle: &FontHandle) -> Option<&str> {
        let i = handle.0 as usize;
        if i < self.registry.len() && self.registry[i].handle.0 != u32::MAX {
            Some(self.registry[i].face.family.as_str())
        } else {
            None
        }
    }

    /// 返回字体文件路径（若从文件加载）。
    pub fn font_path(&self, handle: &FontHandle) -> Option<&str> {
        let i = handle.0 as usize;
        if i < self.registry.len() {
            self.registry[i].face.path.as_deref()
        } else {
            None
        }
    }

    /// 已注册的字体数量（含已卸载的无效槽位）。
    pub fn font_count(&self) -> usize {
        self.registry.len()
    }

    /// 回退链中的字体数量。
    pub fn fallback_count(&self) -> usize {
        self.fallback_handles.len()
    }

    // ── 字体加载 ──

    /// 设置首选字体族名称。
    ///
    /// 必须在调用 `load_default_system_font()` **之前**调用才能生效。
    pub fn set_font_family(&mut self, family: impl Into<String>) {
        self.primary_family = family.into();
        self.user_family_set = true;
    }

    /// 加载字体数据，返回字体句柄，同时在注册表中记录元数据。
    pub fn load_font(&mut self, data: &[u8]) -> Result<FontHandle, Error> {
        let handle = self.text_backend.load_font(data)?;
        // 确保注册表有足够槽位
        let idx = handle.0 as usize;
        while self.registry.len() <= idx {
            self.registry.push(FontSlot {
                handle: FontHandle::new(u32::MAX),
                face: FontFace {
                    family: String::new(),
                    path: None,
                },
            });
        }
        self.registry[idx] = FontSlot {
            handle,
            face: FontFace {
                family: self.primary_family.clone(),
                path: None,
            },
        };
        // 注意：不更新 loaded_font_handle。loaded_font_handle 只由
        // load_default_system_font() 设置为主字体句柄。图标字体等辅助字体
        // 应通过 load_font() 加载但不应覆盖主字体句柄。
        Ok(handle)
    }

    /// 从文件路径加载字体，注册时会记录路径信息。
    pub fn load_font_from_path(&mut self, path: &str, _size: f32) -> Option<FontHandle> {
        let data = std::fs::read(path).ok()?;
        let handle = self.text_backend.load_font(&data).ok()?;
        let idx = handle.0 as usize;
        while self.registry.len() <= idx {
            self.registry.push(FontSlot {
                handle: FontHandle::new(u32::MAX),
                face: FontFace {
                    family: String::new(),
                    path: None,
                },
            });
        }
        // 尝试从文件名推导 family
        let family = Self::infer_family_from_path(path);
        self.registry[idx] = FontSlot {
            handle,
            face: FontFace {
                family,
                path: Some(path.to_owned()),
            },
        };
        Some(handle)
    }

    /// 从路径字符串猜测字体族名称。
    fn infer_family_from_path(path: &str) -> String {
        let name = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        // 去掉常见的变体后缀
        let cleaned = name
            .replace("-Regular", "")
            .replace("-Bold", "")
            .replace("-Italic", "")
            .replace("-Medium", "")
            .replace("-Light", "")
            .replace("_Regular", "")
            .replace("_Bold", "");
        if cleaned.is_empty() {
            name.to_owned()
        } else {
            cleaned
        }
    }

    /// 卸载字体。
    pub fn unload_font(&mut self, handle: &FontHandle) {
        self.text_backend.unload_font(handle);
        let idx = handle.0 as usize;
        if idx < self.registry.len() {
            self.registry[idx].handle = FontHandle::new(u32::MAX);
        }
        // 从回退链中移除
        self.fallback_handles.retain(|h| h.0 != handle.0);
    }

    /// 检查字体句柄是否有效。
    pub fn is_valid(&self, handle: &FontHandle) -> bool {
        self.text_backend.is_valid(handle)
    }

    /// 检查字体是否包含指定字符的字形。
    pub fn has_glyph(&self, font: &FontHandle, ch: char) -> bool {
        self.text_backend.has_glyph(font, ch)
    }

    /// 添加字体到回退链尾部。
    ///
    /// 在 `load_default_system_font` 之后调用，添加额外的回退字体。
    /// 后添加的字体优先级更低（排在链尾）。
    pub fn add_fallback(&mut self, handle: FontHandle) {
        if !self.fallback_handles.iter().any(|h| h.0 == handle.0) {
            self.fallback_handles.push(handle);
        }
    }

    /// 显式设置完整回退链。
    pub fn set_fallback_chain(&mut self, handles: &[FontHandle]) {
        self.fallback_handles = handles.to_vec();
    }

    /// 返回当前回退链。
    pub fn fallback_chain(&self) -> &[FontHandle] {
        &self.fallback_handles
    }

    /// 为指定字符查找可用的字体句柄。
    ///
    /// 按优先级：primary（主字体）→ fallback_chain → 返回 None（使用 BitmapFont）。
    fn find_font_for_char(&self, primary: &FontHandle, ch: char) -> Option<FontHandle> {
        // 主字体有 glyph？
        if self.text_backend.is_valid(primary) && self.text_backend.has_glyph(primary, ch) {
            return Some(*primary);
        }
        // 回退链？
        for fb in &self.fallback_handles {
            if self.text_backend.is_valid(fb) && self.text_backend.has_glyph(fb, ch) {
                return Some(*fb);
            }
        }
        None
    }

    /// 加载系统默认字体（自动检测平台）。
    /// 第一个成功加载的字体作为主字体，其余作为回退链。
    /// 始终在注册表中记录 family 信息。
    pub fn load_default_system_font(
        &mut self,
        size: f32,
        system_info: &dyn crate::native::traits::system::ISystemInfo,
    ) {
        if self.user_family_set {
            let family = self.primary_family.clone();
            let found = self.load_family_font(&family, size, system_info);
            if found.is_some() {
                return;
            }

            let fallback_paths = system_info.default_font_paths();
            if let Some(path) = fallback_paths.first() {
                if let Ok(data) = std::fs::read(path) {
                    if self.load_raw_font(data, size).is_some() {
                        crate::core::log::info_fn(format!(
                            "Configured font '{}' not found, fallback: {}",
                            self.primary_family, path
                        ));
                        return;
                    }
                }
            }
        } else {
            let paths = system_info.default_font_paths();

            if !paths.is_empty() {
                let mut primary_loaded = false;

                for path in &paths {
                    match std::fs::read(path) {
                        Ok(data) => {
                            if let Some(handle) = self.load_raw_font(data, size) {
                                let idx = handle.0 as usize;
                                let fallback_name = std::path::Path::new(path)
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("fallback");
                                if idx < self.registry.len() {
                                    self.registry[idx].face.path = Some(path.clone());
                                    if !primary_loaded {
                                        self.registry[idx].face.family =
                                            self.primary_family.clone();
                                    } else {
                                        self.registry[idx].face.family = fallback_name.to_owned();
                                    }
                                }

                                if !primary_loaded {
                                    primary_loaded = true;
                                    self.loaded_font_handle = handle;
                                    crate::core::log::info_fn(format!(
                                        "Loaded system default font: {} (handle={:?})",
                                        path, handle
                                    ));
                                } else {
                                    crate::core::log::info_fn(format!(
                                        "Loaded fallback font: {}",
                                        path
                                    ));
                                    self.fallback_handles.push(handle);
                                }
                            }
                        }
                        Err(e) => {
                            crate::core::log::info_fn(format!(
                                "Failed to read font file {}: {}",
                                path, e
                            ));
                        }
                    }
                }

                if primary_loaded {
                    self.load_cjk_fallback(size, system_info);

                    if !self.fallback_handles.is_empty() {
                        self.text_backend.set_fallback_fonts(&self.fallback_handles);
                    }
                    return;
                }
            }
        }

        // 第 4 步：最后的兜底——随机扫描一个可用字体
        crate::core::log::info_fn("No primary font found via platform, scanning for fallback...");
        if let Some(path) = system_info.scan_fallback_font_path() {
            if let Ok(data) = std::fs::read(&path) {
                if let Some(handle) = self.load_raw_font(data, size) {
                    self.loaded_font_handle = handle;
                    let idx = handle.0 as usize;
                    if idx < self.registry.len() {
                        self.registry[idx].face.family = self.primary_family.clone();
                        self.registry[idx].face.path = Some(path.clone());
                    }
                    crate::core::log::info_fn(format!(
                        "Loaded fallback font (random scan): {} (handle={:?})",
                        path, handle
                    ));
                    return;
                }
            }
        }

        crate::core::log::info_fn("No primary font found, using bitmap fallback");
    }

    /// 加载 CJK 回退字体（通过平台层探测）。
    /// 如果主字体本身已经包含中文字形则跳过，避免重复加载。
    fn load_cjk_fallback(
        &mut self,
        size: f32,
        system_info: &dyn crate::native::traits::system::ISystemInfo,
    ) {
        let cjk_test = ['中', '国', '文'];
        let primary_has_cjk = cjk_test.iter().all(|&ch| {
            self.text_backend.is_valid(&self.loaded_font_handle)
                && self.text_backend.has_glyph(&self.loaded_font_handle, ch)
        });
        if primary_has_cjk {
            crate::core::log::info_fn(
                "Primary font already supports CJK, skipping CJK fallback load",
            );
            return;
        }

        let cjk_path = system_info.probe_cjk_font_path();
        match cjk_path {
            Some(path) => match std::fs::read(&path) {
                Ok(data) => {
                    if let Some(handle) = self.load_raw_font(data, size) {
                        if !self.fallback_handles.iter().any(|h| h.0 == handle.0) {
                            self.fallback_handles.push(handle);
                            crate::core::log::info_fn(format!(
                                "Loaded CJK fallback font: {}",
                                path
                            ));
                        }
                    } else {
                        crate::core::log::info_fn(format!(
                            "CJK font '{}' found but failed to load (unsupported format)",
                            path
                        ));
                    }
                }
                Err(e) => {
                    crate::core::log::info_fn(format!(
                        "Failed to read CJK font file {}: {}",
                        path, e
                    ));
                }
            },
            None => {
                crate::core::log::info_fn("No CJK fallback font found via platform");
            }
        }
    }

    /// 通过平台层按字体族名称查找并加载字体。
    fn load_family_font(
        &mut self,
        family: &str,
        size: f32,
        system_info: &dyn crate::native::traits::system::ISystemInfo,
    ) -> Option<()> {
        if let Some(p) = system_info.probe_family_font_path(family) {
            if let Ok(data) = std::fs::read(&p) {
                if self.load_raw_font(data, size).is_some() {
                    let idx = self.loaded_font_handle.0 as usize;
                    if idx < self.registry.len() {
                        self.registry[idx].face.family = family.to_owned();
                        self.registry[idx].face.path = Some(p.clone());
                    }
                    crate::core::log::info_fn(format!("Loaded family font '{}': {}", family, p));
                    return Some(());
                }
            }
        }
        None
    }

    /// 从 TTC（TrueType Collection）数据中提取第一个 TTF 子字体。
    /// TTC 文件结构："ttcf" + 版本(4B) + 字体数量(4B) + offset表。
    fn extract_first_ttf_from_ttc(data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 12 || &data[0..4] != b"ttcf" {
            return None;
        }
        // TTC header: tag(4) + version(4) + num_fonts(4) + offsets(num_fonts*4)
        let num_fonts = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;
        if num_fonts == 0 || data.len() < 12 + num_fonts * 4 {
            return None;
        }
        // 取第一个子字体的 offset
        let offset = u32::from_be_bytes([data[12], data[13], data[14], data[15]]) as usize;
        if offset >= data.len() {
            return None;
        }
        // TTC 中第一个子字体结束于第二个子字体的 offset（或文件末尾）
        let next_offset = if num_fonts > 1 {
            u32::from_be_bytes([data[16], data[17], data[18], data[19]]) as usize
        } else {
            data.len()
        };
        if next_offset > data.len() || offset >= next_offset {
            return None;
        }
        Some(data[offset..next_offset].to_vec())
    }

    /// 直接加载原始字体数据。
    /// 使用 text_backend 进行验证和加载，无需额外解析。
    /// 自动处理 TTC 格式：如果 text_backend 无法解析数据且数据是 TTC，
    /// 则尝试提取第一个 TTF 子字体后重试。
    fn load_raw_font(&mut self, data: Vec<u8>, size: f32) -> Option<FontHandle> {
        // 第 1 次尝试：直接加载原始数据
        let mut result = self.try_load_raw_font_data(&data, size);

        // 第 2 次尝试：如果失败且是 TTC 格式，提取第一个 TTF 子字体
        if result.is_none() && data.len() >= 4 && &data[0..4] == b"ttcf" {
            if let Some(ttf_data) = Self::extract_first_ttf_from_ttc(&data) {
                result = self.try_load_raw_font_data(&ttf_data, size);
            }
        }

        result
    }

    /// 尝试用 text_backend 加载字体数据并验证字形可光栅化。
    fn try_load_raw_font_data(&mut self, data: &[u8], size: f32) -> Option<FontHandle> {
        let handle = self.text_backend.load_font(data).ok()?;
        let raster = self.text_backend.rasterize_glyph(&handle, 65, size);
        if raster.width == 0 || raster.height == 0 {
            self.text_backend.unload_font(&handle);
            return None;
        }
        // 注册到 registry
        let idx = handle.0 as usize;
        while self.registry.len() <= idx {
            self.registry.push(FontSlot {
                handle: FontHandle::new(u32::MAX),
                face: FontFace {
                    family: String::new(),
                    path: None,
                },
            });
        }
        self.registry[idx] = FontSlot {
            handle,
            face: FontFace {
                family: self.primary_family.clone(),
                path: None,
            },
        };
        Some(handle)
    }

    // ── 文本度量 ──

    /// 测量文本尺寸。
    ///
    /// 使用 TTF 布局（含自动回退），最终回退到 BitmapFont 度量。
    pub fn measure_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> Size {
        if self.text_backend.is_valid(font) {
            let f = *font;
            let layout = self.layout_text(&f, text, opts);
            Size::new(layout.width, layout.height.max(opts.font_size))
        } else {
            // 回退到简单度量（无后端字体可用时）：按字符数，非字节
            let cw = 6.0;
            let lh = 10.0;
            let len = text.chars().count() as f32;
            Size::new(len * cw, lh)
        }
    }

    /// 光栅化一个字形（使用统一缓存）。
    pub fn rasterize_glyph(
        &self,
        font: &FontHandle,
        glyph_id: u32,
        pixel_size: f32,
    ) -> GlyphRaster {
        // 缺字 tofu：合成空心方框，避免静默丢字。
        if glyph_id == tb::TOFU_GLYPH_ID {
            return Self::rasterize_tofu(pixel_size);
        }

        if !self.text_backend.is_valid(font) {
            return GlyphRaster {
                width: 0,
                height: 0,
                coverage: Arc::new(vec![]),
                bearing_x: 0.0,
                bearing_y: 0.0,
            };
        }

        let ps = pixel_size.round() as u32;
        let key = GlyphCacheKey {
            font_idx: font.0,
            glyph_id,
            pixel_size: ps,
        };

        // 查缓存
        if let Some(cached) = self.glyph_cache.get(&key) {
            return GlyphRaster {
                width: cached.width,
                height: cached.height,
                coverage: cached.coverage,
                bearing_x: cached.bearing_x,
                bearing_y: cached.bearing_y,
            };
        }

        // 从后端光栅化
        let raster = self
            .text_backend
            .rasterize_glyph(font, glyph_id, pixel_size);

        // 写入缓存
        if ps > 0 && raster.width > 0 && raster.height > 0 {
            self.glyph_cache.insert(
                key,
                CachedRaster {
                    width: raster.width,
                    height: raster.height,
                    coverage: Arc::clone(&raster.coverage),
                    bearing_x: raster.bearing_x,
                    bearing_y: raster.bearing_y,
                },
            );
        }

        raster
    }

    /// 合成缺字 tofu（空心方框），相对基线的 bearing 与常规字形一致。
    fn rasterize_tofu(pixel_size: f32) -> GlyphRaster {
        let fs = pixel_size.max(1.0);
        let w = ((fs * 0.5).round() as usize).max(4);
        let h = ((fs * 0.7).round() as usize).max(5);
        let mut coverage = vec![0u8; w * h];
        for x in 0..w {
            coverage[x] = 220;
            coverage[(h - 1) * w + x] = 220;
        }
        for y in 0..h {
            coverage[y * w] = 220;
            coverage[y * w + (w - 1)] = 220;
        }
        GlyphRaster {
            width: w,
            height: h,
            coverage: Arc::new(coverage),
            bearing_x: (fs * 0.05).max(0.0),
            // 相对布局 y（≈ ascent）：方框顶落在 ascent 下方一点
            bearing_y: -(fs * 0.75),
        }
    }

    /// 命中测试，返回 **字符下标**（`chars()` 序）。
    pub fn hit_test_text(
        &self,
        font: &FontHandle,
        text: &str,
        opts: &TextLayoutOptions,
        point: Point,
    ) -> Option<usize> {
        if text.is_empty() {
            return None;
        }
        if self.text_backend.is_valid(font) {
            let f = *font;
            let layout = self.layout_text(&f, text, opts);
            let total_chars = text.chars().count();
            for li in &layout.lines {
                if point.y >= li.y && point.y < li.y + li.height {
                    let end = li.glyph_start + li.glyph_count;
                    let glyphs = &layout.glyphs[li.glyph_start..end.min(layout.glyphs.len())];
                    if glyphs.is_empty() {
                        return Some(li.start_char.min(total_chars));
                    }
                    for g in glyphs {
                        if point.x < g.x + g.width * 0.5 {
                            return Some(g.char_index.min(total_chars));
                        }
                    }
                    return Some(
                        glyphs
                            .last()
                            .map(|g| (g.char_index + 1).min(total_chars))
                            .unwrap_or(li.end_char.min(total_chars)),
                    );
                }
            }
            if let Some(last) = layout.lines.last() {
                return Some(last.end_char.min(total_chars));
            }
        }
        Some(0)
    }

    /// 获取指定 **字符下标** 的光标 x 位置。
    pub fn text_cursor_x(
        &self,
        font: &FontHandle,
        text: &str,
        opts: &TextLayoutOptions,
        char_index: usize,
    ) -> f32 {
        if text.is_empty() || !self.text_backend.is_valid(font) {
            return 0.0;
        }
        let f = *font;
        let layout = self.layout_text(&f, text, opts);
        if let Some(g) = layout.glyphs.iter().find(|g| g.char_index == char_index) {
            return g.x;
        }
        // 落在末尾或缺口：取最后一个 char_index < 目标 的右缘
        layout
            .glyphs
            .iter()
            .rev()
            .find(|g| g.char_index < char_index)
            .map_or(0.0, |g| g.x + g.width.max(0.0))
    }

    /// 获取水平行度量。
    pub fn horizontal_line_metrics(
        &self,
        font: &FontHandle,
        pixel_size: f32,
    ) -> Option<tb::LineMetrics> {
        self.text_backend.horizontal_line_metrics(font, pixel_size)
    }

    /// 返回字体原始数据。
    pub fn font_data(&self, font: &FontHandle) -> Option<Vec<u8>> {
        self.text_backend.font_data(font)
    }

    /// 清空字形位图缓存（释放内存）。
    pub fn clear_glyph_cache(&mut self) {
        self.glyph_cache.clear();
        self.text_backend.clear_cache();
    }

    /// 返回字体子系统近似内存使用（字节）。
    pub fn memory_usage(&self) -> usize {
        let backend_mem = self.text_backend.memory_usage();
        let cache_mem = self.glyph_cache.memory_usage();
        backend_mem + cache_mem
    }
}

impl Default for FontService {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for FontService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontService")
            .field("backend", &self.text_backend)
            .field("primary_family", &self.primary_family)
            .field("registry_size", &self.registry.len())
            .field("fallback_count", &self.fallback_handles.len())
            .field("cache_entries", &self.glyph_cache.len())
            .finish()
    }
}

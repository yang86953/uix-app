//! FontService — 独立于渲染器的字体系统。
//!
//! 职责：字体加载、元数据管理、文本布局、字形光栅化（含统一缓存）、
//!       自动多字体回退（主字体→回退链→Bitmap 位图回退）。
//! 渲染引擎只需通过 `draw_glyph_raster` 绘制像素，无需实现字体逻辑。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use uix_core::{Point, Size};
use uix_diag::Error;
use crate::bitmap_font::BitmapFont;
use crate::text_backend::{
    self as tb, GlyphRaster, TextBackend, TextLayout, TextLayoutOptions,
};
use crate::FontHandle;

// ════════════════════════════════════════════════════════════════════════════
// 字体元数据
// ════════════════════════════════════════════════════════════════════════════

/// 已加载字体的元数据。
#[derive(Debug, Clone)]
pub struct FontFace {
    /// 字体族名称（如 "Noto Sans SC", "sans-serif"）。
    pub family: String,
    /// 字体文件路径（从文件加载时才有值）。
    pub path: Option<String>,
}

/// 注册表中一个字体槽位。
#[derive(Debug)]
struct FontSlot {
    handle: FontHandle,
    face: FontFace,
}

// ════════════════════════════════════════════════════════════════════════════
// 字形缓存（统一、线程安全）
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct GlyphCacheKey {
    font_idx: u32,
    glyph_id: u16,
    pixel_size: u32,
}

#[derive(Debug, Clone)]
struct CachedRaster {
    width: usize,
    height: usize,
    coverage: Arc<Vec<u8>>,
    bearing_x: f32,
    bearing_y: f32,
}

/// 统一字形光栅缓存。
///
/// 由 FontService 统一持有，所有后端共享。后端自身不应再维护独立缓存。
/// 默认最多缓存 8192 个字形的覆盖位图。
#[derive(Debug)]
pub struct GlyphCache {
    inner: Mutex<HashMap<GlyphCacheKey, CachedRaster>>,
    max_entries: usize,
}

impl GlyphCache {
    fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            max_entries: 8192,
        }
    }

    fn get(&self, key: &GlyphCacheKey) -> Option<CachedRaster> {
        let map = self.inner.lock().ok()?;
        map.get(key).cloned()
    }

    fn insert(&self, key: GlyphCacheKey, raster: CachedRaster) {
        if let Ok(mut map) = self.inner.lock() {
            if map.len() >= self.max_entries {
                map.clear();
            }
            map.insert(key, raster);
        }
    }

    fn clear(&self) {
        if let Ok(mut map) = self.inner.lock() {
            map.clear();
        }
    }

    fn len(&self) -> usize {
        self.inner.lock().map(|m| m.len()).unwrap_or(0)
    }

    fn memory_usage(&self) -> usize {
        self.inner
            .lock()
            .map(|map| {
                map.iter()
                    .map(|(_, r)| std::mem::size_of::<GlyphCacheKey>() + r.width * r.height)
                    .sum()
            })
            .unwrap_or(0)
    }
}

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
    /// BitmapFont 实例（5x7 位图回退字体）。
    #[allow(dead_code)]
    bitmap_font: BitmapFont,
}

impl FontService {
    /// 创建新的字体服务实例（ab_glyph 后端）。
    pub fn new() -> Self {
        Self {
            text_backend: Box::new(
                crate::text_backends::ab_glyph::AbGlyphBackend::new(),
            ),
            registry: Vec::new(),
            primary_family: "sans-serif".into(),
            user_family_set: false,
            loaded_font_handle: FontHandle::new(0),
            fallback_handles: Vec::new(),
            glyph_cache: GlyphCache::new(),
            bitmap_font: BitmapFont::new(),
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
    pub fn load_default_system_font(&mut self, size: f32, system_info: &dyn uix_platform::ISystemInfo) {
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
                        uix_diag::log::info_fn(format!(
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
                                        self.registry[idx].face.family =
                                            fallback_name.to_owned();
                                    }
                                }

                                if !primary_loaded {
                                    primary_loaded = true;
                                    self.loaded_font_handle = handle;
                                    uix_diag::log::info_fn(format!(
                                        "Loaded system default font: {} (handle={:?})",
                                        path, handle
                                    ));
                                } else {
                                    uix_diag::log::info_fn(format!(
                                        "Loaded fallback font: {}",
                                        path
                                    ));
                                    self.fallback_handles.push(handle);
                                }
                            }
                        }
                        Err(e) => {
                            uix_diag::log::info_fn(format!(
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
        uix_diag::log::info_fn("No primary font, using bitmap fallback");
    }

    /// 加载 CJK 回退字体（通过平台层探测）。
    /// 如果主字体本身已经包含中文字形则跳过，避免重复加载。
    fn load_cjk_fallback(&mut self, size: f32, system_info: &dyn uix_platform::ISystemInfo) {
        let cjk_test = ['中', '国', '文'];
        let primary_has_cjk = cjk_test.iter().all(|&ch| {
            self.text_backend.is_valid(&self.loaded_font_handle)
                && self.text_backend.has_glyph(&self.loaded_font_handle, ch)
        });
        if primary_has_cjk {
            uix_diag::log::info_fn(
                "Primary font already supports CJK, skipping CJK fallback load"
            );
            return;
        }

        let cjk_path = system_info.probe_cjk_font_path();
        match cjk_path {
            Some(path) => {
                match std::fs::read(&path) {
                    Ok(data) => {
                        if let Some(handle) = self.load_raw_font(data, size) {
                            if !self.fallback_handles.iter().any(|h| h.0 == handle.0) {
                                self.fallback_handles.push(handle);
                                uix_diag::log::info_fn(format!(
                                    "Loaded CJK fallback font: {}",
                                    path
                                ));
                            }
                        } else {
                            uix_diag::log::info_fn(format!(
                                "CJK font '{}' found but failed to load (incompatible format)",
                                path
                            ));
                        }
                    }
                    Err(e) => {
                        uix_diag::log::info_fn(format!(
                            "Failed to read CJK font file {}: {}",
                            path, e
                        ));
                    }
                }
            }
            None => {
                uix_diag::log::info_fn(
                    "No CJK fallback font found via platform"
                );
            }
        }
    }

    /// 通过平台层按字体族名称查找并加载字体。
    fn load_family_font(&mut self, family: &str, size: f32, system_info: &dyn uix_platform::ISystemInfo) -> Option<()> {
        if let Some(p) = system_info.probe_family_font_path(family) {
            if let Ok(data) = std::fs::read(&p) {
                if self.load_raw_font(data, size).is_some() {
                    let idx = self.loaded_font_handle.0 as usize;
                    if idx < self.registry.len() {
                        self.registry[idx].face.family = family.to_owned();
                        self.registry[idx].face.path = Some(p.clone());
                    }
                    uix_diag::log::info_fn(format!(
                        "Loaded family font '{}': {}",
                        family, p
                    ));
                    return Some(());
                }
            }
        }
        None
    }

    /// 直接加载原始字体数据。
    /// 使用 text_backend 进行验证和加载，无需额外解析。
    fn load_raw_font(&mut self, data: Vec<u8>, size: f32) -> Option<FontHandle> {
        let handle = self.text_backend.load_font(&data).ok()?;
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
            // 回退到简单度量（无后端字体可用时）
            let cw = 6.0;
            let lh = 10.0;
            let len = text.len() as f32;
            Size::new(len * cw, lh)
        }
    }

    /// 布局文本，返回定位后的字形（含自动多字体回退）。
    ///
    /// - 逐行内分段：连续相同字体的字符组成一段，每段调用后端 layout_text。
    /// - 段间 x 坐标累积：同一行内后一段的 glyph.x += 前一段总宽度。
    /// - 垂直基线对齐：不同字体段共享同一行基线，用段 ascent 修正 y 偏移。
    /// - 换行：`\n` 强制换行；word_wrap 开启时按 max_width 自动换行（段级精度）。
    /// - LineInfo：按实际行构建，每行包含正确的起止 glyph 索引、宽度和高度。
    pub fn layout_text(
        &self,
        font: &FontHandle,
        text: &str,
        opts: &TextLayoutOptions,
    ) -> TextLayout {
        if !self.text_backend.is_valid(font) || text.is_empty() {
            return TextLayout {
                glyphs: vec![],
                lines: vec![],
                width: 0.0,
                height: 0.0,
            };
        }

        let fs = opts.font_size.max(1.0);
        let line_h = if opts.line_height > 0.0 {
            opts.line_height
        } else {
            fs * 1.5
        };
        let has_max_w = opts.max_width.is_finite() && opts.max_width > 0.0;
        let do_wrap = has_max_w && opts.word_wrap;
        let max_w = if has_max_w { opts.max_width } else { f32::MAX };

        // 主字体行度量（作为整行基线参考）
        let primary_metrics = self.text_backend.horizontal_line_metrics(font, fs);
        let primary_ascent = primary_metrics
            .map(|m| m.ascent)
            .unwrap_or(fs * 0.8);

        // 无换行的后端选项（段内不自动换行，由 FontService 控制）
        let seg_opts = TextLayoutOptions {
            max_width: f32::MAX,
            ..opts.clone()
        };

        let chars: Vec<char> = text.chars().collect();

        // ── 第 1 遍：分割为字体段 ──
        // FontSegment: (start, end, font)
        struct FontSegment {
            start: usize,
            end: usize,
            font: FontHandle,
        }
        let mut segments: Vec<FontSegment> = Vec::new();
        let mut seg_start = 0usize;
        let mut seg_font = *font;

        for (i, &ch) in chars.iter().enumerate() {
            if ch == '\n' {
                // 换行符结束一段
                if i > seg_start {
                    segments.push(FontSegment {
                        start: seg_start,
                        end: i,
                        font: seg_font,
                    });
                }
                segments.push(FontSegment {
                    start: i,
                    end: i + 1,
                    font: *font, // '\n' 用主字体
                });
                seg_start = i + 1;
                seg_font = *font;
                continue;
            }

            let best = self.find_font_for_char(font, ch);
            let use_font = best.unwrap_or(*font);

            if use_font.0 != seg_font.0 {
                // 字体切换
                if i > seg_start {
                    segments.push(FontSegment {
                        start: seg_start,
                        end: i,
                        font: seg_font,
                    });
                }
                seg_start = i;
                seg_font = use_font;
            }
        }
        // 最后一段
        if seg_start < chars.len() {
            segments.push(FontSegment {
                start: seg_start,
                end: chars.len(),
                font: seg_font,
            });
        }

        // ── 第 2 遍：逐段布局 → 按行拼接 ──
        #[derive(Default)]
        struct LineAccum {
            glyphs: Vec<tb::PositionedGlyph>,
            y: f32,           // 行顶部 y
            height: f32,      // 行高
            width: f32,       // 行总宽度
            #[allow(dead_code)]
            glyph_start: usize,
            #[allow(dead_code)]
            glyph_count: usize,
        }

        let mut lines: Vec<LineAccum> = Vec::new();
        let mut line = LineAccum::default();
        let mut cx = 0.0f32;
        let mut cy = 0.0f32; // 当前行顶部

        for seg in &segments {
            if seg.start >= seg.end {
                continue;
            }

            if chars[seg.start] == '\n' {
                // 换行符：结束当前行
                if !line.glyphs.is_empty() || lines.is_empty() {
                    line.width = cx.max(line.width);
                    line.height = line_h;
                    line.y = cy;
                    lines.push(line);
                }
                line = LineAccum::default();
                cx = 0.0;
                cy += line_h;
                continue;
            }

            // 获取段字体 ascent（用于基线对齐）
            let seg_metrics = self
                .text_backend
                .horizontal_line_metrics(&seg.font, fs);
            let seg_ascent = seg_metrics
                .map(|m| m.ascent)
                .unwrap_or(fs * 0.8);

            // 段文本
            let seg_text: String = chars[seg.start..seg.end].iter().collect();

            // 布局段（无段内换行，由 FontService 控制）
            let seg_layout = self
                .text_backend
                .layout_text(&seg.font, &seg_text, &seg_opts);

            // 计算段宽度
            let seg_width = seg_layout.width;

            // word_wrap：如果当前行已有内容且加入此段超宽，换行
            if do_wrap && cx > 0.0 && cx + seg_width > max_w {
                // 结束当前行
                line.width = cx.max(line.width);
                line.height = line_h;
                line.y = cy;
                lines.push(line);
                line = LineAccum::default();
                cx = 0.0;
                cy += line_h;
            }

            // 基线对齐偏移：段字体基线对齐到主字体基线
            let baseline_offset = primary_ascent - seg_ascent;

            // 将段内 glyph 合并到当前行
            for mut g in seg_layout.glyphs {
                g.x += cx; // 段内 x → 行内 x
                g.y += cy + baseline_offset; // 段内 y → 行内 y（基线对齐）
                g.font = seg.font;
                line.glyphs.push(g);
            }

            cx += seg_width;
        }

        // 最后一行
        if !line.glyphs.is_empty() || lines.is_empty() {
            line.width = cx.max(line.width);
            line.height = line_h;
            line.y = cy;
            lines.push(line);
        }

        // ── 第 3 遍：构建 TextLayout ──
        let mut all_glyphs = Vec::new();
        let mut line_infos = Vec::new();
        let mut total_height = 0.0f32;
        let mut max_line_width = 0.0f32;

        for l in &lines {
            let gs = all_glyphs.len();
            all_glyphs.extend_from_slice(&l.glyphs);
            line_infos.push(tb::LineInfo {
                y: l.y,
                height: l.height,
                width: l.width,
                start_char: 0,   // 由调用方填充
                end_char: 0,
                glyph_start: gs,
                glyph_count: l.glyphs.len(),
            });
            total_height = l.y + l.height;
            max_line_width = max_line_width.max(l.width);
        }

        TextLayout {
            glyphs: all_glyphs,
            lines: line_infos,
            width: max_line_width,
            height: total_height,
        }
    }

    /// 光栅化一个字形（使用统一缓存）。
    pub fn rasterize_glyph(
        &self,
        font: &FontHandle,
        glyph_id: u32,
        pixel_size: f32,
    ) -> GlyphRaster {
        if !self.text_backend.is_valid(font) {
            // 无效字体 → 尝试 BitmapFont 回退
            // BitmapFont 只支持 ASCII 32-126，这里不做具体字符映射
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
            glyph_id: glyph_id as u16,
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
        let raster = self.text_backend.rasterize_glyph(font, glyph_id, pixel_size);

        // 写入缓存
        if ps > 0 && raster.width > 0 && raster.height > 0 {
            self.glyph_cache.insert(key, CachedRaster {
                width: raster.width,
                height: raster.height,
                coverage: Arc::clone(&raster.coverage),
                bearing_x: raster.bearing_x,
                bearing_y: raster.bearing_y,
            });
        }

        raster
    }

    /// 命中测试。
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
            for li in &layout.lines {
                if point.y >= li.y && point.y < li.y + li.height {
                    let end = li.glyph_start + li.glyph_count;
                    let glyphs = &layout.glyphs[li.glyph_start..end.min(layout.glyphs.len())];
                    if glyphs.is_empty() {
                        return Some(li.glyph_start);
                    }
                    for (i, g) in glyphs.iter().enumerate() {
                        if point.x < g.x + g.width * 0.5 {
                            return Some(li.glyph_start + i);
                        }
                    }
                    return Some(li.glyph_start + glyphs.len() - 1);
                }
            }
            if let Some(last) = layout.lines.last() {
                return Some(last.glyph_start + last.glyph_count);
            }
        }
        Some(0)
    }

    /// 获取指定字符索引的光标 x 位置。
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
        if char_index < layout.glyphs.len() {
            return layout.glyphs[char_index].x;
        }
        layout
            .glyphs
            .last()
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

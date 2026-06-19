//! FontService — 独立于渲染器的字体系统。
//!
//! 职责：字体加载、文本布局、字形光栅化。
//! 渲染引擎只需通过 `draw_glyph_raster` 绘制像素，无需实现字体逻辑。

use crate::base::{Point, Size};
use crate::diag::Error;
use crate::graphics::text_backend::{
    self as tb, GlyphRaster, TextBackend, TextLayout, TextLayoutOptions,
};
use crate::graphics::FontHandle;

/// 字体服务——渲染器无关的字体子系统。
///
/// 每个 GraphicsEngine 应持有（而非实现）一个 FontService 实例，
/// 通过 `FontService::render_text(engine, ...)` 完成文本绘制。
pub struct FontService {
    pub(crate) text_backend: Box<dyn TextBackend>,
    pub(crate) primary_family: String,
    pub(crate) user_family_set: bool,
    pub(crate) loaded_font_handle: FontHandle,
}

impl FontService {
    /// 创建新的字体服务实例，使用默认的 fontdue 后端。
    pub fn new() -> Self {
        Self {
            text_backend: Box::new(
                crate::graphics::software_engine::fontdue_backend::FontdueBackend::new(),
            ),
            primary_family: "sans-serif".into(),
            user_family_set: false,
            loaded_font_handle: FontHandle::new(0),
        }
    }

    /// 替换文本后端（例如替换为 FreeType 后端）。
    /// 必须在 `load_default_system_font` 之前调用。
    pub fn with_text_backend(mut self, backend: Box<dyn TextBackend>) -> Self {
        self.text_backend = backend;
        self
    }

    // ── 字体加载 ──

    /// 设置首选字体族名称。
    ///
    /// 必须在调用 `load_default_system_font()` **之前**调用才能生效。
    pub fn set_font_family(&mut self, family: impl Into<String>) {
        self.primary_family = family.into();
        self.user_family_set = true;
    }

    /// 加载字体数据，返回字体句柄。
    pub fn load_font(&mut self, data: &[u8]) -> Result<FontHandle, Error> {
        let handle = self.text_backend.load_font(data)?;
        // 注意：不更新 loaded_font_handle。loaded_font_handle 只由
        // load_default_system_font() 设置为主字体句柄。图标字体等辅助字体
        // 应通过 load_font() 加载但不应覆盖主字体句柄。
        Ok(handle)
    }

    /// 卸载字体。
    pub fn unload_font(&mut self, handle: &FontHandle) {
        self.text_backend.unload_font(handle);
    }

    /// 检查字体句柄是否有效。
    pub fn is_valid(&self, handle: &FontHandle) -> bool {
        self.text_backend.is_valid(handle)
    }

    /// 检查字体是否包含指定字符的字形。
    pub fn has_glyph(&self, font: &FontHandle, ch: char) -> bool {
        self.text_backend.has_glyph(font, ch)
    }

    /// 加载系统默认字体（自动检测平台）。
    /// 第一个成功加载的字体作为主字体，其余作为回退链。
    pub fn load_default_system_font(&mut self, size: f32) {
        if self.user_family_set {
            let family = self.primary_family.clone();
            let found = self.load_family_font(&family, size);
            if found.is_some() {
                return;
            }

            #[cfg(target_os = "linux")]
            {
                let fallback = crate::platform::linux::system_info::probe_system_default_font();
                if let Some(path) = fallback {
                    if let Ok(data) = std::fs::read(&path) {
                        if self.load_raw_font(data, size).is_some() {
                            crate::diag::log::info_fn(format!(
                                "Configured font '{}' not found, fallback: {}",
                                self.primary_family, path
                            ));
                            return;
                        }
                    }
                }
            }
        } else {
            let paths: Vec<String> = {
                #[cfg(target_os = "windows")]
                {
                    crate::platform::windows::util::system_default_font_paths()
                }
                #[cfg(target_os = "linux")]
                {
                    match crate::platform::linux::system_info::probe_system_default_font() {
                        Some(p) => vec![p],
                        None => vec![],
                    }
                }
                #[cfg(not(any(target_os = "linux", target_os = "windows")))]
                {
                    vec![]
                }
            };

            if !paths.is_empty() {
                let mut primary_loaded = false;
                let mut fallback_handles: Vec<FontHandle> = Vec::new();

                for path in &paths {
                    match std::fs::read(path) {
                        Ok(data) => {
                            if let Some(handle) = self.load_raw_font(data, size) {
                                if !primary_loaded {
                                    primary_loaded = true;
                                    self.loaded_font_handle = handle;
                                    crate::diag::log::info_fn(format!(
                                        "Loaded system default font: {} (handle={:?})",
                                        path, handle
                                    ));
                                } else {
                                    crate::diag::log::info_fn(format!(
                                        "Loaded fallback font: {}",
                                        path
                                    ));
                                    fallback_handles.push(handle);
                                }
                            }
                        }
                        Err(e) => {
                            crate::diag::log::info_fn(format!(
                                "Failed to read font file {}: {}",
                                path, e
                            ));
                        }
                    }
                }

                if !fallback_handles.is_empty() {
                    self.text_backend.set_fallback_fonts(&fallback_handles);
                }

                if primary_loaded {
                    return;
                }
            }
        }
        crate::diag::log::info_fn("No primary font, using bitmap fallback");
    }

    /// 通过平台层按字体族名称查找并加载字体。
    fn load_family_font(&mut self, family: &str, size: f32) -> Option<()> {
        #[cfg(target_os = "linux")]
        {
            let pattern = format!("{}:scalable=true", family);
            let path = crate::platform::linux::system_info::probe_font_path_via_fc_match(&pattern)
                .or_else(|| {
                    let p2 = format!("{}", family);
                    crate::platform::linux::system_info::probe_font_path_via_fc_match(&p2)
                });
            if let Some(p) = path {
                if let Ok(data) = std::fs::read(&p) {
                    if self.load_raw_font(data, size).is_some() {
                        crate::diag::log::info_fn(format!(
                            "Loaded family font '{}': {}",
                            family, p
                        ));
                        return Some(());
                    }
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (family, size);
        }
        None
    }

    /// 直接加载原始字体数据，跳过 fontdb。
    fn load_raw_font(&mut self, data: Vec<u8>, size: f32) -> Option<FontHandle> {
        let Ok(ref test_font) =
            fontdue::Font::from_bytes(data.as_slice(), fontdue::FontSettings::default())
        else {
            return None;
        };
        let (metrics, _) = test_font.rasterize('A', size);
        if metrics.width == 0 || metrics.height == 0 {
            return None;
        }
        self.text_backend.load_font(&data).ok()
    }

    // ── 文本度量 ──

    /// 测量文本尺寸（使用 TTF 布局，回退到 bitmap 字体）。
    pub fn measure_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> Size {
        if self.text_backend.is_valid(font) {
            let layout = self.text_backend.layout_text(font, text, opts);
            Size::new(layout.width, layout.height.max(opts.font_size))
        } else {
            // 回退到位图字体度量
            let cw = 6.0;
            let lh = 10.0;
            let len = text.len() as f32;
            Size::new(len * cw, lh)
        }
    }

    /// 布局文本，返回定位后的字形。
    pub fn layout_text(
        &self,
        font: &FontHandle,
        text: &str,
        opts: &TextLayoutOptions,
    ) -> TextLayout {
        self.text_backend.layout_text(font, text, opts)
    }

    /// 光栅化一个字形。
    pub fn rasterize_glyph(
        &self,
        font: &FontHandle,
        glyph_id: u32,
        pixel_size: f32,
    ) -> GlyphRaster {
        self.text_backend
            .rasterize_glyph(font, glyph_id, pixel_size)
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
            let layout = self.text_backend.layout_text(font, text, opts);
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
        let layout = self.text_backend.layout_text(font, text, opts);
        if char_index < layout.glyphs.len() {
            return layout.glyphs[char_index].x;
        }
        layout.glyphs.last().map_or(0.0, |g| g.x + g.width.max(0.0))
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
            .field("fonts_loaded", &self.loaded_font_handle)
            .finish()
    }
}

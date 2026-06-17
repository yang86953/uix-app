//! FontdueBackend — `TextBackend` implementation using the `fontdue` crate.
//!
//! Supports TrueType outlines (`glyf`/`loca`) and CFF1 PostScript outlines.
//! Does **not** support CFF2 variable fonts (`.ttc` with OTTO sfVersion)
//! because fontdue 0.9 does not process the `gvar`/`CFF2` variation tables.
//! Use a `FreeTypeBackend` (future) for complete format coverage.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::diag::{Errc, Error};
use crate::graphics::text_backend::{
    GlyphRaster, LineInfo, LineMetrics, PositionedGlyph, TextBackend, TextLayout, TextLayoutOptions,
};
use crate::graphics::FontHandle;
use fontdue::layout::*;

/// Glyph 位图缓存键：在同一字体文件中，(glyph_id, pixel_size) 唯一确定一个字形位图。
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct GlyphCacheKey {
    glyph_id: u32,
    /// 取整的像素尺寸（fontdue 按整数 px 渲染，floating 会重光栅化）
    pixel_size: u32,
}

/// 缓存的字形位图，与 `GlyphRaster` 结构相同但去掉了泛化包装。
struct CachedGlyph {
    width: usize,
    height: usize,
    coverage: Arc<Vec<u8>>,
}

/// Internal slot for a loaded font.
struct FontSlot {
    handle: FontHandle,
    font: fontdue::Font,
    /// 字体原始字节数据，用于检查覆盖范围和序列化等。
    raw_data: Vec<u8>,
}

/// Fontdue-based text backend.
///
/// Stores parsed fonts in an internal `Vec`. Each `FontHandle` is an index
/// into this vector. Thread-safe after construction (all methods take `&self`
/// except `load_font`/`unload_font` which require `&mut self`).
///
/// 内置 glyph 位图缓存（LRU），避免每帧重复调用 fontdue `rasterize_indexed`。
/// 缓存按 font_index 分桶，unload_font 时自动清理对应条目。
///
/// 支持字体回退链：`fallback_indices` 中的字体在主字体缺少字形时被依次尝试。
/// fontdue 的 `Layout::append` 原生支持多字体回退。
pub struct FontdueBackend {
    fonts: Vec<FontSlot>,
    /// (font_index → map_of_key_to_cached_glyph)
    glyph_cache: Mutex<HashMap<usize, HashMap<GlyphCacheKey, CachedGlyph>>>,
    /// 每个 font 缓存上限，超过时清空该 font 的缓存
    max_cache_per_font: usize,
    /// 字体回退链：主字体中缺失的字形将依次在这些字体中查找
    fallback_indices: Vec<usize>,
}

impl std::fmt::Debug for FontdueBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total_entries: usize = self
            .glyph_cache
            .lock()
            .map(|g| g.values().map(|m| m.len()).sum::<usize>())
            .unwrap_or(0);
        f.debug_struct("FontdueBackend")
            .field("fonts", &self.fonts.len())
            .field("cache_entries", &total_entries)
            .finish()
    }
}

impl Default for FontdueBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl FontdueBackend {
    pub fn new() -> Self {
        Self {
            fonts: Vec::new(),
            glyph_cache: Mutex::new(HashMap::new()),
            max_cache_per_font: 512,
            fallback_indices: Vec::new(),
        }
    }

    /// Run the fontdue Layout engine, returning positioned glyphs that share
    /// the same coordinate system (top-left origin, positive Y down).
    ///
    /// `fonts` 是多字体回退链，fontdue 会自动从第一个字体中查找字形，
    /// 如果缺失则依次尝试后续字体。
    fn run_layout(
        fonts: &[&fontdue::Font],
        text: &str,
        opts: &TextLayoutOptions,
        pos_x: f32,
        pos_y: f32,
    ) -> Layout<()> {
        if fonts.is_empty() {
            return Layout::new(CoordinateSystem::PositiveYDown);
        }
        let fs = opts.font_size.max(1.0);
        // 使用第一个字体的行高度量（主字体决定排版基线）
        let new_line_size = fonts[0]
            .horizontal_line_metrics(fs)
            .map(|m| m.new_line_size)
            .unwrap_or(fs * 1.3);
        let lh_abs = if opts.line_height > 0.0 {
            opts.line_height
        } else {
            new_line_size
        };
        let lh_mult = lh_abs / new_line_size;
        let max_w = if opts.max_width.is_finite() && opts.max_width > 0.0 {
            Some(opts.max_width)
        } else {
            None
        };
        let max_h = if opts.max_height > 0.0 {
            Some(opts.max_height)
        } else {
            None
        };

        let h_align = match opts.h_align {
            crate::graphics::HAlign::Left => HorizontalAlign::Left,
            crate::graphics::HAlign::Center => HorizontalAlign::Center,
            crate::graphics::HAlign::Right => HorizontalAlign::Right,
        };
        let v_align = match opts.v_align {
            crate::graphics::VAlign::Top => VerticalAlign::Top,
            crate::graphics::VAlign::Middle => VerticalAlign::Middle,
            crate::graphics::VAlign::Bottom => VerticalAlign::Bottom,
            crate::graphics::VAlign::Baseline => VerticalAlign::Top,
        };
        let wrap = if opts.word_wrap {
            WrapStyle::Word
        } else {
            WrapStyle::Letter
        };

        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: pos_x,
            y: pos_y,
            max_width: max_w,
            max_height: max_h,
            horizontal_align: h_align,
            vertical_align: v_align,
            line_height: lh_mult,
            wrap_style: wrap,
            wrap_hard_breaks: true,
        });
        layout.append(fonts, &TextStyle::new(text, fs, 0));
        layout
    }
}

impl TextBackend for FontdueBackend {
    fn load_font(&mut self, data: &[u8]) -> Result<FontHandle, Error> {
        let font = fontdue::Font::from_bytes(data, fontdue::FontSettings::default())
            .map_err(|e| Error::new(Errc::FormatError, format!("fontdue parse failed: {}", e)))?;
        let idx = self.fonts.len() as u32;
        self.fonts.push(FontSlot {
            handle: FontHandle::new(idx),
            font,
            raw_data: data.to_vec(),
        });
        Ok(FontHandle::new(idx))
    }

    fn unload_font(&mut self, handle: &FontHandle) {
        let idx = handle.0 as usize;
        if idx < self.fonts.len() {
            // 清理该字体的 glyph 缓存
            if let Ok(mut cache) = self.glyph_cache.lock() {
                cache.remove(&idx);
            }
            // Replace with a placeholder to keep indices stable.
            // This prevents handle reuse from silently pointing to a different font.
            self.fonts[idx] = FontSlot {
                handle: FontHandle::new(u32::MAX),
                font: fontdue::Font::from_bytes(&[] as &[u8], fontdue::FontSettings::default())
                    .unwrap_or_else(|_| {
                        panic!("fontdue_backend: failed to create placeholder font")
                    }),
                raw_data: Vec::new(),
            };
        }
    }

    fn is_valid(&self, handle: &FontHandle) -> bool {
        let idx = handle.0 as usize;
        idx < self.fonts.len() && self.fonts[idx].handle.0 != u32::MAX
    }

    fn has_glyph(&self, font: &FontHandle, ch: char) -> bool {
        let idx = font.0 as usize;
        // 检查主字体
        if let Some(slot) = self.fonts.get(idx) {
            if slot.handle.0 != u32::MAX && slot.font.lookup_glyph_index(ch) > 0 {
                return true;
            }
        }
        // 依次检查回退字体
        for &fi in &self.fallback_indices {
            if fi == idx {
                continue;
            }
            if let Some(slot) = self.fonts.get(fi) {
                if slot.handle.0 != u32::MAX && slot.font.lookup_glyph_index(ch) > 0 {
                    return true;
                }
            }
        }
        false
    }

    fn layout_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> TextLayout {
        let idx = font.0 as usize;
        let data = match self.fonts.get(idx) {
            Some(d) => d,
            None => {
                return TextLayout {
                    glyphs: Vec::new(),
                    lines: Vec::new(),
                    width: 0.0,
                    height: 0.0,
                };
            }
        };

        // 构建字体回退链：主字体 + 回退字体列表
        let mut all_fonts: Vec<&fontdue::Font> =
            Vec::with_capacity(1 + self.fallback_indices.len());
        all_fonts.push(&data.font);
        for &fi in &self.fallback_indices {
            if fi != idx {
                if let Some(slot) = self.fonts.get(fi) {
                    if slot.handle.0 != u32::MAX {
                        all_fonts.push(&slot.font);
                    }
                }
            }
        }

        // 构建 font_hash → all_fonts 索引的映射，用于布局后识别 glyph 来自哪个字体
        let hash_to_font_idx: std::collections::HashMap<u64, usize> = all_fonts
            .iter()
            .enumerate()
            .map(|(i, &f)| (f.file_hash() as u64, i))
            .collect();

        let layout = Self::run_layout(&all_fonts, text, opts, 0.0, 0.0);
        let gp = layout.glyphs();
        let glyphs: Vec<PositionedGlyph> = gp
            .iter()
            .map(|g| {
                // 将字体索引编码到 glyph_id 高位字节中，以便光栅化时路由到正确的回退字体
                // 编码格式：高位字节 = all_fonts 中的索引，低位 = 字形索引
                let fi = hash_to_font_idx
                    .get(&(g.key.font_hash as u64))
                    .copied()
                    .unwrap_or(0) as u32;
                let gi = u32::from(g.key.glyph_index);
                let encoded = if fi == 0 { gi } else { (fi << 24) | gi };
                PositionedGlyph {
                    x: g.x,
                    y: g.y,
                    width: g.width as f32,
                    height: g.height as f32,
                    glyph_id: encoded,
                }
            })
            .collect();

        // 提取行信息
        let mut lines = Vec::new();
        if let Some(g_lines) = layout.lines() {
            let mut glyph_idx = 0;
            for line in g_lines {
                let count = line.glyph_end - line.glyph_start;
                let start = glyph_idx;
                // 计算该行宽度：遍历 glyph 找到最大 x
                let mut w = 0.0f32;
                for gi in start..start + count {
                    if gi < glyphs.len() {
                        let gx = glyphs[gi].x + glyphs[gi].width.max(0.0);
                        w = w.max(gx);
                    }
                }
                lines.push(LineInfo {
                    y: line.baseline_y - line.max_ascent,
                    height: line.max_new_line_size,
                    width: w,
                    start_char: 0,
                    end_char: 0,
                    glyph_start: start,
                    glyph_count: count,
                });
                glyph_idx += count;
            }
        }

        let max_x = glyphs
            .iter()
            .fold(0.0f32, |m, g| (g.x + g.width.max(0.0)).max(m));

        TextLayout {
            width: max_x,
            height: opts.font_size.max(0.0),
            glyphs,
            lines,
        }
    }

    fn rasterize_glyph(&self, font: &FontHandle, glyph_id: u32, pixel_size: f32) -> GlyphRaster {
        // 从 glyph_id 高位字节解码回退字体索引
        // 编码格式：高位字节 = all_fonts 中的索引，低位 = glyph index
        let fb_offset = (glyph_id >> 24) as u8;
        let actual_glyph_id = (glyph_id & 0x00FFFFFF) as u16;

        let idx = if fb_offset == 0 {
            font.0 as usize // 主字体
        } else {
            let fi = (fb_offset as usize).wrapping_sub(1);
            if fi < self.fallback_indices.len() {
                self.fallback_indices[fi]
            } else {
                font.0 as usize // 回退索引无效，兜底到主字体
            }
        };

        let data = match self.fonts.get(idx) {
            Some(d) => d,
            None => {
                return GlyphRaster {
                    width: 0,
                    height: 0,
                    coverage: Arc::new(Vec::new()),
                };
            }
        };

        // Glyph 位图缓存：pixel_size 取整后做 key，包含编码后的 glyph_id 以保证唯一性
        let ps_int = pixel_size.round() as u32;
        if ps_int > 0 {
            let key = GlyphCacheKey {
                glyph_id,
                pixel_size: ps_int,
            };
            if let Ok(mut cache) = self.glyph_cache.lock() {
                let font_cache = cache.entry(idx).or_insert_with(HashMap::new);
                if let Some(cached) = font_cache.get(&key) {
                    return GlyphRaster {
                        width: cached.width,
                        height: cached.height,
                        coverage: Arc::clone(&cached.coverage),
                    };
                }
                // 缓存未命中：使用解码后的 actual_glyph_id 光栅化
                let glyph_index = actual_glyph_id.min(data.font.glyph_count() - 1);
                let (metrics, coverage) = data.font.rasterize_indexed(glyph_index, pixel_size);
                let coverage = Arc::new(coverage);
                let entry = CachedGlyph {
                    width: metrics.width,
                    height: metrics.height,
                    coverage: Arc::clone(&coverage),
                };
                // 缓存超限时清空该 font 的缓存（简单 FIFO 淘汰）
                if font_cache.len() >= self.max_cache_per_font {
                    font_cache.clear();
                }
                font_cache.insert(key, entry);
                return GlyphRaster {
                    width: metrics.width,
                    height: metrics.height,
                    coverage,
                };
            }
        }

        // 兜底：缓存不可用时直接光栅化
        let glyph_index = actual_glyph_id.min(data.font.glyph_count() - 1);
        let (metrics, coverage) = data.font.rasterize_indexed(glyph_index, pixel_size);
        GlyphRaster {
            width: metrics.width,
            height: metrics.height,
            coverage: Arc::new(coverage),
        }
    }

    fn set_fallback_fonts(&mut self, fallback_handles: &[FontHandle]) {
        self.fallback_indices = fallback_handles
            .iter()
            .map(|h| h.0 as usize)
            .filter(|&idx| idx < self.fonts.len() && self.fonts[idx].handle.0 != u32::MAX)
            .collect();
    }

    fn horizontal_line_metrics(&self, font: &FontHandle, pixel_size: f32) -> Option<LineMetrics> {
        let idx = font.0 as usize;
        let data = self.fonts.get(idx)?;
        data.font
            .horizontal_line_metrics(pixel_size)
            .map(|m| LineMetrics {
                ascent: m.ascent,
                descent: m.descent,
                new_line_size: m.new_line_size,
            })
    }

    fn font_data(&self, font: &FontHandle) -> Option<Vec<u8>> {
        let idx = font.0 as usize;
        let slot = self.fonts.get(idx)?;
        if slot.handle.0 == u32::MAX {
            return None;
        }
        Some(slot.raw_data.clone())
    }
}

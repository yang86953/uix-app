//! ab_glyph 后端：纯 advance 定位。字形缓存已统一移到 FontService。

use crate::core::{Errc, Error};
use crate::draw::resources::font::text_backend::{self, *};
use crate::draw::FontHandle;
use crate::draw::TextBackend;
use ab_glyph::*;
use std::sync::Arc;
pub(crate) use text_backend::TextLayoutOptions;
use text_backend::{TOFU_GLYPH_ID, WHITESPACE_GLYPH_ID};

/// 字体槽位：`font` 借用 `_data` 的内容（借用先声明先 drop，安全）。
///
/// 数据存放位置在堆上（Box/Arc），`Vec<FontSlot>` 扩容移动本结构体时
/// 只移动指针/句柄，数据地址不变，借用始终有效。
pub(crate) struct FontSlot {
    handle: FontHandle,
    font: Option<FontRef<'static>>,
    /// 字体文件字节：mmap（惰性分页，中文字体常驻收益）或 Arc（用户 Vec 数据）。
    _data: Option<FontData>,
}

/// 字体字节的所有权来源。
pub(crate) enum FontData {
    /// 内存映射文件：仅实际触达的字形页驻留 working set。
    Mapped(Box<memmap2::Mmap>),
    /// 用户提供或 API 兼容路径拷贝的数据。
    Owned(Arc<[u8]>),
}

impl FontSlot {
    /// 从自有数据构造借用槽位。
    ///
    /// # Safety
    ///
    /// `font` 必须以 `&data[..]` 为源创建（`FontRef::try_from_slice_and_index`），
    /// 且 data 由本槽位持有（堆上，地址稳定）；字段声明顺序保证 `font` 先于
    /// `_data` 释放，借用不会悬垂。
    unsafe fn new_borrowed(handle: FontHandle, font: FontRef<'static>, data: FontData) -> Self {
        Self {
            handle,
            // 将借用字体包在 Option 中，卸载时可以先结束借用再释放数据。
            font: Some(font),
            // 将字体数据包在 Option 中，卸载时释放 mmap 或 Arc 的所有权。
            _data: Some(data),
        }
    }
}

pub struct AbGlyphBackend {
    pub(crate) fonts: Vec<FontSlot>,
}

impl std::fmt::Debug for AbGlyphBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AbGlyphBackend")
            .field("fonts", &self.fonts.len())
            .finish()
    }
}

impl Default for AbGlyphBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn padded_area_coverage(
    outlined: &OutlinedGlyph,
    width: usize,
    height: usize,
) -> Option<Arc<[u8]>> {
    let bounds = outlined.px_bounds();
    let inner_w = bounds.width() as usize;
    let inner_h = bounds.height() as usize;
    let pad = crate::draw::resources::font::glyph_outline::ATLAS_PAD;
    let expected_w = inner_w.checked_add(pad.checked_mul(2)?)?;
    let expected_h = inner_h.checked_add(pad.checked_mul(2)?)?;
    if width != expected_w || height != expected_h {
        return None;
    }

    let pixel_count = width.checked_mul(height)?;
    let mut coverage = Vec::new();
    if coverage.try_reserve_exact(pixel_count).is_err() {
        return None;
    }
    coverage.resize(pixel_count, 0u8);
    outlined.draw(|x, y, cov| {
        let Some(dst_x) = (x as usize).checked_add(pad) else {
            return;
        };
        let Some(dst_y) = (y as usize).checked_add(pad) else {
            return;
        };
        if dst_x < width && dst_y < height {
            coverage[dst_y * width + dst_x] = (cov * 255.0).clamp(0.0, 255.0) as u8;
        }
    });
    Some(coverage.into())
}

impl AbGlyphBackend {
    pub fn new() -> Self {
        Self { fonts: vec![] }
    }
    fn idx(&self, h: &FontHandle) -> Option<usize> {
        let i = h.0 as usize;
        self.fonts
            .get(i)
            .filter(|slot| slot.handle.0 == h.0)
            .map(|_| i)
    }
}

impl TextBackend for AbGlyphBackend {
    fn load_font(&mut self, data: &[u8]) -> Result<FontHandle, Error> {
        // 对 TTC/OTC 保留完整文件并选择首个 face。集合内表可跨 face 共享，
        // 不能把 offset 区间切成伪 TTF 后再解析。
        let id = self.fonts.len() as u32;
        // SAFETY: f 借用 data 的内容；data 复制进 Arc 由槽位持有（见 new_borrowed 契约）。
        let data: Arc<[u8]> = Arc::from(data);
        let f = FontRef::try_from_slice_and_index(&data, 0)
            .map_err(|e| Error::new(Errc::FormatError, format!("ab_glyph: {:?}", e)))?;
        let f = unsafe {
            std::mem::transmute::<FontRef<'_>, FontRef<'static>>(f)
        };
        self.fonts.push(unsafe {
            FontSlot::new_borrowed(FontHandle::new(id), f, FontData::Owned(data))
        });
        Ok(FontHandle::new(id))
    }

    fn load_font_owned(&mut self, data: Vec<u8>) -> Result<FontHandle, Error> {
        let id = self.fonts.len() as u32;
        // SAFETY: f 借用 data 的内容；data 由槽位持有（见 new_borrowed 契约）。
        let data: Arc<[u8]> = Arc::from(data);
        let f = FontRef::try_from_slice_and_index(&data, 0)
            .map_err(|e| Error::new(Errc::FormatError, format!("ab_glyph: {:?}", e)))?;
        let f = unsafe {
            std::mem::transmute::<FontRef<'_>, FontRef<'static>>(f)
        };
        self.fonts.push(unsafe {
            FontSlot::new_borrowed(FontHandle::new(id), f, FontData::Owned(data))
        });
        Ok(FontHandle::new(id))
    }

    fn load_font_mapped(&mut self, mmap: memmap2::Mmap) -> Result<FontHandle, Error> {
        let boxed = Box::new(mmap);
        let f = FontRef::try_from_slice_and_index(boxed.as_ref(), 0)
            .map_err(|e| Error::new(Errc::FormatError, format!("ab_glyph: {:?}", e)))?;
        let id = self.fonts.len() as u32;
        // SAFETY: f 借用 boxed 的映射内容；boxed 由槽位持有，堆地址稳定（见 new_borrowed 契约）。
        let f = unsafe {
            std::mem::transmute::<FontRef<'_>, FontRef<'static>>(f)
        };
        self.fonts.push(unsafe {
            FontSlot::new_borrowed(FontHandle::new(id), f, FontData::Mapped(boxed))
        });
        Ok(FontHandle::new(id))
    }

    fn unload_font(&mut self, handle: &FontHandle) {
        let i = handle.0 as usize;
        if i < self.fonts.len() {
            let slot = &mut self.fonts[i];
            // 先销毁借用字体，确保其底层数据仍然存活到借用结束。
            let font = slot.font.take();
            drop(font);
            // 再释放内存映射或自有字节，避免卸载后继续占用 private bytes。
            let data = slot._data.take();
            drop(data);
            // 最后标记句柄无效，保留槽位编号以维持句柄稳定性。
            slot.handle = FontHandle::new(u32::MAX);
        }
    }

    fn is_valid(&self, h: &FontHandle) -> bool {
        self.idx(h).is_some()
    }

    fn has_glyph(&self, font: &FontHandle, ch: char) -> bool {
        self.idx(font)
            .and_then(|i| self.fonts[i].font.as_ref())
            .is_some_and(|font| font.glyph_id(ch) != GlyphId(0))
    }

    fn layout_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> TextLayout {
        let Some(idx) = self.idx(font) else {
            return TextLayout {
                glyphs: vec![],
                lines: vec![],
                width: 0.0,
                height: 0.0,
            };
        };
        // 有效句柄必然对应仍存活的字体借用。
        let f = self.fonts[idx]
            .font
            .as_ref()
            .expect("valid font slot must retain its parsed font");
        let fs = text_backend::bounded_font_size(opts.font_size);
        let sf = f.as_scaled(PxScale { x: fs, y: fs });

        let asc = sf.ascent();
        let desc = sf.descent();
        let lg = sf.line_gap();
        let font_h = (asc - desc + lg).max(fs);
        let line_h = if opts.line_height > 0.0 {
            opts.line_height
        } else {
            font_h
        };
        let max_w = opts.max_width.is_finite() && opts.max_width > 0.0;
        let tofu_adv = (fs * 0.55).max(4.0);
        let space_id = f.glyph_id(' ');
        let space_adv = if space_id == GlyphId(0) {
            (fs * 0.35).max(1.0)
        } else {
            sf.h_advance(space_id)
        };

        let mut out = Vec::new();
        let mut cx = 0.0f32;
        let mut cy = asc;
        let mut prev = GlyphId(0);
        let mut char_index = 0usize;

        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\r' {
                let break_chars = if chars.peek() == Some(&'\n') {
                    chars.next();
                    2
                } else {
                    1
                };
                cx = 0.0;
                cy += line_h;
                prev = GlyphId(0);
                char_index += break_chars;
                continue;
            }
            if ch == '\n' {
                cx = 0.0;
                cy += line_h;
                prev = GlyphId(0);
                char_index += 1;
                continue;
            }
            let gid = f.glyph_id(ch);
            let (glyph_id, adv) = if ch == '\t' {
                (WHITESPACE_GLYPH_ID, space_adv * 4.0)
            } else if ch == '\u{200b}' {
                (WHITESPACE_GLYPH_ID, 0.0)
            } else if ch.is_whitespace() && gid == GlyphId(0) {
                (WHITESPACE_GLYPH_ID, space_adv)
            } else if gid == GlyphId(0) {
                // 缺字：保留占位 advance，避免字符消失与索引错位
                (TOFU_GLYPH_ID, tofu_adv)
            } else {
                if prev != GlyphId(0) {
                    cx += sf.kern(prev, gid);
                }
                (gid.0 as u32, sf.h_advance(gid))
            };

            if max_w && cx > 0.0 && cx + adv > opts.max_width {
                cx = 0.0;
                cy += line_h;
            }

            out.push(PositionedGlyph {
                x: cx,
                y: cy,
                width: adv,
                height: font_h,
                glyph_id,
                char_index,
                font: *font,
            });

            cx += adv;
            prev = if matches!(glyph_id, TOFU_GLYPH_ID | WHITESPACE_GLYPH_ID) {
                GlyphId(0)
            } else {
                gid
            };
            char_index += 1;
        }

        let text_h = cy - asc + font_h;
        let max_h = if opts.max_height > 0.0 {
            opts.max_height
        } else {
            text_h
        };
        let v_off = if max_h > text_h {
            match opts.v_align {
                crate::draw::VAlign::Top | crate::draw::VAlign::Baseline => 0.0,
                crate::draw::VAlign::Middle => (max_h - text_h) * 0.5,
                crate::draw::VAlign::Bottom => max_h - text_h,
            }
        } else {
            0.0
        };

        for g in out.iter_mut() {
            g.y += v_off;
        }

        let tw = out.iter().fold(0.0f32, |m, g| (g.x + g.width).max(m));
        let n_glyphs = out.len();
        TextLayout {
            glyphs: out,
            lines: vec![LineInfo {
                y: 0.0,
                height: max_h.max(text_h),
                width: tw,
                start_char: 0,
                end_char: text.chars().count(),
                glyph_start: 0,
                glyph_count: n_glyphs,
            }],
            width: tw,
            height: max_h.max(text_h),
        }
    }

    fn rasterize_glyph(&self, font: &FontHandle, glyph_id: u32, pixel_size: f32) -> GlyphRaster {
        if glyph_id == WHITESPACE_GLYPH_ID {
            return GlyphRaster::empty();
        }
        let Some(pixel_size) = text_backend::normalized_raster_pixel_size(pixel_size) else {
            return GlyphRaster::empty();
        };
        let Some(idx) = self.idx(font) else {
            return GlyphRaster::empty();
        };
        let pixel_size = pixel_size as f32;
        let gid = GlyphId(glyph_id as u16);
        // 有效句柄必然对应仍存活的字体借用。
        let f = self.fonts[idx]
            .font
            .as_ref()
            .expect("valid font slot must retain its parsed font");
        let glyph = gid.with_scale_and_position(pixel_size, point(0.0, 0.0));
        let scale_factor = f
            .as_scaled(PxScale {
                x: pixel_size,
                y: pixel_size,
            })
            .scale_factor();
        let glyph_position = glyph.position;
        let outlined = f.outline_glyph(glyph);
        // 同一份原始轮廓同时生成两种数据：字体光栅器的真实面积覆盖率供近
        // 1:1 R8 使用，展平边列表供缩放/仿射 MSDF 使用。
        if let (Some(outline), Some(outlined)) = (f.outline(gid), outlined.as_ref()) {
            let px_bounds = outlined.px_bounds();
            if let Some((w, h, bx, by, mesh)) =
                crate::draw::resources::font::glyph_outline::mesh_from_outline(
                    &outline,
                    scale_factor,
                    px_bounds,
                    glyph_position,
                )
            {
                let coverage =
                    padded_area_coverage(outlined, w, h).unwrap_or_else(|| Arc::<[u8]>::from([]));
                return GlyphRaster {
                    width: w,
                    height: h,
                    coverage,
                    bearing_x: bx,
                    bearing_y: by,
                    outline_mesh: Some(mesh),
                };
            }
        }
        // 回退：无轮廓或展平失败时仍走 ab_glyph 面积 coverage。
        let (w, h, data, bx, by) = if let Some(o) = outlined {
            let b = o.px_bounds();
            let bw = (b.max.x - b.min.x).ceil() as usize;
            let bh = (b.max.y - b.min.y).ceil() as usize;
            let bearing_x = b.min.x;
            let bearing_y = b.min.y;
            if let Some(pixel_count) = bw.checked_mul(bh).filter(|count| *count > 0) {
                let mut p = Vec::new();
                if p.try_reserve_exact(pixel_count).is_err() {
                    return GlyphRaster::empty();
                }
                p.resize(pixel_count, 0u8);
                o.draw(|x, y, cov| {
                    if (x as usize) < bw && (y as usize) < bh {
                        p[y as usize * bw + x as usize] = (cov * 255.0) as u8;
                    }
                });
                (bw, bh, p, bearing_x, bearing_y)
            } else {
                (0, 0, vec![], 0.0, 0.0)
            }
        } else {
            (0, 0, vec![], 0.0, 0.0)
        };

        GlyphRaster {
            width: w,
            height: h,
            coverage: Arc::<[u8]>::from(data),
            bearing_x: bx,
            bearing_y: by,
            outline_mesh: None,
        }
    }

    fn horizontal_line_metrics(&self, font: &FontHandle, pixel_size: f32) -> Option<LineMetrics> {
        let i = self.idx(font)?;
        let pixel_size = text_backend::normalized_raster_pixel_size(pixel_size)? as f32;
        // 有效句柄必然对应仍存活的字体借用。
        let sc = self.fonts[i]
            .font
            .as_ref()
            .expect("valid font slot must retain its parsed font")
            .as_scaled(PxScale {
                x: pixel_size,
                y: pixel_size,
            });
        Some(LineMetrics {
            ascent: sc.ascent(),
            descent: -sc.descent(),
            new_line_size: sc.height(),
        })
    }

    fn font_data(&self, font: &FontHandle) -> Option<Vec<u8>> {
        let i = self.idx(font)?;
        // 只为仍有效的字体复制底层数据，卸载后的槽位不再暴露内容。
        if self.fonts[i].handle.0 == u32::MAX {
            return None;
        }
        // 读取与有效句柄绑定的底层数据所有权。
        let data = self.fonts[i]._data.as_ref()?;
        // 将映射或自有字节复制给调用方，保持后端所有权不变。
        Some(match data {
            FontData::Mapped(m) => m.as_ref().to_vec(),
            FontData::Owned(a) => a.to_vec(),
        })
    }

    fn clear_cache(&mut self) { /* 缓存已统一在 FontService 层 */
    }

    fn memory_usage(&self) -> usize {
        let mut t = 0usize;
        for s in &self.fonts {
            if s.handle.0 != u32::MAX {
                // 只统计仍有效槽位的底层数据，避免掩盖卸载残留。
                if let Some(data) = s._data.as_ref() {
                    t += match data {
                        FontData::Mapped(m) => m.len(),
                        FontData::Owned(a) => a.len(),
                    };
                }
            }
        }
        t
    }
}

#[cfg(test)]
mod tests {
    // 引入被测后端，验证卸载边界而不依赖应用初始化。
    use super::AbGlyphBackend;
    // 引入字体后端 trait，使测试可以调用加载、卸载和内存统计接口。
    use crate::draw::TextBackend;

    #[test]
    fn unload_releases_owned_font_data() {
        // 使用仓库内稳定的 Lucide 字体作为最小可解析输入。
        let data = include_bytes!("../../../../../assets/fonts/lucide.ttf");
        // 创建独立后端，避免测试之间共享字体槽位。
        let mut backend = AbGlyphBackend::new();
        // 加载字体并记录稳定句柄。
        let handle = backend
            .load_font(data)
            .expect("Lucide font must load in the ab_glyph backend");
        // 加载后底层字体数据必须计入后端内存统计。
        assert!(backend.memory_usage() > 0);
        // 卸载字体应释放底层数据而不是只使句柄失效。
        backend.unload_font(&handle);
        // 卸载后句柄不可用，避免继续访问已释放的借用。
        assert!(!backend.is_valid(&handle));
        // 卸载后不应残留字体数据占用。
        assert_eq!(backend.memory_usage(), 0);
    }
}

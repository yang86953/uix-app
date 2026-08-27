//! ab_glyph 后端：纯 advance 定位。字形缓存已统一移到 FontService。

use crate::core::{Errc, Error};
use crate::draw::FontHandle;
use crate::draw::TextBackend;
use crate::draw::resources::font::text_backend::{self, *};
use ab_glyph::*;
use std::sync::Arc;
pub(crate) use text_backend::TextLayoutOptions;
use text_backend::{TOFU_GLYPH_ID, WHITESPACE_GLYPH_ID};

/// 字体槽位：两个字体面借用 `_data` 的内容。
///
/// 数据存放位置在堆上（Box/Arc），`Vec<FontSlot>` 扩容移动本结构体时
/// 只移动指针/句柄，数据地址不变，借用始终有效。
pub(crate) struct FontSlot {
    handle: FontHandle,
    font: Option<FontRef<'static>>,
    /// 与同一字体字节绑定的 OpenType shaping 面，随槽位卸载失效。
    shaping_face: Option<rustybuzz::Face<'static>>,
    /// 字体文件字节：mmap（惰性分页，中文字体常驻收益）或 Arc（用户 Vec 数据）。
    _data: Option<FontData>,
}

/// 字体字节的所有权来源。
pub(crate) enum FontData {
    /// 内存映射文件：仅实际触达的字形页驻留 working set。
    Mapped(Box<memmap2::Mmap>),
    /// 用户提供或 API 兼容路径拷贝的数据。
    Owned(Arc<[u8]>),
    /// 随应用二进制存活的静态字体数据，不占用启动期堆分配。
    Static(&'static [u8]),
}

// 为 shaping 提供不复制的字体字节视图。
impl FontData {
    // 返回当前所有权变体持有的完整字体文件。
    fn as_slice(&self) -> &[u8] {
        // 两种所有权都只借用底层稳定字节。
        match self {
            // 内存映射可直接解引用为字节切片。
            Self::Mapped(data) => data.as_ref(),
            // 共享所有权数据可直接解引用为字节切片。
            Self::Owned(data) => data.as_ref(),
            // 静态切片自身已经覆盖整个字体槽位生命周期。
            Self::Static(data) => data,
            // 结束所有权变体匹配。
        }
        // 结束字体字节视图方法。
    }
    // 结束字体数据辅助实现。
}

impl FontSlot {
    /// 从即将由本槽位唯一持有的数据同时构造两个借用字体面。
    fn parse(handle: FontHandle, data: FontData) -> Result<Self, Error> {
        // 两个解析器必须在同一封闭构造器内借用同一份数据，调用者无法错配来源。
        let font = FontRef::try_from_slice_and_index(data.as_slice(), 0)
            .map_err(|error| Error::new(Errc::FormatError, format!("ab_glyph: {error:?}")))?;
        // rustybuzz 只借用同一份稳定字节；解析失败保留既有逐字符回退语义。
        let shaping_face = rustybuzz::Face::from_slice(data.as_slice(), 0).map(|face| {
            // SAFETY: data 随槽位持有且晚于 shaping_face 释放，底层 Arc/mmap 地址稳定。
            unsafe { std::mem::transmute::<rustybuzz::Face<'_>, rustybuzz::Face<'static>>(face) }
        });
        // SAFETY: font 只借用 data 的堆中字节；Arc 与 mmap 的字节地址不随槽位移动。
        let font = unsafe { std::mem::transmute::<FontRef<'_>, FontRef<'static>>(font) };
        Ok(Self {
            handle,
            // 将借用字体包在 Option 中，卸载时可以先结束借用再释放数据。
            font: Some(font),
            // 保存一次解析后的共享 OpenType 表视图，避免每次动态布局重新扫描字体表。
            shaping_face,
            // 将字体数据包在 Option 中，卸载时释放 mmap 或 Arc 的所有权。
            _data: Some(data),
        })
    }

    /// 显式结束全部借用后再释放底层字节，供卸载与整体析构共用。
    fn release(&mut self) {
        // FontRef 先结束对字体表和预解析子表的借用。
        drop(self.font.take());
        // rustybuzz Face 随后结束对同一字体表的借用。
        drop(self.shaping_face.take());
        // 最后释放 Arc 或解除 mmap；此后槽位不再包含任何借用视图。
        drop(self._data.take());
    }
}

impl Drop for FontSlot {
    fn drop(&mut self) {
        // 即使后端整体销毁而未逐项卸载，也保持与显式卸载相同的释放顺序。
        self.release();
    }
}

/// 使用 `ab_glyph` 提供字体解析、布局与字形光栅化的文本后端。
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
    /// 创建尚未加载任何字体的文本后端。
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
        let data: Arc<[u8]> = Arc::from(data);
        // 槽位构造器从自有 Arc 内同时建立两个字体面，避免来源错配。
        self.fonts
            .push(FontSlot::parse(FontHandle::new(id), FontData::Owned(data))?);
        Ok(FontHandle::new(id))
    }

    fn load_font_owned(&mut self, data: Vec<u8>) -> Result<FontHandle, Error> {
        let id = self.fonts.len() as u32;
        let data: Arc<[u8]> = Arc::from(data);
        // 消费调用方 Vec 后由同一安全构造器封闭自引用不变式。
        self.fonts
            .push(FontSlot::parse(FontHandle::new(id), FontData::Owned(data))?);
        Ok(FontHandle::new(id))
    }

    // 从应用字体包的共享只读数据零复制创建后端字体槽位。
    fn load_font_shared(&mut self, data: Arc<[u8]>) -> Result<FontHandle, Error> {
        // 新句柄严格对应即将追加的后端槽位。
        let id = self.fonts.len() as u32;
        // 保存共享字节本身，并由槽位封闭两个借用面与 owner 的关系。
        self.fonts
            .push(FontSlot::parse(FontHandle::new(id), FontData::Owned(data))?);
        // 返回与追加槽位编号一致的稳定句柄。
        Ok(FontHandle::new(id))
    }

    // 直接借用 include_bytes! 等进程期静态资产，避免构造大型 Arc<[u8]>。
    fn load_font_static(&mut self, data: &'static [u8]) -> Result<FontHandle, Error> {
        // 新句柄严格对应即将追加的后端槽位。
        let id = self.fonts.len() as u32;
        // FontData::Static 以类型系统证明底层字节晚于所有字体面释放。
        self.fonts.push(FontSlot::parse(
            FontHandle::new(id),
            FontData::Static(data),
        )?);
        // 返回与追加槽位编号一致的稳定句柄。
        Ok(FontHandle::new(id))
    }

    fn load_font_mapped(&mut self, mmap: memmap2::Mmap) -> Result<FontHandle, Error> {
        let boxed = Box::new(mmap);
        let id = self.fonts.len() as u32;
        // mmap 所有权先进入统一构造器，解析失败也会在返回前安全解除映射。
        self.fonts.push(FontSlot::parse(
            FontHandle::new(id),
            FontData::Mapped(boxed),
        )?);
        Ok(FontHandle::new(id))
    }

    fn unload_font(&mut self, handle: &FontHandle) {
        if let Some(i) = self.idx(handle) {
            let slot = &mut self.fonts[i];
            // 统一释放函数显式保证 FontRef、Face、数据的实际释放顺序。
            slot.release();
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
        // 槽位字体若已失效，则沿用无效句柄的空布局语义。
        let Some(f) = self.fonts[idx].font.as_ref() else {
            // 返回稳定的空布局，禁止内部槽位漂移触发进程级 panic。
            return TextLayout {
                // 空布局不包含字形。
                glyphs: vec![],
                // 空布局不包含行。
                lines: vec![],
                // 空布局宽度为零。
                width: 0.0,
                // 空布局高度为零。
                height: 0.0,
            };
        };
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
        // 优先使用 OpenType shaping；解析失败时保留原有逐字符回退路径。
        if let Some(layout) = self.fonts[idx]
            // 只借用当前有效槽位在加载期解析的 OpenType 字体面。
            .shaping_face
            // 不支持 OpenType shaping 的字体保留逐字符回退路径。
            .as_ref()
            // 使用只读字体面执行本次文本 shaping。
            .and_then(|face| {
                // shaping 模块负责复杂脚本、cluster 与字形定位。
                super::shaping::layout_text_with_face(
                    // 字体面与 ab_glyph 使用同一首个 face，并由槽位统一拥有生命周期。
                    face,
                    // 保留字形所属字体句柄供后续光栅化。
                    *font,
                    // 传入本段原始 UTF-8 文本。
                    text,
                    // 复用调用方布局约束。
                    opts,
                    // 与后续 ab_glyph 光栅化共享同一设计单位缩放。
                    sf.h_scale_factor(),
                    // 复用 ab_glyph 的像素 ascent。
                    asc,
                    // 复用 ab_glyph 的实际字体行盒高度。
                    font_h,
                    // 复用调用方解析后的行高。
                    line_h,
                    // 独立后端入口保留首强字符自动方向推断。
                    None,
                )
            })
        // shaping 成功时直接返回 cluster 感知结果。
        {
            // 避免后续逐字符路径破坏复杂脚本定位。
            return layout;
            // 结束 shaping 快速路径。
        }
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
                // 逐字符回退路径的 cluster 只覆盖当前字符。
                char_end: char_index + 1,
                // 兼容布局默认使用 LTR，FontService 会回填行级双向级别。
                bidi_level: 0,
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

    // 使用段落级 UAX #9 方向执行单一视觉 run 的 OpenType shaping。
    fn layout_text_directional(
        // 借用字体后端。
        &self,
        // 指定当前 run 使用的字体句柄。
        font: &FontHandle,
        // 指定当前 run 的逻辑源文本。
        text: &str,
        // 复用调用方布局约束。
        opts: &TextLayoutOptions,
        // 使用 UAX #9 已解析方向覆盖局部猜测。
        direction: TextDirection,
    ) -> TextLayout {
        // 无效句柄沿用统一后端的空布局行为。
        let Some(idx) = self.idx(font) else {
            // 通过既有入口生成兼容结果。
            return self.layout_text(font, text, opts);
        };
        // 槽位字体若已失效，则回退到统一兼容布局入口。
        let Some(parsed_font) = self.fonts[idx].font.as_ref() else {
            // 保持无效字体句柄与方向布局的一致退化行为。
            return self.layout_text(font, text, opts);
        };
        // 约束异常字号以避免非有限 shaping 缩放。
        let font_size = text_backend::bounded_font_size(opts.font_size);
        // 建立与普通入口一致的像素缩放字体。
        let scaled_font = parsed_font.as_scaled(PxScale {
            // 水平方向使用统一字号。
            x: font_size,
            // 垂直方向使用统一字号。
            y: font_size,
        });
        // 读取像素 ascent 供 shaping 基线定位。
        let ascent = scaled_font.ascent();
        // 读取 descent 供行盒高度计算。
        let descent = scaled_font.descent();
        // 读取字体行间距。
        let line_gap = scaled_font.line_gap();
        // 形成与普通入口一致的有限字体行盒。
        let font_height = (ascent - descent + line_gap).max(font_size);
        // 优先使用调用方显式行高。
        let line_height = if opts.line_height > 0.0 {
            // 保留正显式行高。
            opts.line_height
        // 缺少显式行高时使用字体自然行盒。
        } else {
            // 返回字体自然高度。
            font_height
        };
        // 尝试使用同一字体数据执行显式方向 shaping。
        let shaped = self.fonts[idx]
            // 借用加载期解析且与句柄同生命周期的 OpenType 字体面。
            .shaping_face
            // 不支持 OpenType shaping 时保留统一兼容布局。
            .as_ref()
            // 对缓存的只读字体面执行显式方向 shaping。
            .and_then(|face| {
                // 调用共享 OpenType shaping 实现。
                super::shaping::layout_text_with_face(
                    // 传入与 ab_glyph 句柄绑定的同一字体面。
                    face,
                    // 保留稳定字体句柄。
                    *font,
                    // 传入当前单向 run 文本。
                    text,
                    // 复用布局约束。
                    opts,
                    // 与后续 ab_glyph 光栅化共享同一设计单位缩放。
                    scaled_font.h_scale_factor(),
                    // 传入像素 ascent。
                    ascent,
                    // 传入字体行盒高度。
                    font_height,
                    // 传入解析后的行高。
                    line_height,
                    // 显式覆盖 rustybuzz 的局部方向猜测。
                    Some(direction),
                )
            });
        // shaping 成功时直接返回视觉字形与逻辑 cluster。
        if let Some(layout) = shaped {
            // 返回显式方向布局。
            return layout;
        }
        // 控制字符或异常字体数据使用既有兼容布局，后续仍由 FontService 重排。
        self.layout_text(font, text, opts)
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
        // 槽位字体若已失效，则返回稳定的空栅格。
        let Some(f) = self.fonts[idx].font.as_ref() else {
            // 禁止内部槽位漂移触发进程级 panic。
            return GlyphRaster::empty();
        };
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
        // 槽位字体若已失效，则按 Option 契约返回无指标。
        let sc = self.fonts[i].font.as_ref()?.as_scaled(PxScale {
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
            FontData::Static(data) => data.to_vec(),
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
                        FontData::Static(data) => data.len(),
                    };
                }
            }
        }
        t
    }
}

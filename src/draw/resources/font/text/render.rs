//! TextRenderService — 文本渲染服务。

use crate::core::{Point, Rect, Size};
use crate::draw::Canvas2D;
use crate::draw::geometry::spatial::{AABB3D, PhysicalUnit, SpatialContext, Vec3};
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::font::text_backend::TextLayoutOptions;
use crate::draw::{Color, FontHandle, HAlign, VAlign};

/// 文本渲染服务 — 字体管理、文本布局、glyph 光栅化。
pub struct TextRenderService<'a> {
    /// 当前用于布局和绘制文本的字体句柄。
    pub font: FontHandle,
    /// 提供字体解析、布局与字形缓存的字体服务。
    pub font_service: &'a FontService,
    /// 单次文本布局允许使用的最大宽度。
    pub max_text_width: f32,
}

impl<'a> TextRenderService<'a> {
    /// 使用指定字体服务、字体和最大文本宽度创建渲染服务。
    pub fn new(font: FontHandle, font_service: &'a FontService, max_text_width: f32) -> Self {
        Self {
            font,
            font_service,
            max_text_width,
        }
    }

    /// 更新字体句柄。
    pub fn set_font(&mut self, font: FontHandle) {
        self.font = font;
    }

    /// 设置文本绘制最大宽度。
    pub fn set_max_text_width(&mut self, width: f32) {
        self.max_text_width = width;
    }

    // ── 内部辅助 ──

    fn text_opts(
        &self,
        font_size: f32,
        max_width: f32,
        max_height: f32,
        word_wrap: bool,
        h_align: HAlign,
        v_align: VAlign,
    ) -> TextLayoutOptions {
        let font_size = crate::draw::resources::font::text_backend::bounded_font_size(font_size);
        TextLayoutOptions {
            max_width,
            max_height,
            line_height: font_size * 1.5,
            word_wrap,
            h_align,
            v_align,
            font_size,
        }
    }

    fn blit_glyph_layout(
        &mut self,
        canvas: &mut dyn Canvas2D,
        layout: &crate::draw::resources::font::text_backend::TextLayout,
        pos: Point,
        color: Color,
        font_size: f32,
    ) {
        let fs = crate::draw::resources::font::text_backend::bounded_font_size(font_size);
        for gp in &layout.glyphs {
            let fh = if gp.font.0 != u32::MAX {
                gp.font
            } else {
                self.font
            };
            let raster = self.font_service.rasterize_glyph(&fh, gp.glyph_id, fs);
            if raster.width == 0 || raster.height == 0 {
                continue;
            }
            let gx = (pos.x + gp.x + raster.bearing_x) as i32;
            let gy = (pos.y + gp.y + raster.bearing_y) as i32;
            if let Some(mesh) = raster
                .outline_mesh
                .as_ref()
                .filter(|m| crate::draw::resources::font::glyph_outline::is_outline_edges(m))
            {
                let area_coverage =
                    if raster.coverage.len() >= raster.width.saturating_mul(raster.height) {
                        Some(std::sync::Arc::clone(&raster.coverage))
                    } else {
                        None
                    };
                canvas.blit_glyph_outline_shared(
                    gx,
                    gy,
                    std::sync::Arc::clone(mesh),
                    area_coverage,
                    raster.width,
                    raster.height,
                    color,
                );
            } else if !raster.coverage.is_empty() {
                canvas.blit_glyph_shared(
                    gx,
                    gy,
                    std::sync::Arc::clone(&raster.coverage),
                    raster.width,
                    raster.height,
                    color,
                );
            }
        }
    }

    /// 公开的 glyph 绘制入口（供 PaintContext 委托）。
    pub fn blit_to(
        &mut self,
        canvas: &mut dyn Canvas2D,
        layout: &crate::draw::resources::font::text_backend::TextLayout,
        pos: Point,
        color: Color,
        font_size: f32,
    ) {
        self.blit_glyph_layout(canvas, layout, pos, color, font_size);
    }

    /// 单行行盒高度 = ascent + descent（与 `layout_text` 行盒一致）。
    pub fn line_box_height(&mut self, font_size: f32) -> f32 {
        let fs = crate::draw::resources::font::text_backend::bounded_font_size(font_size);
        self.font_service
            .horizontal_line_metrics(&self.font, fs)
            .map(|m| m.ascent + m.descent)
            .unwrap_or(fs * 1.2)
    }

    // ── 2D 文本绘制 ──

    /// 绘制文本（左对齐，顶部对齐）。
    pub fn draw_text(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        pos: Point,
        color: Color,
        font_size: f32,
    ) {
        if text.is_empty() {
            return;
        }
        let backend_opts = self.text_opts(
            font_size,
            self.max_text_width,
            0.0,
            false,
            HAlign::Left,
            VAlign::Top,
        );
        let layout = self
            .font_service
            .layout_text_shared(&self.font, text, &backend_opts);
        self.blit_glyph_layout(canvas, &layout, pos, color, font_size);
    }

    /// 基于基线绘制文本。
    pub fn draw_text_baseline(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        x: f32,
        baseline_y: f32,
        color: Color,
        font_size: f32,
    ) {
        if text.is_empty() {
            return;
        }
        let fs = crate::draw::resources::font::text_backend::bounded_font_size(font_size);
        let metrics = self.font_service.horizontal_line_metrics(&self.font, fs);
        let ascent = metrics.map(|m| m.ascent).unwrap_or(fs * 0.8);
        let top_y = baseline_y - ascent;
        self.draw_text(canvas, text, Point::new(x, top_y), color, fs);
    }

    /// 在矩形内居中绘制文本（过渡 helper：行盒几何居中，无光学系数）。
    ///
    /// 控件主路径应先由布局算出 `text_rect`，再 [`Self::draw_text`] 顶对齐；
    /// 勿在此叠加观感修正。
    pub fn text_center(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        rect: Rect,
        color: Color,
        font_size: f32,
    ) {
        if text.is_empty() {
            return;
        }
        let cnt = rect.x + rect.w * 0.5;
        let backend_opts = self.text_opts(
            font_size,
            self.max_text_width,
            0.0,
            false,
            HAlign::Left,
            VAlign::Top,
        );
        let layout = self
            .font_service
            .layout_text_shared(&self.font, text, &backend_opts);
        let x = cnt - layout.width * 0.5;
        let y = self.visual_center_y(rect, font_size);
        self.blit_glyph_layout(canvas, &layout, Point::new(x, y), color, font_size);
    }

    /// 左对齐、垂直居中（过渡 helper：行盒几何居中，无光学系数）。
    pub fn draw_text_in_frame(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        rect: Rect,
        color: Color,
        font_size: f32,
    ) {
        if text.is_empty() {
            return;
        }
        let backend_opts = self.text_opts(
            font_size,
            rect.w.max(1.0),
            0.0,
            false,
            HAlign::Left,
            VAlign::Top,
        );
        let layout = self
            .font_service
            .layout_text_shared(&self.font, text, &backend_opts);
        let x = rect.x;
        let y = self.visual_center_y(rect, font_size);
        self.blit_glyph_layout(canvas, &layout, Point::new(x, y), color, font_size);
    }

    /// 在矩形内绘制自动换行文本。
    pub fn draw_text_wrapped(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        rect: Rect,
        color: Color,
        font_size: f32,
    ) {
        if text.is_empty() {
            return;
        }
        let backend_opts = self.text_opts(
            font_size,
            rect.w.max(1.0),
            rect.h.max(0.0),
            true,
            HAlign::Left,
            VAlign::Top,
        );
        let layout = self
            .font_service
            .layout_text_shared(&self.font, text, &backend_opts);
        self.blit_glyph_layout(
            canvas,
            &layout,
            Point::new(rect.x, rect.y),
            color,
            font_size,
        );
    }

    /// 绘制文本选中背景 + 文本。
    #[allow(
        clippy::too_many_arguments,
        reason = "selection colors and bounds are part of the text rendering contract"
    )]
    pub fn draw_text_with_selection(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        pos: Point,
        color: Color,
        font_size: f32,
        selection: Option<(usize, usize)>,
        selection_bg: Color,
    ) {
        if let Some((s, e)) = selection {
            // 立即消费选区几何，避免稳态绘制创建范围和矩形临时向量。
            self.visit_selection_rects(text, font_size, pos, s, e, |rect| {
                canvas.fill_rect(rect, selection_bg, None);
            });
        }
        self.draw_text(canvas, text, pos, color, font_size);
    }

    // ── 3D 空间文本 ──

    /// 在 3D 空间中绘制文本。
    pub fn draw_text_spatial(
        &mut self,
        canvas: &mut dyn Canvas2D,
        spatial: &SpatialContext,
        text: &str,
        pos: Vec3,
        color: Color,
        font_size: PhysicalUnit,
    ) {
        if text.is_empty() {
            return;
        }
        let (sx, sy) = spatial.project(&pos);
        let fs = crate::draw::resources::font::text_backend::bounded_font_size(
            font_size.to_dip(spatial.dpi()),
        );
        self.draw_text(canvas, text, Point::new(sx, sy), color, fs);
    }

    /// 在 3D 空间中的矩形区域内居中绘制文本。
    pub fn text_center_spatial(
        &mut self,
        canvas: &mut dyn Canvas2D,
        spatial: &SpatialContext,
        text: &str,
        box_3d: AABB3D,
        color: Color,
        font_size: PhysicalUnit,
    ) {
        if text.is_empty() {
            return;
        }
        let fs = crate::draw::resources::font::text_backend::bounded_font_size(
            font_size.to_dip(spatial.dpi()),
        );
        let backend_opts = self.text_opts(
            fs,
            self.max_text_width,
            0.0,
            false,
            HAlign::Left,
            VAlign::Top,
        );
        let layout = self
            .font_service
            .layout_text_shared(&self.font, text, &backend_opts);
        let quad = spatial.project_aabb(&box_3d);
        let bounds = quad.bounds();
        let x = bounds.x + (bounds.w - layout.width) * 0.5;
        let y = bounds.y + (bounds.h - fs * 1.5) * 0.5;
        self.blit_glyph_layout(canvas, &layout, Point::new(x, y), color, fs);
    }

    // ── 文本选中 ──

    /// 获取选中文本的矩形区域列表。
    pub fn selection_rects(
        &mut self,
        _canvas: &mut dyn Canvas2D,
        text: &str,
        font_size: f32,
        pos: Point,
        start: usize,
        end: usize,
    ) -> Vec<Rect> {
        // 只为确实需要持有几何的兼容调用方收集结果。
        let mut rects = Vec::new();
        // 复用与直接绘制相同的流式几何实现。
        self.visit_selection_rects(text, font_size, pos, start, end, |rect| {
            // 固化当前矩形供调用方后续使用。
            rects.push(rect);
        });
        // 返回兼容接口要求的拥有型结果。
        rects
    }

    /// 流式访问选中文本的矩形区域，供绘制热路径立即消费。
    fn visit_selection_rects(
        &mut self,
        text: &str,
        font_size: f32,
        pos: Point,
        start: usize,
        end: usize,
        mut visit: impl FnMut(Rect),
    ) {
        if text.is_empty() || start >= end {
            return;
        }
        let backend_opts = self.text_opts(
            font_size,
            self.max_text_width,
            0.0,
            false,
            HAlign::Left,
            VAlign::Top,
        );
        let layout = self
            .font_service
            .layout_text_shared(&self.font, text, &backend_opts);
        if layout.glyphs.is_empty() {
            return;
        }
        // 流式扫描源文本，把一次性绘制范围扩展到合法字素簇边界。
        let (start, end) = crate::draw::resources::font::text_index::normalize_selection_in_text(
            // 借用布局对应的完整 UTF-8 源文本。
            text,
            // 显式标注选择起点使用字符下标。
            crate::draw::resources::font::text_index::CharIndex(start),
            // 显式标注选择终点使用字符下标。
            crate::draw::resources::font::text_index::CharIndex(end),
        );
        // 还原布局几何接口使用的数值起点。
        let start = start.0;
        // 还原布局几何接口使用的数值终点。
        let end = end.0;
        let fs = crate::draw::resources::font::text_backend::bounded_font_size(font_size);
        let visual_h = self
            .font_service
            .horizontal_line_metrics(&self.font, fs)
            .map(|m| m.ascent + m.descent)
            .unwrap_or(fs * 1.2);
        for line in &layout.lines {
            let gs = line.glyph_start;
            let gc = line.glyph_count;
            let ge = gs + gc;
            let glyphs = &layout.glyphs[gs..ge.min(layout.glyphs.len())];
            let y0 = pos.y + line.y;
            // 双向行的一个逻辑选择区可能形成多个不连续视觉片段。
            crate::draw::resources::font::text_backend::visit_glyph_selection_x_ranges(
                glyphs,
                start,
                end,
                |line_x0, line_x1| {
                    // 将行内片段左缘平移到绘制原点。
                    let x0 = pos.x + line_x0;
                    // 将行内片段右缘平移到绘制原点。
                    let x1 = pos.x + line_x1;
                    // 每个连续视觉片段独立形成选择矩形。
                    visit(Rect::new(x0, y0, (x1 - x0).max(0.0), visual_h));
                },
            );
        }
    }

    // ── 文本测量 ──

    /// 测量文本尺寸（不换行）。
    pub fn measure_text(&mut self, text: &str, font_size: f32) -> Size {
        let backend_opts = self.text_opts(
            font_size,
            self.max_text_width,
            0.0,
            false,
            HAlign::Left,
            VAlign::Top,
        );
        self.font_service
            .measure_text(&self.font, text, &backend_opts)
    }

    /// 测量文本尺寸（换行模式）。
    pub fn measure_text_wrapped(&mut self, text: &str, font_size: f32, max_width: f32) -> Size {
        let backend_opts =
            self.text_opts(font_size, max_width, 0.0, true, HAlign::Left, VAlign::Top);
        self.font_service
            .measure_text(&self.font, text, &backend_opts)
    }

    /// 文本命中测试。
    pub fn text_hit_test(&mut self, text: &str, font_size: f32, point: Point) -> Option<usize> {
        let backend_opts = self.text_opts(
            font_size,
            self.max_text_width,
            0.0,
            false,
            HAlign::Left,
            VAlign::Top,
        );
        self.font_service
            .hit_test_text(&self.font, text, &backend_opts, point)
    }

    /// 获取指定字符的光标 x 位置。
    pub fn text_cursor_x(&mut self, text: &str, font_size: f32, char_index: usize) -> f32 {
        let backend_opts = self.text_opts(
            font_size,
            self.max_text_width,
            0.0,
            false,
            HAlign::Left,
            VAlign::Top,
        );
        self.font_service
            .text_cursor_x(&self.font, text, &backend_opts, char_index)
    }

    // ── 辅助 ──

    /// 行盒（ascent+descent）在 `rect` 内几何居中时的 layout 原点 y（行顶）。
    ///
    /// 无光学下移。优先用布局把文字盒放好，再 `draw_text`；本函数仅过渡/简单控件。
    pub fn visual_center_y(&mut self, rect: Rect, font_size: f32) -> f32 {
        let fs = crate::draw::resources::font::text_backend::bounded_font_size(font_size);
        match self.font_service.horizontal_line_metrics(&self.font, fs) {
            Some(m) => rect.y + (rect.h - m.ascent - m.descent) * 0.5,
            None => rect.y + (rect.h - fs) * 0.5,
        }
    }

    /// 获取当前字体句柄。
    pub fn font(&self) -> &FontHandle {
        &self.font
    }
}

// ── TextRenderer trait 实现 ────────────────────────────────────────

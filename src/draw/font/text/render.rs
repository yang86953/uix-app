//! TextRenderService — 文本渲染服务。

use crate::draw::font::font_service::FontService;
use crate::draw::spatial::{PhysicalUnit, SpatialContext, Vec3, AABB3D};
use crate::draw::font::text_backend::TextLayoutOptions;
use crate::draw::traits::Canvas2D;
use crate::draw::{Color, FontHandle, HAlign, VAlign};
use crate::native::{Point, Rect, Size};

/// 文本渲染服务 — 字体管理、文本布局、glyph 光栅化。
pub struct TextRenderService<'a> {
    pub font: FontHandle,
    pub font_service: &'a FontService,
    pub max_text_width: f32,
}

impl<'a> TextRenderService<'a> {
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
        layout: &crate::draw::font::text_backend::TextLayout,
        pos: Point,
        color: Color,
        font_size: f32,
    ) {
        let fs = font_size.max(1.0);
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
            canvas.blit_glyph(gx, gy, &raster.coverage, raster.width, raster.height, color);
        }
    }

    /// 公开的 glyph 绘制入口（供 RenderContext 委托）。
    pub fn blit_to(
        &mut self,
        canvas: &mut dyn Canvas2D,
        layout: &crate::draw::font::text_backend::TextLayout,
        pos: Point,
        color: Color,
        font_size: f32,
    ) {
        self.blit_glyph_layout(canvas, layout, pos, color, font_size);
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
            .layout_text(&self.font, text, &backend_opts);
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
        let fs = font_size.max(1.0);
        let metrics = self.font_service.horizontal_line_metrics(&self.font, fs);
        let ascent = metrics.map(|m| m.ascent).unwrap_or(fs * 0.8);
        let top_y = baseline_y - ascent;
        self.draw_text(canvas, text, Point::new(x, top_y), color, fs);
    }

    /// 在矩形内居中绘制文本。
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
            .layout_text(&self.font, text, &backend_opts);
        let x = cnt - layout.width * 0.5;
        let y = self.visual_center_y(rect, font_size);
        self.blit_glyph_layout(canvas, &layout, Point::new(x, y), color, font_size);
    }

    /// 左对齐、垂直居中的文本绘制。
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
            .layout_text(&self.font, text, &backend_opts);
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
            .layout_text(&self.font, text, &backend_opts);
        self.blit_glyph_layout(
            canvas,
            &layout,
            Point::new(rect.x, rect.y),
            color,
            font_size,
        );
    }

    /// 绘制文本选中背景 + 文本。
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
            let rects = self.selection_rects(canvas, text, font_size, pos, s, e);
            for r in rects {
                canvas.fill_rect(r, selection_bg, None);
            }
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
        let fs = font_size.to_dip(spatial.dpi());
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
        let fs = font_size.to_dip(spatial.dpi());
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
            .layout_text(&self.font, text, &backend_opts);
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
        if text.is_empty() || start >= end {
            return Vec::new();
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
            .layout_text(&self.font, text, &backend_opts);
        if layout.glyphs.is_empty() {
            return Vec::new();
        }
        let end = end.min(layout.glyphs.len());
        let start = start.min(end);
        let visual_h = self
            .font_service
            .horizontal_line_metrics(&self.font, font_size)
            .map(|m| m.ascent + m.descent)
            .unwrap_or(font_size * 1.2);
        let mut rects = Vec::new();
        for line in &layout.lines {
            let gs = line.glyph_start;
            let gc = line.glyph_count;
            let ge = gs + gc;
            let sel_start = start.max(gs);
            let sel_end = end.min(ge);
            if sel_start >= sel_end {
                continue;
            }
            let glyphs = &layout.glyphs[sel_start..sel_end];
            let x0 = pos.x + glyphs[0].x;
            let last = glyphs[glyphs.len() - 1];
            let x1 = pos.x + last.x + last.width.max(0.0);
            let y0 = pos.y + line.y;
            rects.push(Rect::new(x0, y0, (x1 - x0).max(0.0), visual_h));
        }
        rects
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

    /// 计算文字视觉中心与 rect 中心对齐时的 y 位置。
    pub fn visual_center_y(&mut self, rect: Rect, font_size: f32) -> f32 {
        let fs = font_size.max(1.0);
        match self.font_service.horizontal_line_metrics(&self.font, fs) {
            Some(m) => rect.y + (rect.h - m.ascent - m.descent) * 0.5,
            None => rect.y + rect.h * 0.5,
        }
    }

    /// 获取当前字体句柄。
    pub fn font(&self) -> &FontHandle {
        &self.font
    }
}

// ── TextRenderer trait 实现 ────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::font::font_service::FontService;

    #[test]
    fn new_sets_font_and_service() {
        let fs = FontService::new();
        let fh = FontHandle::default();
        let trs = TextRenderService::new(fh, &fs, 500.0);
        assert_eq!(trs.font, fh);
        assert_eq!(trs.max_text_width, 500.0);
    }

    #[test]
    fn set_font_updates_handle() {
        let fs = FontService::new();
        let fh1 = FontHandle::new(1);
        let fh2 = FontHandle::new(2);
        let mut trs = TextRenderService::new(fh1, &fs, 500.0);
        assert_eq!(*trs.font(), fh1);
        trs.set_font(fh2);
        assert_eq!(*trs.font(), fh2);
    }

    #[test]
    fn set_max_text_width_updates() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        assert_eq!(trs.max_text_width, 500.0);
        trs.set_max_text_width(800.0);
        assert_eq!(trs.max_text_width, 800.0);
        trs.set_max_text_width(0.0);
        assert_eq!(trs.max_text_width, 0.0);
    }

    #[test]
    fn draw_text_empty_returns_early() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        trs.draw_text(&mut canvas, "", Point::new(0.0, 0.0), Color::black(), 14.0);
    }

    #[test]
    fn draw_text_baseline_empty_returns_early() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        trs.draw_text_baseline(&mut canvas, "", 0.0, 0.0, Color::black(), 14.0);
    }

    #[test]
    fn text_center_empty_returns_early() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        trs.text_center(
            &mut canvas,
            "",
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Color::black(),
            14.0,
        );
    }

    #[test]
    fn draw_text_in_frame_empty_returns_early() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        trs.draw_text_in_frame(
            &mut canvas,
            "",
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Color::black(),
            14.0,
        );
    }

    #[test]
    fn draw_text_wrapped_empty_returns_early() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        trs.draw_text_wrapped(
            &mut canvas,
            "",
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Color::black(),
            14.0,
        );
    }

    #[test]
    fn selection_rects_empty_text() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        let rects = trs.selection_rects(&mut canvas, "", 14.0, Point::new(0.0, 0.0), 0, 0);
        assert!(rects.is_empty());
    }

    #[test]
    fn selection_rects_invalid_range() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        // start > end → empty
        let rects = trs.selection_rects(&mut canvas, "hello", 14.0, Point::new(0.0, 0.0), 3, 1);
        assert!(rects.is_empty());
    }

    #[test]
    fn visual_center_y_default_font() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let center = trs.visual_center_y(Rect::new(0.0, 0.0, 100.0, 50.0), 14.0);
        // 只是验证不 panic 且有合理返回值
        assert!(center >= 0.0);
    }

    #[test]
    fn draw_text_with_selection_no_selection() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        // selection=None 时只绘制文本
        trs.draw_text_with_selection(
            &mut canvas,
            "",
            Point::new(0.0, 0.0),
            Color::black(),
            14.0,
            None,
            Color::blue(),
        );
    }

    #[test]
    fn font_service_ref_accessible() {
        let fs = FontService::new();
        let trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        // font_service 指针应指向传入的 fs
        assert_eq!(trs.font_service as *const _, &fs as *const _);
    }

    #[test]
    fn draw_text_spatial_empty_returns_early() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas_s = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        let spatial = crate::draw::spatial::SpatialContext::new(
            &mut canvas_s,
            96.0,
            1.0,
            crate::draw::spatial::Orientation::YDown,
            800,
            600,
        );
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        trs.draw_text_spatial(
            &mut canvas,
            &spatial,
            "",
            Vec3::zero(),
            Color::black(),
            PhysicalUnit::Px(14.0),
        );
    }

    #[test]
    fn text_center_spatial_empty_returns_early() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas_s = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        let spatial = crate::draw::spatial::SpatialContext::new(
            &mut canvas_s,
            96.0,
            1.0,
            crate::draw::spatial::Orientation::YDown,
            800,
            600,
        );
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        trs.text_center_spatial(
            &mut canvas,
            &spatial,
            "",
            AABB3D::new(Vec3::zero(), Vec3::new(100.0, 50.0, 0.0)),
            Color::black(),
            PhysicalUnit::Px(14.0),
        );
    }

    #[test]
    fn measure_text_returns_size() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let sz = trs.measure_text("hello", 14.0);
        // 默认字体（ab_glyph）能成功布局，返回实际宽高
        assert!(sz.w > 0.0);
        assert!(sz.h > 0.0);
    }

    #[test]
    fn measure_text_empty_returns_minimal_height() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let sz = trs.measure_text("", 14.0);
        // 空文本：宽度为 0，高度为行高
        assert_eq!(sz.w, 0.0);
        assert!(sz.h > 0.0);
    }

    #[test]
    fn measure_text_wrapped_returns_size() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let sz = trs.measure_text_wrapped("hello world", 14.0, 100.0);
        // 换行模式下返回实际布局尺寸
        assert!(sz.w > 0.0);
        assert!(sz.h > 0.0);
    }

    #[test]
    fn text_hit_test_works_with_default_font() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let result = trs.text_hit_test("hello", 14.0, Point::new(0.0, 0.0));
        // 默认字体可用，命中测试返回有效索引
        assert!(result.is_some());
    }

    #[test]
    fn text_cursor_x_returns_position() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let x = trs.text_cursor_x("hello", 14.0, 0);
        // 默认字体可用，返回有效 x 坐标
        assert!(x >= 0.0);
        // 索引 3 应在索引 0 之后
        let x3 = trs.text_cursor_x("hello", 14.0, 3);
        assert!(x3 >= x);
    }

    #[test]
    fn draw_text_with_selection_empty_renders_text() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        // 空 selection 应只绘制文本
        trs.draw_text_with_selection(
            &mut canvas,
            "text",
            Point::new(0.0, 0.0),
            Color::black(),
            14.0,
            None,
            Color::blue(),
        );
    }

    #[test]
    fn draw_text_with_selection_empty_range_skips_bg() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        // selection(0,0) 是空范围 → 跳过背景矩形
        trs.draw_text_with_selection(
            &mut canvas,
            "text",
            Point::new(0.0, 0.0),
            Color::black(),
            14.0,
            Some((0, 0)),
            Color::blue(),
        );
    }

    #[test]
    fn blit_to_with_empty_layout_no_panic() {
        use crate::draw::font::text_backend::TextLayout;
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        let layout = TextLayout {
            width: 0.0,
            height: 0.0,
            lines: vec![],
            glyphs: vec![],
        };
        trs.blit_to(
            &mut canvas,
            &layout,
            Point::new(0.0, 0.0),
            Color::black(),
            14.0,
        );
    }
}

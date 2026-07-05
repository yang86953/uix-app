//! UI 层 trait 桥接 — TextRenderer / DebugRenderer 实现。

use crate::draw::debug::DebugRenderService;
use crate::draw::spatial::{PhysicalUnit, SpatialContext, Vec3, AABB3D};
use crate::draw::font::text::TextRenderService;
use crate::draw::font::text_backend::TextLayout;
use crate::draw::traits::Canvas2D;
use crate::draw::{Color, FontHandle};
use crate::native::{Point, Rect, Size};

use crate::ui::traits::{DebugRenderer, TextRenderer};

impl DebugRenderer for DebugRenderService {
    fn set_debug_mode(&mut self, mode: bool) {
        DebugRenderService::set_debug_mode(self, mode);
    }
    fn debug_mode(&self) -> bool {
        self.debug_mode
    }
    fn draw_debug_border(
        &self,
        canvas: &mut dyn Canvas2D,
        rect: Rect,
        depth: usize,
        hovered: bool,
    ) {
        self.draw_debug_border(canvas, rect, depth, hovered);
    }
}

impl<'a> TextRenderer for TextRenderService<'a> {
    fn set_font(&mut self, font: FontHandle) {
        TextRenderService::set_font(self, font);
    }
    fn set_max_text_width(&mut self, width: f32) {
        TextRenderService::set_max_text_width(self, width);
    }
    fn font(&self) -> &FontHandle {
        TextRenderService::font(self)
    }
    fn font_service(&mut self) -> &crate::draw::font::font_service::FontService {
        self.font_service
    }

    fn draw_text(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        pos: Point,
        color: Color,
        font_size: f32,
    ) {
        TextRenderService::draw_text(self, canvas, text, pos, color, font_size);
    }
    fn draw_text_baseline(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        x: f32,
        baseline_y: f32,
        color: Color,
        font_size: f32,
    ) {
        TextRenderService::draw_text_baseline(self, canvas, text, x, baseline_y, color, font_size);
    }
    fn text_center(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        rect: Rect,
        color: Color,
        font_size: f32,
    ) {
        TextRenderService::text_center(self, canvas, text, rect, color, font_size);
    }
    fn draw_text_in_frame(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        rect: Rect,
        color: Color,
        font_size: f32,
    ) {
        TextRenderService::draw_text_in_frame(self, canvas, text, rect, color, font_size);
    }
    fn draw_text_wrapped(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        rect: Rect,
        color: Color,
        font_size: f32,
    ) {
        TextRenderService::draw_text_wrapped(self, canvas, text, rect, color, font_size);
    }
    fn draw_text_with_selection(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        pos: Point,
        color: Color,
        font_size: f32,
        selection: Option<(usize, usize)>,
        selection_bg: Color,
    ) {
        TextRenderService::draw_text_with_selection(
            self, canvas, text, pos, color, font_size, selection, selection_bg,
        );
    }
    fn selection_rects(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        font_size: f32,
        pos: Point,
        start: usize,
        end: usize,
    ) -> Vec<Rect> {
        TextRenderService::selection_rects(self, canvas, text, font_size, pos, start, end)
    }
    fn measure_text(&mut self, text: &str, font_size: f32) -> Size {
        TextRenderService::measure_text(self, text, font_size)
    }
    fn measure_text_wrapped(&mut self, text: &str, font_size: f32, max_width: f32) -> Size {
        TextRenderService::measure_text_wrapped(self, text, font_size, max_width)
    }
    fn text_hit_test(&mut self, text: &str, font_size: f32, point: Point) -> Option<usize> {
        TextRenderService::text_hit_test(self, text, font_size, point)
    }
    fn text_cursor_x(&mut self, text: &str, font_size: f32, char_index: usize) -> f32 {
        TextRenderService::text_cursor_x(self, text, font_size, char_index)
    }
    fn visual_center_y(&mut self, rect: Rect, font_size: f32) -> f32 {
        TextRenderService::visual_center_y(self, rect, font_size)
    }
    fn draw_text_spatial(
        &mut self,
        canvas: &mut dyn Canvas2D,
        spatial: &SpatialContext,
        text: &str,
        pos: Vec3,
        color: Color,
        font_size: PhysicalUnit,
    ) {
        TextRenderService::draw_text_spatial(self, canvas, spatial, text, pos, color, font_size);
    }
    fn text_center_spatial(
        &mut self,
        canvas: &mut dyn Canvas2D,
        spatial: &SpatialContext,
        text: &str,
        box_3d: AABB3D,
        color: Color,
        font_size: PhysicalUnit,
    ) {
        TextRenderService::text_center_spatial(self, canvas, spatial, text, box_3d, color, font_size);
    }
    fn blit_to(
        &mut self,
        canvas: &mut dyn Canvas2D,
        layout: &TextLayout,
        pos: Point,
        color: Color,
        font_size: f32,
    ) {
        TextRenderService::blit_to(self, canvas, layout, pos, color, font_size);
    }
}

//! # 渲染服务契约
//!
//! 文本绘制与调试渲染的 trait 接口。

use crate::core::{Point, Rect, Size};
use crate::draw::font::font_service::FontService;
use crate::draw::spatial::PhysicalUnit;
use crate::draw::traits::Canvas2D;
use crate::draw::Color;
use crate::draw::FontHandle;
/// 文本渲染服务接口 — 字体管理、文本布局、glyph 光栅化。
pub trait TextRenderer {
    /// 更新字体句柄。
    fn set_font(&mut self, font: FontHandle);
    /// 设置文本绘制最大宽度。
    fn set_max_text_width(&mut self, width: f32);
    /// 获取当前字体句柄。
    fn font(&self) -> &FontHandle;
    /// 获取字体服务引用。
    fn font_service(&mut self) -> &FontService;

    // ── 2D 文本绘制 ──
    fn draw_text(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        pos: Point,
        color: Color,
        font_size: f32,
    );
    fn draw_text_baseline(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        x: f32,
        baseline_y: f32,
        color: Color,
        font_size: f32,
    );
    fn text_center(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        rect: Rect,
        color: Color,
        font_size: f32,
    );
    fn draw_text_in_frame(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        rect: Rect,
        color: Color,
        font_size: f32,
    );
    fn draw_text_wrapped(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        rect: Rect,
        color: Color,
        font_size: f32,
    );

    // ── 文本选中 ──
    #[allow(
        clippy::too_many_arguments,
        reason = "selection colors and bounds are part of the rendering trait contract"
    )]
    fn draw_text_with_selection(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        pos: Point,
        color: Color,
        font_size: f32,
        selection: Option<(usize, usize)>,
        selection_bg: Color,
    );
    fn selection_rects(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        font_size: f32,
        pos: Point,
        start: usize,
        end: usize,
    ) -> Vec<Rect>;

    // ── 文本测量 ──
    fn measure_text(&mut self, text: &str, font_size: f32) -> Size;
    fn measure_text_wrapped(&mut self, text: &str, font_size: f32, max_width: f32) -> Size;
    fn text_hit_test(&mut self, text: &str, font_size: f32, point: Point) -> Option<usize>;
    fn text_cursor_x(&mut self, text: &str, font_size: f32, char_index: usize) -> f32;

    // ── 辅助 ──
    /// 行盒在 rect 内几何居中的 layout 原点 y（过渡；优先布局算盒再 draw_text）。
    fn visual_center_y(&mut self, rect: Rect, font_size: f32) -> f32;

    // ── 3D 空间文本 ──
    fn draw_text_spatial(
        &mut self,
        canvas: &mut dyn Canvas2D,
        spatial: &crate::draw::spatial::SpatialContext,
        text: &str,
        pos: crate::draw::spatial::Vec3,
        color: Color,
        font_size: PhysicalUnit,
    );
    fn text_center_spatial(
        &mut self,
        canvas: &mut dyn Canvas2D,
        spatial: &crate::draw::spatial::SpatialContext,
        text: &str,
        box_3d: crate::draw::spatial::AABB3D,
        color: Color,
        font_size: PhysicalUnit,
    );

    // ── 底层 glyph 绘制 ──
    fn blit_to(
        &mut self,
        canvas: &mut dyn Canvas2D,
        layout: &crate::draw::font::text_backend::TextLayout,
        pos: Point,
        color: Color,
        font_size: f32,
    );
}

/// 调试渲染服务接口。
pub trait DebugRenderer {
    fn set_debug_mode(&mut self, mode: bool);
    fn debug_mode(&self) -> bool;
    fn draw_debug_border(&self, canvas: &mut dyn Canvas2D, rect: Rect, depth: usize, hovered: bool);
}

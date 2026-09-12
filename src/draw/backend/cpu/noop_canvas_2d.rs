//! NoopCanvas2D — Canvas2D 的空实现。
//! 所有绘制操作都是空操作，仅用于绘图单元测试夹具。

use crate::core::{Rect, Size};
use crate::draw::Canvas2D;
use crate::draw::geometry::color::Color;
use crate::draw::geometry::path::{FillRule, Path};
use crate::draw::geometry::stroker::StrokeOptions;
use crate::draw::geometry::types::{BlendMode, GradientDirection, Radius, Transform};

/// Canvas2D 空实现。
pub(crate) struct NoopCanvas2D;

impl Canvas2D for NoopCanvas2D {
    fn current_transform(&self) -> Transform {
        Transform::identity()
    }
    fn set_transform(&mut self, _: Transform) {}
    fn fill_rect(&mut self, _: Rect, _: Color, _: Option<Radius>) {}
    fn fill_circle(&mut self, _: f32, _: f32, _: f32, _: Color) {}
    fn fill_ellipse(&mut self, _: Rect, _: Color) {}
    fn fill_sector(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: Color) {}
    fn fill_path(&mut self, _: &Path, _: Color, _: FillRule) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: Option<Radius>) {}
    fn stroke_circle(&mut self, _: f32, _: f32, _: f32, _: Color, _: f32) {}
    fn stroke_path(&mut self, _: &Path, _: Color, _: &StrokeOptions) {}
    fn draw_line(&mut self, _: f32, _: f32, _: f32, _: f32, _: Color, _: f32) {}
    fn fill_linear_gradient_stops(&mut self, _: Rect, _: crate::draw::LinearGradient, _: Option<Radius>) {}
    fn fill_linear_gradient(&mut self, _: Rect, _: Color, _: Color, _: GradientDirection) {}
    fn fill_radial_gradient(&mut self, _: f32, _: f32, _: f32, _: f32, _: Color, _: Color) {}
    fn draw_box_shadow(&mut self, _: Rect, _: f32, _: f32, _: f32, _: Color, _: Option<Radius>) {}
    fn draw_box_shadow_ambient(
        &mut self,
        _: Rect,
        _: f32,
        _: f32,
        _: f32,
        _: Color,
        _: Option<Radius>,
    ) {
    }
    fn blit_image(&mut self, _: &[u32], _: i32, _: Rect, _: Rect) {}
    fn blit_glyph(&mut self, _: i32, _: i32, _: &[u8], _: usize, _: usize, _: Color) {}
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn push_clip(&mut self, _: Rect) {}
    fn pop_clip(&mut self) {}
    fn set_opacity(&mut self, _: f32) {}
    fn opacity(&self) -> f32 {
        1.0
    }

    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn push_clip_path(&mut self, _: &Path) {}
    fn pixels(&self) -> &[u32] {
        &[]
    }
    fn surface_size(&self) -> Size {
        Size::new(0.0, 0.0)
    }
    fn current_clip(&self) -> Rect {
        Rect::new(0.0, 0.0, 1.0, 1.0)
    }
    // The test backend intentionally has no pixel target; making this explicit
    // prevents a production Canvas2D implementation from inheriting no-op
    // scroll-copy semantics by accident.
    fn scroll_region(&mut self, _: Rect, _: f32, _: f32) {}
        #[cfg(test)]
        noop_canvas2d_pixels_mut_impl!();
}

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../../tests-src/draw/backend/cpu/noop_canvas_2d_tests.rs"]
mod noop_canvas_2d_tests;

#[cfg(test)]
use crate::draw::painting::canvas::canvas2d_test_macros::noop_canvas2d_pixels_mut_impl;

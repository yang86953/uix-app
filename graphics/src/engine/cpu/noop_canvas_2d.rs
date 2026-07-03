//! NoopCanvas2D — Canvas2D 的空实现。
//! 所有绘制操作都是空操作，用于 NullEngine。

use crate::color::Color;
use crate::path::{FillRule, Path};
use crate::stroker::StrokeOptions;
use crate::traits::Canvas2D;
use crate::types::{BlendMode, GradientDirection, Radius};
use uix_platform::{Rect, Size};

/// Canvas2D 空实现。
pub struct NoopCanvas2D;

impl Canvas2D for NoopCanvas2D {
    fn fill_rect(&mut self, _: Rect, _: Color, _: Option<Radius>) {}
    fn fill_circle(&mut self, _: f32, _: f32, _: f32, _: Color) {}
    fn fill_ellipse(&mut self, _: Rect, _: Color) {}
    fn fill_sector(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: Color) {}
    fn fill_path(&mut self, _: &Path, _: Color, _: FillRule) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: Option<Radius>) {}
    fn stroke_circle(&mut self, _: f32, _: f32, _: f32, _: Color, _: f32) {}
    fn stroke_path(&mut self, _: &Path, _: Color, _: &StrokeOptions) {}
    fn draw_line(&mut self, _: f32, _: f32, _: f32, _: f32, _: Color, _: f32) {}
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
    fn pixels_mut(&mut self) -> &mut [u32] {
        &mut []
    }
    fn surface_size(&self) -> Size {
        Size::new(0.0, 0.0)
    }
    fn current_clip(&self) -> Rect {
        Rect::new(0.0, 0.0, 1.0, 1.0)
    }
}

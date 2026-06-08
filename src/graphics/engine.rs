use crate::diag::Error;
use crate::graphics::{Color, Point, Rect, Size};
use crate::graphics::types::*;

/// GraphicsEngine — abstract 2D rendering interface.
pub trait GraphicsEngine: 'static {
    // Lifetime
    fn initialize(&mut self, native_window: *mut std::ffi::c_void, width: i32, height: i32) -> Result<(), Error>;
    fn shutdown(&mut self);
    fn resize(&mut self, width: i32, height: i32);

    // Frame control
    fn begin_frame(&mut self, dirty: &DirtyRegion);
    fn end_frame(&mut self, dirty: &DirtyRegion);

    // Clip & state
    fn push_clip_rect(&mut self, rect: Rect);
    fn pop_clip_rect(&mut self);
    fn set_opacity(&mut self, opacity: f32);
    fn opacity(&self) -> f32;
    fn save(&mut self);
    fn restore(&mut self);
    fn set_transform(&mut self, t: Transform);
    fn reset_transform(&mut self);
    fn set_blend_mode(&mut self, mode: BlendMode);

    // Shapes
    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>);
    fn stroke_rect(&mut self, rect: Rect, color: Color, line_width: f32, radius: Option<Radius>);
    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color);
    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, line_width: f32);
    fn fill_ellipse(&mut self, rect: Rect, color: Color);
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32);

    // Gradients
    fn fill_linear_gradient(&mut self, rect: Rect, color_a: Color, color_b: Color, dir: GradientDirection);
    fn fill_radial_gradient(&mut self, cx: f32, cy: f32, inner_r: f32, outer_r: f32, inner_color: Color, outer_color: Color);

    // Text
    fn load_font(&mut self, path: &str, size: f32) -> Result<&mut FontHandle, Error>;
    fn unload_font(&mut self, font: &FontHandle);
    fn measure_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> Size;
    fn draw_text(&mut self, font: &FontHandle, text: &str, pos: Point, color: Color, opts: &TextLayoutOptions);

    // Images
    fn load_image(&mut self, path: &str) -> Result<&mut ImageHandle, Error>;
    fn unload_image(&mut self, image: &ImageHandle);
    fn image_size(&self, image: &ImageHandle) -> Size;
    fn draw_image(&mut self, image: &ImageHandle, src: Rect, dst: Rect);

    // Offscreen
    fn create_offscreen(&mut self, w: i32, h: i32) -> Result<&mut ImageHandle, Error>;
    fn destroy_offscreen(&mut self, offscreen: &ImageHandle);
    fn begin_offscreen(&mut self, offscreen: &ImageHandle);
    fn end_offscreen(&mut self);

    // Query
    fn pixels(&self) -> &[u32];
    fn width(&self) -> i32;
    fn height(&self) -> i32;
    // Supersample control for software renderer (0 = adaptive/default).
    fn set_supersample_level(&mut self, _level: u8) {}
    fn supersample_level(&self) -> u8 { 0 }
}

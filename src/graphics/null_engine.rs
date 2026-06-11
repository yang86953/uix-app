//! NullEngine — GraphicsEngine stub implementation for testing/demo.
//! All rendering operations are no-ops; font/image loading returns NotImplemented.

use crate::diag::{info_fn, Errc, Error};
use crate::graphics::{BlendMode, Color, DirtyRegion, FontHandle, GradientDirection, GraphicsEngine, ImageHandle, Radius, TextLayoutOptions, Transform};
use crate::base::{Point, Rect, Size};

/// A GraphicsEngine that does nothing — useful as a compile-time stub
/// or for testing code that depends on `dyn GraphicsEngine`.
pub struct NullEngine {
    width: i32,
    height: i32,
    opacity: f32,
    #[allow(dead_code)]
    transform: Transform,
    blend_mode: BlendMode,
}

impl NullEngine {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            opacity: 1.0,
            transform: Transform::identity(),
            blend_mode: BlendMode::Alpha,
        }
    }
}

impl Default for NullEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphicsEngine for NullEngine {
    fn initialize(&mut self, w: i32, h: i32) -> Result<(), Error> {
        self.width = w;
        self.height = h;
        info_fn(format!("NullEngine::initialize({}x{})", w, h));
        Ok(())
    }

    fn shutdown(&mut self) {
        info_fn("NullEngine::shutdown".to_string());
    }

    fn resize(&mut self, w: i32, h: i32) {
        self.width = w;
        self.height = h;
        info_fn(format!("NullEngine::resize({}x{})", w, h));
    }

    fn begin_frame(&mut self, _d: &DirtyRegion) {}
    fn end_frame(&mut self, _d: &DirtyRegion) {}
    fn scroll_region(&mut self, _viewport: Rect, _dy: f32) {}

    fn push_clip_rect(&mut self, _r: Rect) {}
    fn pop_clip_rect(&mut self) {}

    fn set_opacity(&mut self, o: f32) {
        self.opacity = o;
    }
    fn opacity(&self) -> f32 {
        self.opacity
    }

    fn save(&mut self) {}
    fn restore(&mut self) {}

    fn set_transform(&mut self, _t: Transform) {}
    fn reset_transform(&mut self) {}

    fn set_blend_mode(&mut self, m: BlendMode) {
        self.blend_mode = m;
    }

    fn fill_rect(&mut self, r: Rect, _c: Color, _rad: Option<Radius>) {
        info_fn(format!("NullEngine::fill_rect({:?})", r));
    }

    fn stroke_rect(&mut self, r: Rect, _c: Color, _lw: f32, _rad: Option<Radius>) {
        info_fn(format!("NullEngine::stroke_rect({:?})", r));
    }

    fn fill_circle(&mut self, _cx: f32, _cy: f32, _r: f32, _c: Color) {}
    fn fill_circle_radial(&mut self, _cx: f32, _cy: f32, _r: f32, _c: Color) {}
    fn fill_sector(&mut self, _cx: f32, _cy: f32, _r: f32, _sa: f32, _ea: f32, _c: Color) {}
    fn stroke_circle(&mut self, _cx: f32, _cy: f32, _r: f32, _c: Color, _lw: f32) {}
    fn fill_ellipse(&mut self, _r: Rect, _c: Color) {}
    fn draw_box_shadow(
        &mut self,
        _r: Rect,
        _blur: f32,
        _ox: f32,
        _oy: f32,
        _c: Color,
        _cr: Option<Radius>,
    ) {
    }

    fn draw_line(&mut self, _x1: f32, _y1: f32, _x2: f32, _y2: f32, _c: Color, _w: f32) {}

    fn fill_linear_gradient(&mut self, _r: Rect, _ca: Color, _cb: Color, _d: GradientDirection) {}
    fn fill_radial_gradient(
        &mut self,
        _cx: f32,
        _cy: f32,
        _ir: f32,
        _or: f32,
        _ic: Color,
        _oc: Color,
    ) {
    }

    fn load_font(&mut self, _data: &[u8], _s: f32) -> Result<&mut FontHandle, Error> {
        Err(Error::new(Errc::NotImplemented, "NullEngine"))
    }
    fn unload_font(&mut self, _f: &FontHandle) {}

    fn measure_text(&self, _f: &FontHandle, _t: &str, _o: &TextLayoutOptions) -> Size {
        Size::new(0.0, 0.0)
    }

    fn draw_text(
        &mut self,
        _f: &FontHandle,
        _t: &str,
        _p: Point,
        _c: Color,
        _o: &TextLayoutOptions,
    ) {
    }

    fn load_image(&mut self, _data: &[u8]) -> Result<&mut ImageHandle, Error> {
        Err(Error::new(Errc::NotImplemented, "NullEngine"))
    }
    fn unload_image(&mut self, _i: &ImageHandle) {}

    fn image_size(&self, _i: &ImageHandle) -> Size {
        Size::new(0.0, 0.0)
    }

    fn draw_image(&mut self, _i: &ImageHandle, _s: Rect, _d: Rect) {}

    fn create_offscreen(&mut self, _w: i32, _h: i32) -> Result<&mut ImageHandle, Error> {
        Err(Error::new(Errc::NotImplemented, "NullEngine"))
    }
    fn destroy_offscreen(&mut self, _o: &ImageHandle) {}
    fn begin_offscreen(&mut self, _o: &ImageHandle) {}
    fn end_offscreen(&mut self) {}

    fn pixels(&self) -> &[u32] {
        &[]
    }
    fn width(&self) -> i32 {
        self.width
    }
    fn height(&self) -> i32 {
        self.height
    }
    fn set_supersample_level(&mut self, _level: u8) {}
    fn supersample_level(&self) -> u8 {
        0
    }
}

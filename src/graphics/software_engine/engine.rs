use super::asset_store::AssetStore;
use super::core::RenderTarget;
use super::FontData;
use crate::diag::{Errc, Error};
use crate::graphics::{
    BlendMode, Color, DirtyRegion, FontHandle, GraphicsEngine, ImageHandle, Point, Radius, Rect,
    Size, TextLayoutOptions,
};
use crate::graphics::{GradientDirection, Transform};

// ════════════════════════════════════════════════════════════════════════════
// FontData 辅助方法
// ════════════════════════════════════════════════════════════════════════════

impl FontData {
    pub(crate) fn char_advance(&self, ch: char) -> f32 {
        self.font.metrics(ch, self.size).advance_width
    }
}

// ════════════════════════════════════════════════════════════════════════════
// SoftwareEngine — 组合 AssetStore + RenderTarget
// ════════════════════════════════════════════════════════════════════════════

enum ActiveTarget {
    Main,
    Offscreen(usize),
}

pub struct SoftwareEngine {
    pub(crate) rt: RenderTarget,
    pub(crate) assets: AssetStore,
    active_target: ActiveTarget,
    main_width: i32,
    main_height: i32,
    saved_pixels: Vec<u32>,
    pre_frame_clip: Rect,
}

impl Default for SoftwareEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SoftwareEngine {
    pub fn new() -> Self {
        Self {
            rt: RenderTarget::new(),
            assets: AssetStore::new(),
            active_target: ActiveTarget::Main,
            main_width: 0,
            main_height: 0,
            saved_pixels: Vec::new(),
            pre_frame_clip: Rect::new(0.0, 0.0, f32::MAX, f32::MAX),
        }
    }

    pub fn pixel_buffer(&self) -> &[u32] {
        self.rt.pixel_buffer()
    }
    pub fn pixel_buffer_mut(&mut self) -> &mut [u32] {
        self.rt.pixel_buffer_mut()
    }
    pub fn pixel_bytes(&self) -> &[u8] {
        self.rt.pixel_bytes()
    }

    /// Set supersample level for software render target (0 = adaptive default).
    pub fn set_supersample_level(&mut self, level: u8) {
        self.rt.set_supersample_level(level);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// GraphicsEngine trait 实现
// ════════════════════════════════════════════════════════════════════════════

impl GraphicsEngine for SoftwareEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.main_width = width;
        self.main_height = height;
        self.rt.initialize(width, height);
        self.active_target = ActiveTarget::Main;
        Ok(())
    }

    fn shutdown(&mut self) {
        self.rt = RenderTarget::new();
        self.assets.shutdown();
        self.main_width = 0;
        self.main_height = 0;
        self.active_target = ActiveTarget::Main;
        self.saved_pixels.clear();
    }

    fn resize(&mut self, width: i32, height: i32) {
        let _ = self.initialize(width, height);
    }

    // ── 帧控制 ──

    fn begin_frame(&mut self, dirty: &DirtyRegion) {
        self.pre_frame_clip = self.rt.clip_rect();
        self.rt.clip_stack_mut().clear();
        let tw = self.rt.width();
        let th = self.rt.height();

        if dirty.full_frame {
            self.rt.clear_all();
            *self.rt.clip_rect_mut() = Rect::new(0.0, 0.0, tw as f32, th as f32);
        } else if dirty.clear_required {
            let r = dirty.rect;
            let x = r.x.max(0.0) as i32;
            let y = r.y.max(0.0) as i32;
            let w = (r.w as i32).min(tw - x);
            let h = (r.h as i32).min(th - y);
            if w > 0 && h > 0 {
                self.rt.clear_region(x, y, w, h);
            }
            *self.rt.clip_rect_mut() = Rect::new(
                r.x.max(0.0),
                r.y.max(0.0),
                (r.w).min(tw as f32 - r.x.max(0.0)),
                (r.h).min(th as f32 - r.y.max(0.0)),
            );
        } else {
            *self.rt.clip_rect_mut() = Rect::new(0.0, 0.0, tw as f32, th as f32);
        }
    }

    fn end_frame(&mut self, _dirty: &DirtyRegion) {
        *self.rt.clip_rect_mut() = self.pre_frame_clip;
        self.rt.clip_stack_mut().clear();
    }

    fn push_clip_rect(&mut self, rect: Rect) {
        self.rt.push_clip_rect(rect);
    }
    fn pop_clip_rect(&mut self) {
        self.rt.pop_clip_rect();
    }
    fn set_opacity(&mut self, opacity: f32) {
        self.rt.set_opacity(opacity);
    }
    fn opacity(&self) -> f32 {
        self.rt.opacity_val()
    }
    fn save(&mut self) {
        self.rt.save();
    }
    fn restore(&mut self) {
        self.rt.restore();
    }
    fn set_transform(&mut self, t: Transform) {
        self.rt.set_transform(t);
    }
    fn reset_transform(&mut self) {
        self.rt.reset_transform();
    }
    fn set_blend_mode(&mut self, mode: BlendMode) {
        self.rt.set_blend_mode(mode);
    }

    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        self.rt.fill_rect(rect, color, radius);
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, line_width: f32, radius: Option<Radius>) {
        self.rt.stroke_rect(rect, color, line_width, radius);
    }

    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        self.rt.fill_circle(cx, cy, r, color);
    }

    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, line_width: f32) {
        self.rt.stroke_circle(cx, cy, r, color, line_width);
    }

    fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        self.rt.fill_ellipse(rect, color);
    }

    fn draw_box_shadow(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    ) {
        self.rt
            .draw_box_shadow(rect, blur_radius, offset_x, offset_y, color, corner_radius);
    }

    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
        self.rt.draw_line(x1, y1, x2, y2, color, width);
    }

    fn fill_linear_gradient(
        &mut self,
        rect: Rect,
        color_a: Color,
        color_b: Color,
        dir: GradientDirection,
    ) {
        self.rt.fill_linear_gradient(rect, color_a, color_b, dir);
    }

    fn fill_radial_gradient(
        &mut self,
        cx: f32,
        cy: f32,
        inner_r: f32,
        outer_r: f32,
        inner_color: Color,
        outer_color: Color,
    ) {
        self.rt
            .fill_radial_gradient(cx, cy, inner_r, outer_r, inner_color, outer_color);
    }

    fn load_font(&mut self, data: &[u8], size: f32) -> Result<&mut FontHandle, Error> {
        self.assets.load_font(data.to_vec(), size)
    }

    fn unload_font(&mut self, _font: &FontHandle) {}

    fn measure_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> Size {
        if let Some(data) = self.assets.find_font(font) {
            RenderTarget::measure_ttf(data, text, opts)
        } else {
            self.assets.bitmap_font().measure(text, opts)
        }
    }

    fn draw_text(
        &mut self,
        font: &FontHandle,
        text: &str,
        pos: Point,
        color: Color,
        opts: &TextLayoutOptions,
    ) {
        if let Some(data) = self.assets.find_font(font) {
            self.rt.draw_ttf(data, text, pos, color, opts);
        } else {
            self.rt.draw_bitmap_text(text, pos, color);
        }
    }

    fn load_image(&mut self, data: &[u8]) -> Result<&mut ImageHandle, Error> {
        let img = image::load_from_memory(data)
            .map_err(|e| Error::new(Errc::FormatError, format!("cannot decode image: {}", e)))?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let pixels: Vec<u32> = rgba
            .chunks_exact(4)
            .map(|p| {
                let a = p[3] as u32;
                let r = (p[0] as u32 * a / 255).min(255);
                let g = (p[1] as u32 * a / 255).min(255);
                let b = (p[2] as u32 * a / 255).min(255);
                (a << 24) | (b << 16) | (g << 8) | r
            })
            .collect();
        Ok(self.assets.load_image(pixels, w as i32, h as i32))
    }

    fn unload_image(&mut self, _image: &ImageHandle) {}

    fn image_size(&self, image: &ImageHandle) -> Size {
        if let Some((w, h)) = self.assets.find_any_size(image) {
            Size::new(w as f32, h as f32)
        } else {
            Size::new(0.0, 0.0)
        }
    }

    fn draw_image(&mut self, image: &ImageHandle, src: Rect, dst: Rect) {
        if let Some((pixels, w, h)) = self.assets.find_image(image) {
            let src_clone = src;
            let dst_clone = dst;
            let op = self.rt.opacity_val();
            self.rt.blit_image(pixels, w, h, &src_clone, &dst_clone, op);
            return;
        }
        if let Some((pixels, w, h)) = self.assets.find_offscreen(image) {
            let src_clone = src;
            let dst_clone = dst;
            let op = self.rt.opacity_val();
            self.rt.blit_image(pixels, w, h, &src_clone, &dst_clone, op);
        }
    }

    fn create_offscreen(&mut self, w: i32, h: i32) -> Result<&mut ImageHandle, Error> {
        self.assets.create_offscreen(w, h)
    }

    fn destroy_offscreen(&mut self, _offscreen: &ImageHandle) {}

    fn begin_offscreen(&mut self, offscreen: &ImageHandle) {
        if let Some(idx) = self.assets.offscreen_slot_index(offscreen) {
            self.saved_pixels = self.rt.take_pixels();
            let (pixels, w, h) = self.assets.take_offscreen_pixels(idx);
            self.rt.set_pixels(pixels, w, h);
            self.active_target = ActiveTarget::Offscreen(idx);
        }
    }

    fn end_offscreen(&mut self) {
        if let ActiveTarget::Offscreen(idx) = self.active_target {
            let pixels = self.rt.take_pixels();
            self.assets.return_offscreen_pixels(idx, pixels);
            self.rt.set_pixels(
                std::mem::take(&mut self.saved_pixels),
                self.main_width,
                self.main_height,
            );
            self.active_target = ActiveTarget::Main;
        }
    }

    fn pixels(&self) -> &[u32] {
        self.rt.pixels()
    }
    fn width(&self) -> i32 {
        self.main_width
    }
    fn height(&self) -> i32 {
        self.main_height
    }
    fn set_supersample_level(&mut self, level: u8) {
        self.rt.set_supersample_level(level);
    }
    fn supersample_level(&self) -> u8 {
        self.rt.supersample_level()
    }
}

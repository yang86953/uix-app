//! CPU 渲染后端 — PixelSurface + CpuCanvas2D。

use uix_platform::{Error, Point, Rect};

use crate::backend::traits::{
    BackendCapabilities, BackendKind, DamageRegion, DrawSurface, RenderBackend,
};
use crate::color::Color;
use crate::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::engine::cpu::pixel_surface::PixelSurface;
use crate::rasterizer::image::blit_image;
use crate::traits::Canvas2D;
use crate::types::ImageHandle;

/// CPU 主缓冲 DrawSurface 适配器。
pub struct CpuDrawSurface {
    canvas: CpuCanvas2D,
}

impl CpuDrawSurface {
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            canvas: CpuCanvas2D::new(PixelSurface::new(width, height)),
        }
    }

    pub fn canvas_mut(&mut self) -> &mut CpuCanvas2D {
        &mut self.canvas
    }

    pub fn surface(&self) -> &PixelSurface {
        self.canvas.surface()
    }

    pub fn set_clear_color(&mut self, color: Color) {
        self.canvas.surface_mut().set_clear_color(color);
    }
}

impl DrawSurface for CpuDrawSurface {
    fn size(&self) -> uix_platform::Size {
        self.canvas.surface_size()
    }

    fn push_clip(&mut self, rect: Rect) {
        self.canvas.push_clip(rect);
    }

    fn pop_clip(&mut self) {
        self.canvas.pop_clip();
    }

    fn clear_all(&mut self) {
        self.canvas.surface_mut().clear_all();
    }

    fn clear_rect_raw(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.canvas.surface_mut().clear_rect_raw(x, y, w, h);
    }

    fn copy_region(&mut self, src: Rect, dst: Point) {
        use crate::traits::RenderingBackend;
        let canvas = &mut self.canvas;
        RenderingBackend::copy_region(
            canvas.surface_mut(),
            src,
            dst.x as i32,
            dst.y as i32,
        );
    }

    fn canvas(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas
    }
}

/// CPU 软件渲染后端。
pub struct CpuBackend {
    width: i32,
    height: i32,
    clear_color: Color,
    main: CpuDrawSurface,
    offscreens: Vec<Option<CpuCanvas2D>>,
    next_offscreen_id: u32,
}

impl CpuBackend {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            clear_color: Color::from_rgba(0, 0, 0, 0),
            main: CpuDrawSurface::new(1, 1),
            offscreens: Vec::new(),
            next_offscreen_id: 0,
        }
    }

    pub fn set_clear_color(&mut self, color: Color) {
        self.clear_color = color;
        self.main.set_clear_color(color);
    }

    pub fn clear_color(&self) -> Color {
        self.clear_color
    }

    fn blit_offscreen_impl(&mut self, handle: &ImageHandle, src_rect: Rect, dst_rect: Rect) {
        let idx = handle.0 as usize;
        if idx >= self.offscreens.len() {
            return;
        }
        if let Some(ref offscreen_canvas) = self.offscreens[idx] {
            let surf = offscreen_canvas.surface();
            let src_pixels = surf.pixels();
            let src_w = surf.width();
            let main = self.main.canvas_mut();
            let size = main.width();
            let h = main.height();
            let clip = main.current_clip();
            let opacity = main.opacity();
            blit_image(
                main.pixels_mut(),
                size,
                h,
                clip,
                opacity,
                src_pixels,
                src_w,
                src_rect,
                dst_rect,
            );
        }
    }
}

impl Default for CpuBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderBackend for CpuBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Cpu
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::cpu()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.width = width;
        self.height = height;
        self.main = CpuDrawSurface::new(width, height);
        Ok(())
    }

    fn shutdown(&mut self) {
        self.width = 0;
        self.height = 0;
        self.main = CpuDrawSurface::new(1, 1);
        self.offscreens.clear();
    }

    fn surface(&mut self) -> &mut dyn DrawSurface {
        &mut self.main
    }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let id = self.next_offscreen_id;
        self.next_offscreen_id += 1;
        let idx = id as usize;
        while self.offscreens.len() <= idx {
            self.offscreens.push(None);
        }
        let canvas = CpuCanvas2D::new(PixelSurface::new(width, height));
        self.offscreens[idx] = Some(canvas);
        Some(ImageHandle(id))
    }

    fn destroy_offscreen(&mut self, handle: ImageHandle) {
        let idx = handle.0 as usize;
        if idx < self.offscreens.len() {
            self.offscreens[idx] = None;
        }
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        let idx = handle.0 as usize;
        if idx < self.offscreens.len() {
            if let Some(ref mut canvas) = self.offscreens[idx] {
                return Some(canvas as &mut dyn Canvas2D);
            }
        }
        None
    }

    fn present(&mut self, _damage: &DamageRegion) -> Result<(), Error> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl CpuBackend {
    pub fn blit_offscreen(&mut self, handle: &ImageHandle, dst_rect: Rect) {
        let idx = handle.0 as usize;
        if idx >= self.offscreens.len() {
            return;
        }
        if let Some(ref offscreen_canvas) = self.offscreens[idx] {
            let surf = offscreen_canvas.surface();
            let src_rect = Rect::new(0.0, 0.0, surf.width() as f32, surf.height() as f32);
            self.blit_offscreen_impl(handle, src_rect, dst_rect);
        }
    }

    pub fn blit_offscreen_src(&mut self, handle: &ImageHandle, src_rect: Rect, dst_rect: Rect) {
        self.blit_offscreen_impl(handle, src_rect, dst_rect);
    }

    pub fn memory_usage(&self) -> usize {
        let surf = self.main.surface();
        let main_bytes = (surf.width() * surf.height() * 4) as usize;
        let offscreen_bytes: usize = self
            .offscreens
            .iter()
            .filter_map(|o| {
                o.as_ref().map(|c| {
                    let s = c.surface();
                    (s.width() * s.height() * 4) as usize
                })
            })
            .sum();
        main_bytes + offscreen_bytes
    }
}

//! CPU 渲染后端 — PixelSurface + CpuCanvas2D。

use crate::core::{Error, Point, Rect};

use crate::core::DamageRegion;
use crate::draw::backend::traits::{BackendCapabilities, BackendKind, DrawSurface, RenderBackend};
use crate::draw::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::primitives::color::Color;
use crate::draw::primitives::types::ImageHandle;
use crate::draw::rasterizer::image::blit_image;
use crate::draw::traits::Canvas2D;

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
    fn size(&self) -> crate::core::Size {
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
        use crate::draw::traits::RenderingBackend;
        let canvas = &mut self.canvas;
        RenderingBackend::copy_region(canvas.surface_mut(), src, dst.x as i32, dst.y as i32);
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
    /// Freed handle ids available for reuse (#105 — avoid unbounded Vec growth).
    free_offscreen_ids: Vec<u32>,
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
            free_offscreen_ids: Vec::new(),
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

    pub fn pixels(&self) -> &[u32] {
        self.main.surface().pixels()
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
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
        self.free_offscreen_ids.clear();
        self.next_offscreen_id = 0;
    }

    fn surface(&mut self) -> &mut dyn DrawSurface {
        &mut self.main
    }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let id = if let Some(id) = self.free_offscreen_ids.pop() {
            id
        } else {
            let id = self.next_offscreen_id;
            self.next_offscreen_id = self.next_offscreen_id.saturating_add(1);
            id
        };
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
        if idx < self.offscreens.len() && self.offscreens[idx].take().is_some() {
            self.free_offscreen_ids.push(handle.0);
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

    /// 将离屏缓冲内容 blit 到任意 Canvas2D（支持嵌套 Picture 合成）。
    pub fn blit_offscreen_to_canvas(
        &self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
        canvas: &mut dyn crate::draw::traits::Canvas2D,
    ) {
        let idx = handle.0 as usize;
        if idx >= self.offscreens.len() {
            return;
        }
        if let Some(ref offscreen_canvas) = self.offscreens[idx] {
            let surf = offscreen_canvas.surface();
            canvas.blit_image(surf.pixels(), surf.width(), src_rect, dst_rect);
        }
    }

    /// 复制离屏像素（避免与 offscreen_canvas 可变借用冲突）。
    pub fn copy_offscreen_pixels(&self, handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        let idx = handle.0 as usize;
        let canvas = self.offscreens.get(idx)?.as_ref()?;
        let surf = canvas.surface();
        Some((surf.pixels().to_vec(), surf.width()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::backend::traits::RenderBackend;

    #[test]
    fn create_offscreen_reuses_destroyed_ids() {
        let mut backend = CpuBackend::new();
        backend.resize(64, 64).expect("resize");
        let a = backend.create_offscreen(16, 16).expect("a");
        let b = backend.create_offscreen(16, 16).expect("b");
        assert_ne!(a.0, b.0);
        backend.destroy_offscreen(a);
        let c = backend.create_offscreen(8, 8).expect("c");
        assert_eq!(c.0, a.0, "destroyed id must be reused");
        assert_eq!(backend.offscreens.len(), 2, "slot vec must not grow on reuse");
        backend.destroy_offscreen(b);
        backend.destroy_offscreen(c);
    }
}

//! CPU 渲染后端 — PixelSurface + CpuCanvas2D。

use crate::core::{Error, Point, Rect};

use crate::core::DamageRegion;
use crate::draw::backend::offscreen_pool::CpuOffscreenPool;
use crate::draw::backend::traits::{BackendCapabilities, BackendKind, DrawSurface, RenderBackend};
use crate::draw::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::pipeline::{EncodedFrameExecution, EncodedPictureExecution, FrameEncoder};
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

    fn take_deferred_error(&mut self) -> Option<Error> {
        self.canvas.take_deferred_error()
    }
}

/// CPU 软件渲染后端。
pub struct CpuBackend {
    width: i32,
    height: i32,
    clear_color: Color,
    main: CpuDrawSurface,
    offscreens: CpuOffscreenPool,
    active_offscreen: Option<u32>,
}

impl CpuBackend {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            clear_color: Color::from_rgba(0, 0, 0, 0),
            main: CpuDrawSurface::new(1, 1),
            offscreens: CpuOffscreenPool::new(),
            active_offscreen: None,
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
        let Some(offscreen_canvas) = self.offscreens.get(handle) else {
            return;
        };
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
        self.active_offscreen = None;
        self.offscreens.clear();
    }

    fn surface(&mut self) -> &mut dyn DrawSurface {
        &mut self.main
    }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        self.offscreens.create(width, height)
    }

    fn destroy_offscreen(&mut self, handle: ImageHandle) {
        if self.active_offscreen == Some(handle.0) {
            self.active_offscreen = None;
        }
        self.offscreens.destroy(handle);
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.offscreens.canvas_mut(handle)
    }

    fn copy_offscreen_pixels(&self, handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        self.offscreens.copy_pixels(handle)
    }

    fn try_execute_encoded_picture(
        &mut self,
        handle: &ImageHandle,
        encoder: &FrameEncoder,
    ) -> Result<EncodedPictureExecution, Error> {
        let target = self.offscreens.get_mut(handle).ok_or_else(|| {
            Error::new(
                crate::core::Errc::InvalidState,
                "Picture offscreen target disappeared before FrameEncoder execution",
            )
        })?;
        let surface = target.surface_mut();
        if surface.width() != encoder.width() || surface.height() != encoder.height() {
            return Err(Error::new(
                crate::core::Errc::InvalidState,
                format!(
                    "FrameEncoder {}x{} does not match Picture target {}x{}",
                    encoder.width(),
                    encoder.height(),
                    surface.width(),
                    surface.height()
                ),
            ));
        }
        encoder.execute_into_pixels(surface.pixels_mut());
        Ok(EncodedPictureExecution::Executed)
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        if (self.width, self.height) != (encoder.width(), encoder.height()) {
            return Err(Error::new(
                crate::core::Errc::InvalidState,
                format!(
                    "FrameEncoder {}x{} does not match main CPU target {}x{}",
                    encoder.width(),
                    encoder.height(),
                    self.width,
                    self.height
                ),
            ));
        }
        encoder.execute_into_pixels(self.main.canvas_mut().pixels_mut());
        Ok(EncodedFrameExecution::Executed)
    }

    fn begin_offscreen_paint(&mut self, handle: &ImageHandle) -> bool {
        self.try_begin_offscreen_paint(handle).is_ok()
    }

    fn try_begin_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        let Some(canvas) = self.offscreens.get_mut(handle) else {
            return Err(Error::new(
                crate::core::Errc::InvalidState,
                "Picture offscreen target does not exist",
            ));
        };
        // Picture rasterization is replace semantics even for transparent
        // pixels. Filling with transparent through Canvas2D would be an
        // alpha-over no-op and could retain stale cached content.
        canvas.surface_mut().clear_all();
        self.active_offscreen = Some(handle.0);
        Ok(())
    }

    fn flush_offscreen_paint(&mut self, _handle: &ImageHandle) {}

    fn try_flush_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        let canvas = self.offscreens.get_mut(handle).ok_or_else(|| {
            Error::new(
                crate::core::Errc::InvalidState,
                "Picture offscreen target disappeared before flush",
            )
        })?;
        if let Some(error) = canvas.take_deferred_error() {
            return Err(error);
        }
        Ok(())
    }

    fn end_offscreen_paint(&mut self) {
        self.active_offscreen = None;
    }

    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        self.active_offscreen = None;
        Ok(())
    }

    fn blit_offscreen(&mut self, handle: &ImageHandle, dst_rect: Rect) {
        let Some(offscreen_canvas) = self.offscreens.get(handle) else {
            return;
        };
        let surf = offscreen_canvas.surface();
        let src_rect = Rect::new(0.0, 0.0, surf.width() as f32, surf.height() as f32);
        self.blit_offscreen_src(handle, src_rect, dst_rect);
    }

    fn blit_offscreen_src(&mut self, handle: &ImageHandle, src_rect: Rect, dst_rect: Rect) {
        if let Some(dst_id) = self.active_offscreen {
            if dst_id == handle.0 {
                return;
            }
            let Some((pixels, pw)) = self.offscreens.copy_pixels(handle) else {
                return;
            };
            let dst_handle = ImageHandle(dst_id);
            if let Some(dst_canvas) = self.offscreens.canvas_mut(&dst_handle) {
                dst_canvas.blit_image(&pixels, pw, src_rect, dst_rect);
            }
            return;
        }
        self.blit_offscreen_impl(handle, src_rect, dst_rect);
    }

    fn try_blit_offscreen_src(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
    ) -> Result<(), Error> {
        if self.offscreens.get(handle).is_none() {
            return Err(Error::new(
                crate::core::Errc::InvalidState,
                "Picture offscreen target does not exist before blit",
            ));
        }
        if let Some(dst_id) = self.active_offscreen {
            if dst_id == handle.0 {
                return Err(Error::new(
                    crate::core::Errc::InvalidArgument,
                    "Picture offscreen target cannot blit into itself",
                ));
            }
            let dst_handle = ImageHandle(dst_id);
            let Some((pixels, pw)) = self.offscreens.copy_pixels(handle) else {
                return Err(Error::new(
                    crate::core::Errc::InvalidState,
                    "Picture offscreen pixels disappeared before nested blit",
                ));
            };
            let Some(dst_canvas) = self.offscreens.canvas_mut(&dst_handle) else {
                return Err(Error::new(
                    crate::core::Errc::InvalidState,
                    "active Picture offscreen target disappeared before blit",
                ));
            };
            dst_canvas.blit_image(&pixels, pw, src_rect, dst_rect);
            return Ok(());
        }
        self.blit_offscreen_impl(handle, src_rect, dst_rect);
        Ok(())
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
        RenderBackend::blit_offscreen(self, handle, dst_rect);
    }

    pub fn blit_offscreen_src(&mut self, handle: &ImageHandle, src_rect: Rect, dst_rect: Rect) {
        RenderBackend::blit_offscreen_src(self, handle, src_rect, dst_rect);
    }

    /// 将离屏缓冲内容 blit 到任意 Canvas2D（支持嵌套 Picture 合成）。
    pub fn blit_offscreen_to_canvas(
        &self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
        canvas: &mut dyn crate::draw::traits::Canvas2D,
    ) {
        let Some(offscreen_canvas) = self.offscreens.get(handle) else {
            return;
        };
        let surf = offscreen_canvas.surface();
        canvas.blit_image(surf.pixels(), surf.width(), src_rect, dst_rect);
    }

    /// 复制离屏像素（避免与 offscreen_canvas 可变借用冲突）。
    pub fn copy_offscreen_pixels(&self, handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        self.offscreens.copy_pixels(handle)
    }

    pub fn memory_usage(&self) -> usize {
        let surf = self.main.surface();
        let main_bytes = (surf.width() * surf.height() * 4) as usize;
        main_bytes + self.offscreens.memory_usage()
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
        assert_eq!(
            backend.offscreens.slot_len(),
            2,
            "slot vec must not grow on reuse"
        );
        backend.destroy_offscreen(b);
        backend.destroy_offscreen(c);
    }

    #[test]
    fn encoded_picture_replaces_cpu_offscreen_pixels_and_begin_clears_stale_content() {
        use crate::draw::pipeline::{
            EncodedPictureExecution, FrameEncoder, FrameRasterOp, FrameRect,
        };

        let mut backend = CpuBackend::new();
        let handle = backend.create_offscreen(4, 3).expect("Picture target");
        {
            let canvas = backend.offscreen_canvas(&handle).expect("offscreen canvas");
            canvas.fill_rect(
                Rect::new(0.0, 0.0, 4.0, 3.0),
                Color::from_rgb(220, 10, 10),
                None,
            );
        }
        backend
            .try_begin_offscreen_paint(&handle)
            .expect("replace clear");
        assert!(backend
            .copy_offscreen_pixels(&handle)
            .expect("cleared pixels")
            .0
            .iter()
            .all(|&pixel| pixel == Color::transparent().premultiplied()));

        let mut encoder = FrameEncoder::new(4, 3).expect("encoder");
        encoder.clear(Color::transparent());
        encoder.cpu_segment([FrameRasterOp::FillRect {
            rect: FrameRect::new(1, 1, 2, 1),
            color: Color::from_rgba(20, 40, 200, 128),
        }]);
        assert_eq!(
            backend
                .try_execute_encoded_picture(&handle, &encoder)
                .expect("FrameEncoder execution"),
            EncodedPictureExecution::Executed
        );
        let pixels = backend
            .copy_offscreen_pixels(&handle)
            .expect("encoded pixels")
            .0;
        assert_eq!(
            pixels[1 + 4],
            Color::from_rgba(20, 40, 200, 128).premultiplied()
        );
        assert_eq!(pixels[0], Color::transparent().premultiplied());
    }
}

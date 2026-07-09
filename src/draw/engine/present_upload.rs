//! Engine-managed presentation through a native GPU context.
//!
//! This engine keeps the existing CPU Canvas2D raster path, then uploads the
//! frame pixels to a non-GL swapchain at present time.

use crate::core::{Error, Rect};
use crate::draw::backend::{BackendKind, CpuBackend, DamageRegion};
use crate::draw::engine::RenderOutcome;
use crate::draw::pipeline::RenderSession;
use crate::draw::primitives::color::Color;
use crate::draw::traits::{Canvas2D, GraphicsCapabilities, GraphicsEngine, UpdateStrategy};
use crate::draw::ImageHandle;
use crate::native::traits::present::{IGraphicsContext, PresentFrame};

pub struct PresentUploadEngine {
    session: RenderSession,
    gpu_ctx: Box<dyn IGraphicsContext>,
    pub clear_color: Color,
}

impl PresentUploadEngine {
    pub fn new(gpu_ctx: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        let session = match RenderSession::new(BackendKind::Cpu) {
            Ok(session) => session,
            Err(err) => {
                let mut ctx = gpu_ctx;
                ctx.shutdown();
                return Err(err);
            }
        };
        Ok(Self {
            session,
            gpu_ctx,
            clear_color: Color::from_rgba(0, 0, 0, 0),
        })
    }

    pub fn backend_name(&self) -> &'static str {
        self.gpu_ctx.graphics_backend().as_str()
    }

    fn sync_clear_color(&mut self) {
        if let Some(cpu) = self.session.cpu_backend_mut() {
            cpu.set_clear_color(self.clear_color);
        }
    }

    fn cpu(&mut self) -> Option<&mut CpuBackend> {
        self.sync_clear_color();
        self.session.cpu_backend_mut()
    }
}

impl GraphicsEngine for PresentUploadEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        let logical_w = width.max(1);
        let logical_h = height.max(1);
        self.gpu_ctx
            .initialize(std::ptr::null_mut(), logical_w, logical_h)?;
        // Canvas2D / 布局使用逻辑像素；物理尺寸仅由 GPU present 路径消费。
        self.sync_clear_color();
        self.session.initialize(logical_w, logical_h)
    }

    fn shutdown(&mut self) {
        self.session.shutdown();
        self.gpu_ctx.shutdown();
    }

    fn resize(&mut self, width: i32, height: i32) {
        let logical_w = width.max(1);
        let logical_h = height.max(1);
        self.gpu_ctx.resize(logical_w, logical_h);
        self.sync_clear_color();
        self.session.resize(logical_w, logical_h);
    }

    fn begin_frame(&mut self, _strategy: UpdateStrategy) -> RenderOutcome {
        self.sync_clear_color();
        self.session.begin_frame(UpdateStrategy::FullRedraw)
    }

    fn end_frame(&mut self, present_damage: &DamageRegion) -> RenderOutcome {
        let outcome = self.session.end_frame();
        if let Some(cpu) = self.session.cpu_backend() {
            let frame = PresentFrame::PixelBuffer {
                pixels: cpu.pixels(),
                width: cpu.width(),
                height: cpu.height(),
                damage: present_damage.to_present_damage(),
            };
            if let Err(err) = self.gpu_ctx.present(&frame) {
                crate::core::log::error_fn(format!(
                    "PresentUploadEngine {} present failed: {}",
                    self.backend_name(),
                    err.short_what()
                ));
            }
        }
        outcome
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.session.canvas_2d()
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        GraphicsCapabilities::engine_managed_full_redraw()
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.gpu_ctx.device_pixel_ratio()
    }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        self.session.backend_mut().create_offscreen(width, height)
    }

    fn destroy_offscreen(&mut self, handle: ImageHandle) {
        self.session.backend_mut().destroy_offscreen(handle);
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.session.backend_mut().offscreen_canvas(handle)
    }

    fn blit_offscreen(&mut self, handle: &ImageHandle, dst_rect: Rect) {
        if let Some(cpu) = self.cpu() {
            cpu.blit_offscreen(handle, dst_rect);
        }
    }

    fn blit_offscreen_src(&mut self, handle: &ImageHandle, src_rect: Rect, dst_rect: Rect) {
        if let Some(cpu) = self.cpu() {
            cpu.blit_offscreen_src(handle, src_rect, dst_rect);
        }
    }

    fn blit_offscreen_to_canvas(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
        canvas: &mut dyn Canvas2D,
    ) {
        if let Some(cpu) = self.session.cpu_backend() {
            cpu.blit_offscreen_to_canvas(handle, src_rect, dst_rect, canvas);
        }
    }

    fn copy_offscreen_pixels(&self, handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        self.session.cpu_backend()?.copy_offscreen_pixels(handle)
    }

    fn memory_usage(&self) -> usize {
        self.session
            .cpu_backend()
            .map(|cpu| cpu.memory_usage())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Result;
    use crate::native::traits::present::{
        GraphicsBackend, GraphicsContextCaps, IGraphicsContext, PresentDamage,
    };
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Debug, PartialEq)]
    struct PresentedFrame {
        width: i32,
        height: i32,
        len: usize,
        damage: PresentDamage,
    }

    #[derive(Default)]
    struct RecordingPixelContext {
        frame: Arc<Mutex<Option<PresentedFrame>>>,
        width: i32,
        height: i32,
    }

    impl RecordingPixelContext {
        fn new(frame: Arc<Mutex<Option<PresentedFrame>>>) -> Self {
            Self {
                frame,
                width: 0,
                height: 0,
            }
        }
    }

    impl IGraphicsContext for RecordingPixelContext {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::cpu_pixel_upload(GraphicsBackend::D3d11, 1.0)
        }

        fn graphics_backend(&self) -> GraphicsBackend {
            GraphicsBackend::D3d11
        }

        fn initialize(
            &mut self,
            _native_window: *mut std::ffi::c_void,
            width: i32,
            height: i32,
        ) -> Result<()> {
            self.width = width;
            self.height = height;
            Ok(())
        }

        fn resize(&mut self, width: i32, height: i32) {
            self.width = width;
            self.height = height;
        }

        fn make_current(&mut self) {}

        fn swap_buffers(&mut self, _damage: PresentDamage) {}

        fn shutdown(&mut self) {}

        fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Vec<u32> {
            Vec::new()
        }

        fn width(&self) -> i32 {
            self.width
        }

        fn height(&self) -> i32 {
            self.height
        }

        fn present_pixels(
            &mut self,
            pixels: &[u32],
            width: i32,
            height: i32,
            damage: PresentDamage,
        ) -> Result<()> {
            *self.frame.lock().unwrap() = Some(PresentedFrame {
                width,
                height,
                len: pixels.len(),
                damage,
            });
            Ok(())
        }
    }

    #[test]
    fn present_upload_engine_submits_cpu_pixels_to_context() {
        let frame = Arc::new(Mutex::new(None));
        let context = RecordingPixelContext::new(frame.clone());
        let mut engine = PresentUploadEngine::new(Box::new(context)).unwrap();

        engine.initialize(4, 3).unwrap();
        let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
        let _ = engine.end_frame(&DamageRegion::full());

        assert_eq!(
            *frame.lock().unwrap(),
            Some(PresentedFrame {
                width: 4,
                height: 3,
                len: 12,
                damage: PresentDamage::Full,
            })
        );
        assert_eq!(
            engine.capabilities(),
            GraphicsCapabilities::engine_managed_full_redraw()
        );
    }
}

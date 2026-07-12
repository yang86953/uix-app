//! Engine-managed presentation through a native GPU context.
//!
//! This engine keeps the existing CPU Canvas2D raster path, then uploads the
//! frame pixels to a non-GL swapchain at present time.

use crate::core::{Error, Rect};
use crate::draw::ImageHandle;
use crate::draw::backend::{BackendKind, CpuBackend, DamageRegion};
use crate::draw::engine::{GraphicsFailure, RenderOutcome};
use crate::draw::pipeline::RenderSession;
use crate::draw::pipeline::{EncodedFrameExecution, EncodedPictureExecution, FrameEncoder};
use crate::draw::primitives::color::Color;
use crate::draw::traits::{Canvas2D, GraphicsCapabilities, GraphicsEngine, UpdateStrategy};
use crate::native::traits::present::{IGraphicsContext, PresentDamage, PresentFrame};

pub struct PresentUploadEngine {
    session: RenderSession,
    gpu_ctx: Box<dyn IGraphicsContext>,
    pub clear_color: Color,
    shutdown: bool,
}

impl PresentUploadEngine {
    pub(crate) fn new(gpu_ctx: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        let session = match RenderSession::new(BackendKind::Cpu) {
            Ok(session) => session,
            Err(err) => {
                let mut ctx = gpu_ctx;
                ctx.try_shutdown()?;
                return Err(err);
            }
        };
        Ok(Self {
            session,
            gpu_ctx,
            clear_color: Color::from_rgba(0, 0, 0, 0),
            shutdown: false,
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
    fn initialize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        // The native factory owns creation against the real surface. Calling
        // `IGraphicsContext::initialize` here would reinitialize a live
        // context with a null surface, so this stage only creates the CPU
        // draw session at the factory-reported drawable size.
        let actual_w = self.gpu_ctx.width().max(1);
        let actual_h = self.gpu_ctx.height().max(1);
        self.sync_clear_color();
        self.session.initialize(actual_w, actual_h)
    }

    fn shutdown(&mut self) {
        if self.shutdown {
            return;
        }
        self.shutdown = true;
        self.session.shutdown();
        self.gpu_ctx.shutdown();
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        let logical_w = width.max(1);
        let logical_h = height.max(1);
        self.gpu_ctx.resize(logical_w, logical_h)?;
        // 与 NativeGpuBackend 一致：CPU 表面跟 GPU 实际客户区对齐。
        let actual_w = self.gpu_ctx.width().max(1);
        let actual_h = self.gpu_ctx.height().max(1);
        self.sync_clear_color();
        self.session.resize(actual_w, actual_h)
    }

    fn begin_frame(&mut self, _strategy: UpdateStrategy) -> RenderOutcome {
        self.sync_clear_color();
        self.session.begin_frame(UpdateStrategy::FullRedraw)
    }

    fn end_frame(&mut self, present_damage: &DamageRegion) -> RenderOutcome {
        let outcome = self.session.end_frame();
        if matches!(outcome, RenderOutcome::Failed(_)) {
            return outcome;
        }
        if let Some(cpu) = self.session.cpu_backend() {
            let frame = PresentFrame::PixelBuffer {
                pixels: cpu.pixels(),
                width: cpu.width(),
                height: cpu.height(),
                // CPU upload has the same preservation precondition as a
                // swapchain present. Do not let a caller turn an unproven
                // partial-present context into a partial redraw path.
                damage: if self.gpu_ctx.caps().partial_present {
                    present_damage.to_present_damage()
                } else {
                    PresentDamage::Full
                },
            };
            if let Err(err) = self.gpu_ctx.present(&frame) {
                crate::core::log::error_fn(format!(
                    "PresentUploadEngine {} present failed: {}",
                    self.backend_name(),
                    err.short_what()
                ));
                return RenderOutcome::Failed(GraphicsFailure::from_error(err));
            }
        }
        outcome
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.session.canvas_2d()
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        // CPU 栅格 + GPU upload：离屏走 CpuBackend。
        GraphicsCapabilities::engine_managed_with_offscreen()
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

    fn try_destroy_offscreen(&mut self, handle: ImageHandle) -> Result<(), Error> {
        self.session.backend_mut().try_destroy_offscreen(handle)
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

    fn try_execute_encoded_picture(
        &mut self,
        handle: &ImageHandle,
        encoder: &FrameEncoder,
    ) -> Result<EncodedPictureExecution, Error> {
        self.session
            .backend_mut()
            .try_execute_encoded_picture(handle, encoder)
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        self.session
            .backend_mut()
            .try_execute_encoded_frame(encoder)
    }

    fn begin_offscreen_paint(&mut self, handle: &ImageHandle) -> bool {
        self.session.backend_mut().begin_offscreen_paint(handle)
    }

    fn try_begin_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.session.backend_mut().try_begin_offscreen_paint(handle)
    }

    fn flush_offscreen_paint(&mut self, handle: &ImageHandle) {
        self.session.backend_mut().flush_offscreen_paint(handle);
    }

    fn try_flush_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.session.backend_mut().try_flush_offscreen_paint(handle)
    }

    fn end_offscreen_paint(&mut self) {
        self.session.backend_mut().end_offscreen_paint();
    }

    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        self.session.backend_mut().try_end_offscreen_paint()
    }

    fn try_blit_offscreen_src(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
    ) -> Result<(), Error> {
        self.session
            .backend_mut()
            .try_blit_offscreen_src(handle, src_rect, dst_rect)
    }

    fn memory_usage(&self) -> usize {
        self.session
            .cpu_backend()
            .map(|cpu| cpu.memory_usage())
            .unwrap_or(0)
    }
}

impl Drop for PresentUploadEngine {
    fn drop(&mut self) {
        <Self as GraphicsEngine>::shutdown(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Result;
    use crate::native::traits::present::{
        GraphicsBackend, GraphicsContextCaps, IGraphicsContext, PresentDamage,
    };
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

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
        initialize_calls: Arc<AtomicUsize>,
        fail_present: bool,
        fail_resize: bool,
        partial_present: bool,
        shutdowns: Arc<AtomicUsize>,
    }

    impl RecordingPixelContext {
        fn new(frame: Arc<Mutex<Option<PresentedFrame>>>) -> Self {
            Self::with_extent(frame, 4, 3)
        }

        fn with_extent(frame: Arc<Mutex<Option<PresentedFrame>>>, width: i32, height: i32) -> Self {
            Self {
                frame,
                width,
                height,
                initialize_calls: Arc::new(AtomicUsize::new(0)),
                fail_present: false,
                fail_resize: false,
                partial_present: false,
                shutdowns: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl IGraphicsContext for RecordingPixelContext {
        fn caps(&self) -> GraphicsContextCaps {
            let mut caps = GraphicsContextCaps::cpu_pixel_upload(GraphicsBackend::D3d11, 1.0);
            caps.partial_present = self.partial_present;
            caps
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
            self.initialize_calls.fetch_add(1, Ordering::SeqCst);
            self.width = width;
            self.height = height;
            Ok(())
        }

        fn resize(&mut self, width: i32, height: i32) -> crate::core::Result<()> {
            if self.fail_resize {
                return Err(Error::new(
                    crate::core::error::Errc::GraphicsSurfaceLost,
                    "injected resize failure",
                ));
            }
            self.width = width;
            self.height = height;

            Ok(())
        }

        fn make_current(&mut self) -> crate::core::Result<()> {
            Ok(())
        }

        fn swap_buffers(&mut self, _damage: PresentDamage) -> crate::core::Result<()> {
            Ok(())
        }

        fn shutdown(&mut self) {
            self.shutdowns.fetch_add(1, Ordering::SeqCst);
        }

        fn read_pixels(
            &mut self,
            _x: i32,
            _y: i32,
            _width: i32,
            _height: i32,
        ) -> crate::core::Result<Vec<u32>> {
            Ok(Vec::new())
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
            if self.fail_present {
                return Err(Error::new(
                    crate::core::error::Errc::PlatformError,
                    "injected pixel-present failure",
                ));
            }
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
    fn present_upload_engine_uses_factory_initialized_context_without_second_initialize() {
        let frame = Arc::new(Mutex::new(None));
        let context = RecordingPixelContext::with_extent(frame, 7, 5);
        let initialize_calls = Arc::clone(&context.initialize_calls);
        let mut engine = PresentUploadEngine::new(Box::new(context)).unwrap();

        engine.initialize(64, 48).unwrap();

        assert_eq!(initialize_calls.load(Ordering::SeqCst), 0);
        assert_eq!((engine.session.width(), engine.session.height()), (7, 5));
    }

    #[test]
    fn present_upload_engine_does_not_report_present_after_context_failure() {
        let frame = Arc::new(Mutex::new(None));
        let mut context = RecordingPixelContext::new(frame.clone());
        context.fail_present = true;
        let mut engine = PresentUploadEngine::new(Box::new(context)).unwrap();

        engine.initialize(4, 3).unwrap();
        let _ = engine.begin_frame(UpdateStrategy::FullRedraw);

        assert!(matches!(
            engine.end_frame(&DamageRegion::full()),
            RenderOutcome::Failed(GraphicsFailure::Other(error))
                if error.code() == crate::core::error::Errc::PlatformError
        ));
        assert_eq!(*frame.lock().unwrap(), None);
    }

    #[test]
    fn present_upload_engine_propagates_resize_failure_without_committing_new_extent() {
        let frame = Arc::new(Mutex::new(None));
        let mut context = RecordingPixelContext::new(frame);
        context.fail_resize = true;
        let mut engine = PresentUploadEngine::new(Box::new(context)).unwrap();
        engine.initialize(4, 3).unwrap();

        let error = engine.resize(8, 6).expect_err("resize must be observable");

        assert_eq!(error.code(), crate::core::error::Errc::GraphicsSurfaceLost);
        assert_eq!((engine.session.width(), engine.session.height()), (4, 3));
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
            GraphicsCapabilities::engine_managed_with_offscreen()
        );
    }

    #[test]
    fn present_upload_upgrades_partial_damage_when_context_does_not_prove_preservation() {
        let frame = Arc::new(Mutex::new(None));
        let context = RecordingPixelContext::new(frame.clone());
        let mut engine = PresentUploadEngine::new(Box::new(context)).unwrap();

        engine.initialize(4, 3).unwrap();
        let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
        let _ = engine.end_frame(&DamageRegion::partial(vec![Rect::new(1.0, 1.0, 2.0, 1.0)]));

        assert_eq!(
            frame
                .lock()
                .unwrap()
                .as_ref()
                .map(|presented| &presented.damage),
            Some(&PresentDamage::Full)
        );
    }

    #[test]
    fn present_upload_forwards_partial_damage_only_when_context_proves_preservation() {
        let frame = Arc::new(Mutex::new(None));
        let mut context = RecordingPixelContext::new(frame.clone());
        context.partial_present = true;
        let mut engine = PresentUploadEngine::new(Box::new(context)).unwrap();

        engine.initialize(4, 3).unwrap();
        let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
        let _ = engine.end_frame(&DamageRegion::partial(vec![Rect::new(1.0, 1.0, 2.0, 1.0)]));

        assert_eq!(
            frame
                .lock()
                .unwrap()
                .as_ref()
                .map(|presented| &presented.damage),
            Some(&PresentDamage::Partial(vec![(1, 1, 2, 1)]))
        );
    }

    #[test]
    fn present_upload_shutdown_and_drop_release_context_once() {
        let frame = Arc::new(Mutex::new(None));
        let context = RecordingPixelContext::new(frame);
        let shutdowns = context.shutdowns.clone();
        let mut engine = PresentUploadEngine::new(Box::new(context)).unwrap();

        engine.shutdown();
        drop(engine);

        assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
    }
}

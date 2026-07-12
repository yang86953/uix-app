// ============================================================================
// draw/gpu_engine/mod.rs — GPU 渲染引擎（GLES 3.0）
// ============================================================================

use crate::core::Error;
use crate::draw::backend::DamageRegion;
use crate::draw::backend::registry::create_native_raster_backend;
use crate::draw::engine::{GraphicsFailure, RenderOutcome};
use crate::draw::pipeline::{
    EncodedFrameExecution, EncodedPictureExecution, FrameEncoder, RenderSession,
};
use crate::draw::traits::{Canvas2D, GraphicsCapabilities, GraphicsEngine, UpdateStrategy};
use crate::native::traits::present::IGraphicsContext;

// Compatibility names retain their former public draw path while the source
// of truth is native OpenGL ES. They do not expose a GL object or loader.
#[cfg(feature = "opengles")]
pub use crate::native::graphics::opengl::shaders::{
    BLIT_FRAG, BLIT_RGBA_FRAG, BLUR_FRAG, FULLSCREEN_VERT, RECT_FRAG, RECT_VERT,
};
/// GPU 渲染引擎 — 委托 `RenderSession` + `RenderBackend`（GL / D3D11 / …）。
pub struct GpuEngine {
    session: RenderSession,
}

impl GpuEngine {
    pub(crate) fn new(gpu_ctx: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        let session = match create_native_raster_backend(gpu_ctx) {
            Ok(backend) => RenderSession::with_backend(backend),
            Err(err) => return Err(err),
        };
        Ok(Self { session })
    }

    pub fn session(&self) -> &RenderSession {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut RenderSession {
        &mut self.session
    }

    fn make_current(&mut self) -> Result<(), Error> {
        self.session.backend_mut().make_current()
    }
}

impl GraphicsEngine for GpuEngine {
    fn initialize(&mut self, w: i32, h: i32) -> Result<(), Error> {
        self.session.initialize_prepared(w, h)?;
        self.make_current()?;
        Ok(())
    }

    fn shutdown(&mut self) {
        self.session.shutdown();
    }

    fn resize(&mut self, w: i32, h: i32) -> Result<(), Error> {
        self.session.resize(w, h)?;
        self.make_current()?;
        Ok(())
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        if let Err(error) = self.make_current() {
            return RenderOutcome::Failed(GraphicsFailure::from_error(error));
        }
        self.session.begin_frame(strategy)
    }

    fn end_frame(&mut self, present_damage: &DamageRegion) -> RenderOutcome {
        let outcome = self.session.end_frame();
        if !matches!(outcome, RenderOutcome::Present(_)) {
            return outcome;
        }
        if let Err(error) = self.session.backend_mut().present(present_damage) {
            crate::core::log::error_fn("GpuEngine present 失败");
            return RenderOutcome::Failed(GraphicsFailure::from_error(error));
        }
        outcome
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.session.canvas_2d()
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        self.session.graphics_capabilities()
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.session.backend().device_pixel_ratio()
    }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<crate::draw::ImageHandle> {
        self.session.backend_mut().create_offscreen(width, height)
    }

    fn destroy_offscreen(&mut self, handle: crate::draw::ImageHandle) {
        self.session.backend_mut().destroy_offscreen(handle);
    }

    fn try_destroy_offscreen(&mut self, handle: crate::draw::ImageHandle) -> Result<(), Error> {
        self.session.backend_mut().try_destroy_offscreen(handle)
    }

    fn offscreen_canvas(&mut self, handle: &crate::draw::ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.session.backend_mut().offscreen_canvas(handle)
    }

    fn copy_offscreen_pixels(&self, handle: &crate::draw::ImageHandle) -> Option<(Vec<u32>, i32)> {
        self.session.backend().copy_offscreen_pixels(handle)
    }

    fn try_execute_encoded_picture(
        &mut self,
        handle: &crate::draw::ImageHandle,
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

    fn begin_offscreen_paint(&mut self, handle: &crate::draw::ImageHandle) -> bool {
        self.session.backend_mut().begin_offscreen_paint(handle)
    }

    fn try_begin_offscreen_paint(
        &mut self,
        handle: &crate::draw::ImageHandle,
    ) -> Result<(), Error> {
        self.session.backend_mut().try_begin_offscreen_paint(handle)
    }

    fn flush_offscreen_paint(&mut self, handle: &crate::draw::ImageHandle) {
        self.session.backend_mut().flush_offscreen_paint(handle);
    }

    fn try_flush_offscreen_paint(
        &mut self,
        handle: &crate::draw::ImageHandle,
    ) -> Result<(), Error> {
        self.session.backend_mut().try_flush_offscreen_paint(handle)
    }

    fn end_offscreen_paint(&mut self) {
        self.session.backend_mut().end_offscreen_paint();
    }

    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        self.session.backend_mut().try_end_offscreen_paint()
    }

    fn blit_offscreen(&mut self, handle: &crate::draw::ImageHandle, dst_rect: crate::core::Rect) {
        self.session.backend_mut().blit_offscreen(handle, dst_rect);
    }

    fn blit_offscreen_src(
        &mut self,
        handle: &crate::draw::ImageHandle,
        src_rect: crate::core::Rect,
        dst_rect: crate::core::Rect,
    ) {
        self.session
            .backend_mut()
            .blit_offscreen_src(handle, src_rect, dst_rect);
    }

    fn try_blit_offscreen_src(
        &mut self,
        handle: &crate::draw::ImageHandle,
        src_rect: crate::core::Rect,
        dst_rect: crate::core::Rect,
    ) -> Result<(), Error> {
        self.session
            .backend_mut()
            .try_blit_offscreen_src(handle, src_rect, dst_rect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::traits::present::{
        GraphicsBackend, GraphicsContextCaps, NativeRasterCaps, PresentDamage,
    };
    use std::cell::Cell;
    use std::ffi::c_void;
    use std::rc::Rc;

    struct FactoryInitializedNativeContext {
        resize_calls: Rc<Cell<usize>>,
        width: i32,
        height: i32,
    }

    impl IGraphicsContext for FactoryInitializedNativeContext {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::D3d11, false, 1.0)
        }

        fn native_raster_caps(&self) -> NativeRasterCaps {
            NativeRasterCaps::d3d11_full()
        }

        fn initialize(
            &mut self,
            _native_window: *mut c_void,
            _width: i32,
            _height: i32,
        ) -> crate::core::Result<()> {
            panic!("factory-initialized context must not be initialized by GpuEngine")
        }

        fn resize(&mut self, width: i32, height: i32) -> crate::core::Result<()> {
            self.resize_calls.set(self.resize_calls.get() + 1);
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

        fn shutdown(&mut self) {}

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
    }

    #[test]
    fn gpu_native_engine_uses_factory_drawable_without_initial_resize() {
        let resize_calls = Rc::new(Cell::new(0));
        let context = FactoryInitializedNativeContext {
            resize_calls: Rc::clone(&resize_calls),
            width: 7,
            height: 5,
        };
        let mut engine = GpuEngine::new(Box::new(context)).expect("GpuNative engine");

        engine.initialize(640, 480).expect("engine initialization");

        assert_eq!(resize_calls.get(), 0);
        assert_eq!(
            (engine.session().width(), engine.session().height()),
            (7, 5)
        );

        engine.resize(8, 6).expect("subsequent native resize");
        assert_eq!(resize_calls.get(), 1);
        assert_eq!(
            (engine.session().width(), engine.session().height()),
            (8, 6)
        );
    }

    #[cfg(feature = "opengles")]
    #[test]
    fn legacy_draw_module_reexports_native_shader_sources() {
        use crate::native::graphics::opengl::shaders;

        assert_eq!(RECT_VERT, shaders::RECT_VERT);
        assert_eq!(RECT_FRAG, shaders::RECT_FRAG);
        assert_eq!(BLUR_FRAG, shaders::BLUR_FRAG);
        assert_eq!(BLIT_FRAG, shaders::BLIT_FRAG);
        assert_eq!(BLIT_RGBA_FRAG, shaders::BLIT_RGBA_FRAG);
        assert_eq!(FULLSCREEN_VERT, shaders::FULLSCREEN_VERT);
    }
}

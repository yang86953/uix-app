// ============================================================================
// draw/gpu_engine/mod.rs — GPU 渲染引擎（GLES 3.0）
// ============================================================================

#[cfg(feature = "opengles")]
use glow::HasContext as _;
use std::cell::RefCell;

use crate::core::Error;
use crate::draw::backend::registry::create_native_raster_backend;
use crate::draw::backend::DamageRegion;
use crate::draw::engine::{GraphicsFailure, RenderOutcome};
use crate::draw::pipeline::RenderSession;
use crate::draw::traits::{Canvas2D, GraphicsCapabilities, GraphicsEngine, UpdateStrategy};
use crate::native::traits::present::IGraphicsContext;

#[cfg(feature = "opengles")]
pub use canvas_2d::GpuCanvas2D;
#[cfg(feature = "opengles")]
pub use shaders::*;
#[cfg(feature = "opengles")]
mod canvas_2d;
#[cfg(feature = "opengles")]
mod shaders;

/// GPU 渲染引擎 — 委托 `RenderSession` + `RenderBackend`（GL / D3D11 / …）。
pub struct GpuEngine {
    session: RenderSession,
    empty_readback: RefCell<Vec<u32>>,
}

impl GpuEngine {
    pub(crate) fn new(gpu_ctx: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        let session = match create_native_raster_backend(gpu_ctx) {
            Ok(backend) => RenderSession::with_backend(backend),
            Err(err) => return Err(err),
        };
        Ok(Self {
            session,
            empty_readback: RefCell::new(Vec::new()),
        })
    }

    pub fn session(&self) -> &RenderSession {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut RenderSession {
        &mut self.session
    }

    /// 返回像素缓冲的克隆（每次调用分配，仅用于读回）。
    pub fn pixels(&self) -> Vec<u32> {
        #[cfg(feature = "opengles")]
        {
            return self
                .session
                .gpu_backend()
                .map(|g| g.pixels())
                .unwrap_or_default();
        }
        #[cfg(not(feature = "opengles"))]
        {
            Vec::new()
        }
    }

    /// 返回像素缓冲的引用（避免分配）。
    pub fn pixels_ref(&self) -> std::cell::Ref<'_, Vec<u32>> {
        #[cfg(feature = "opengles")]
        {
            if let Some(gpu) = self.session.gpu_backend() {
                return gpu.pixels_ref();
            }
        }
        self.empty_readback.borrow()
    }

    /// 从 GL 前端缓冲读回像素数据（填充 readback）。
    pub fn read_pixels(&self) {
        #[cfg(feature = "opengles")]
        {
            if let Some(gpu) = self.session.gpu_backend() {
                gpu.read_pixels();
            }
        }
    }

    fn make_current(&mut self) -> Result<(), Error> {
        self.session.backend_mut().make_current()
    }
}

impl GraphicsEngine for GpuEngine {
    fn initialize(&mut self, w: i32, h: i32) -> Result<(), Error> {
        self.session.initialize_prepared(w, h)?;
        self.make_current()?;
        #[cfg(feature = "opengles")]
        if let Some(gpu) = self.session.gpu_backend_mut() {
            let vw = gpu.gpu_ctx.width();
            let vh = gpu.gpu_ctx.height();
            unsafe {
                gpu.gl().viewport(0, 0, vw, vh);
                gpu.gl().enable(glow::BLEND);
                gpu.gl()
                    .blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            }
        }
        Ok(())
    }

    fn shutdown(&mut self) {
        self.session.shutdown();
    }

    fn resize(&mut self, w: i32, h: i32) -> Result<(), Error> {
        self.session.resize(w, h)?;
        self.make_current()?;
        #[cfg(feature = "opengles")]
        if let Some(gpu) = self.session.gpu_backend_mut() {
            unsafe {
                gpu.gl()
                    .viewport(0, 0, gpu.gpu_ctx.width(), gpu.gpu_ctx.height());
            }
        }
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

    fn offscreen_canvas(&mut self, handle: &crate::draw::ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.session.backend_mut().offscreen_canvas(handle)
    }

    fn copy_offscreen_pixels(&self, handle: &crate::draw::ImageHandle) -> Option<(Vec<u32>, i32)> {
        self.session.backend().copy_offscreen_pixels(handle)
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

        fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Vec<u32> {
            Vec::new()
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
}

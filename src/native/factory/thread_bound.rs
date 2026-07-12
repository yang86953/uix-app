//! Factory-owned thread affinity for native graphics contexts (#183).
//!
//! Native APIs such as WGL, EGL, D3D and Vulkan associate a context with the
//! creating thread.  Every production context therefore leaves the registry
//! wrapped in this type.  The `Rc` marker deliberately makes the wrapper
//! neither `Send` nor `Sync`; safe Rust cannot transfer it to another thread.
//! Result-returning graphics operations also validate the owner and report an
//! `InvalidState` error instead of reaching an API object from the wrong
//! thread.
//!
//! `IGraphicsContext` still has four legacy non-fallible operations
//! (`shutdown`, `destroy_offscreen_target`, readback and metadata access).
//! They cannot report a typed error without changing that public trait.  A
//! violation on those operations is loud (panic), except `shutdown`, which
//! records the failure because it can be reached by `Drop`.  This is an
//! intentional compatibility boundary, not a successful no-op.

use std::cell::Cell;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::rc::Rc;
use std::thread::{self, ThreadId};

use crate::core::{Errc, Error, Result};
use crate::native::traits::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSolidMesh,
    GpuSolidRect, GpuStrokeRect, GraphicsContextCaps, IGraphicsContext, NativeRasterCaps,
    OffscreenTargetId, PresentDamage, PresentFrame, SoftFallbackTile,
};

pub(crate) fn bind_to_current_thread(
    inner: Box<dyn IGraphicsContext>,
) -> Box<dyn IGraphicsContext> {
    Box::new(ThreadBoundGraphicsContext::new(inner))
}

struct ThreadBoundGraphicsContext {
    owner_thread: ThreadId,
    inner: Box<dyn IGraphicsContext>,
    /// Read-only metadata is captured on the creation thread and refreshed
    /// only after successful owner-thread lifecycle changes. These legacy
    /// non-fallible queries can therefore never touch a native context from a
    /// foreign thread.
    caps: GraphicsContextCaps,
    native_raster_caps: NativeRasterCaps,
    width: i32,
    height: i32,
    device_pixel_ratio: f32,
    // `Rc` is intentionally !Send + !Sync. `Cell` makes the intent equally
    // explicit to readers inspecting the wrapper's auto-trait boundary.
    _thread_bound: PhantomData<Rc<Cell<()>>>,
}

impl ThreadBoundGraphicsContext {
    fn new(inner: Box<dyn IGraphicsContext>) -> Self {
        let caps = inner.caps();
        let native_raster_caps = inner.native_raster_caps();
        let width = inner.width();
        let height = inner.height();
        let device_pixel_ratio = inner.device_pixel_ratio();
        Self {
            owner_thread: thread::current().id(),
            inner,
            caps,
            native_raster_caps,
            width,
            height,
            device_pixel_ratio,
            _thread_bound: PhantomData,
        }
    }

    fn refresh_metadata(&mut self) {
        self.caps = self.inner.caps();
        self.native_raster_caps = self.inner.native_raster_caps();
        self.width = self.inner.width();
        self.height = self.inner.height();
        self.device_pixel_ratio = self.inner.device_pixel_ratio();
    }

    fn require_owner(&self, operation: &str) -> Result<()> {
        let current = thread::current().id();
        if current == self.owner_thread {
            return Ok(());
        }
        Err(Error::new(
            Errc::InvalidState,
            format!(
                "graphics context operation {operation} must run on creation thread {:?}; current thread is {:?}",
                self.owner_thread, current
            ),
        ))
    }

    fn with_owner<T>(
        &mut self,
        operation: &str,
        run: impl FnOnce(&mut dyn IGraphicsContext) -> Result<T>,
    ) -> Result<T> {
        self.require_owner(operation)?;
        run(self.inner.as_mut())
    }

    fn require_owner_or_panic(&self, operation: &str) {
        if let Err(error) = self.require_owner(operation) {
            panic!(
                "non-fallible graphics context operation violated thread affinity: {}",
                error.what()
            );
        }
    }

    #[cfg(test)]
    fn with_test_owner(inner: Box<dyn IGraphicsContext>, owner_thread: ThreadId) -> Self {
        let mut bound = Self::new(inner);
        bound.owner_thread = owner_thread;
        bound
    }
}

macro_rules! forward_result {
    ($name:ident($($argument:ident : $argument_type:ty),* $(,)?) -> $output:ty) => {
        fn $name(&mut self, $($argument: $argument_type),*) -> Result<$output> {
            self.with_owner(stringify!($name), |inner| inner.$name($($argument),*))
        }
    };
}

impl IGraphicsContext for ThreadBoundGraphicsContext {
    fn caps(&self) -> GraphicsContextCaps {
        self.caps
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        self.native_raster_caps
    }

    fn initialize(&mut self, native_window: *mut c_void, width: i32, height: i32) -> Result<()> {
        self.with_owner("initialize", |inner| {
            inner.initialize(native_window, width, height)
        })?;
        self.refresh_metadata();
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        self.with_owner("resize", |inner| inner.resize(width, height))?;
        self.refresh_metadata();
        Ok(())
    }

    forward_result!(make_current() -> ());
    forward_result!(swap_buffers(damage: PresentDamage) -> ());

    fn shutdown(&mut self) {
        match self.require_owner("shutdown") {
            Ok(()) => self.inner.shutdown(),
            Err(error) => crate::core::log::error_fn(format!(
                "graphics context shutdown rejected: {}",
                error.what()
            )),
        }
    }

    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Vec<u32> {
        self.require_owner_or_panic("read_pixels");
        self.inner.read_pixels(x, y, width, height)
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
        self.with_owner("present_pixels", |inner| {
            inner.present_pixels(pixels, width, height, damage)
        })
    }

    forward_result!(present(frame: &PresentFrame) -> ());

    fn device_pixel_ratio(&self) -> f32 {
        self.device_pixel_ratio
    }

    fn get_proc_address(&self, name: &str) -> Option<*const c_void> {
        self.require_owner_or_panic("get_proc_address");
        self.inner.get_proc_address(name)
    }

    forward_result!(clear_render_target(r: f32, g: f32, b: f32, a: f32) -> ());
    forward_result!(upload_surface_pixels(pixels: &[u32], width: i32, height: i32) -> ());
    forward_result!(draw_solid_rects(viewport_w: f32, viewport_h: f32, scissor: Option<(i32, i32, i32, i32)>, rects: &[GpuSolidRect]) -> ());
    forward_result!(draw_stroke_rects(viewport_w: f32, viewport_h: f32, scissor: Option<(i32, i32, i32, i32)>, rects: &[GpuStrokeRect]) -> ());
    forward_result!(draw_glyphs(viewport_w: f32, viewport_h: f32, scissor: Option<(i32, i32, i32, i32)>, glyphs: &[GpuGlyphBlit]) -> ());
    forward_result!(draw_linear_gradients(viewport_w: f32, viewport_h: f32, scissor: Option<(i32, i32, i32, i32)>, rects: &[GpuLinearGradientRect]) -> ());
    forward_result!(draw_radial_gradients(viewport_w: f32, viewport_h: f32, scissor: Option<(i32, i32, i32, i32)>, grads: &[GpuRadialGradient]) -> ());
    forward_result!(draw_solid_meshes(viewport_w: f32, viewport_h: f32, scissor: Option<(i32, i32, i32, i32)>, meshes: &[GpuSolidMesh]) -> ());
    forward_result!(draw_box_shadows(viewport_w: f32, viewport_h: f32, scissor: Option<(i32, i32, i32, i32)>, shadows: &[GpuBoxShadow]) -> ());
    forward_result!(blit_soft_fallback(pixels: &[u32], width: i32, height: i32) -> ());
    forward_result!(blit_soft_fallback_tile(pixels: &[u32], surface_width: i32, surface_height: i32, tile: SoftFallbackTile) -> ());
    forward_result!(clear_rects(viewport_w: f32, viewport_h: f32, rects: &[GpuSolidRect]) -> ());
    forward_result!(create_offscreen_target(width: i32, height: i32) -> OffscreenTargetId);

    fn destroy_offscreen_target(&mut self, id: OffscreenTargetId) {
        self.require_owner_or_panic("destroy_offscreen_target");
        self.inner.destroy_offscreen_target(id);
    }

    forward_result!(bind_offscreen_target(id: OffscreenTargetId) -> ());
    forward_result!(bind_swapchain_target() -> ());
    forward_result!(blit_offscreen_target(id: OffscreenTargetId, src: crate::core::Rect, dst: crate::core::Rect) -> ());
}

#[cfg(test)]
mod tests {
    use super::ThreadBoundGraphicsContext;
    use crate::core::{Errc, Result};
    use crate::native::traits::present::{
        GraphicsBackend, GraphicsContextCaps, IGraphicsContext, PresentDamage, PresentFrame,
    };
    use std::ffi::c_void;

    struct PanicIfCalled;

    impl IGraphicsContext for PanicIfCalled {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::cpu_pixel_upload(GraphicsBackend::D3d11, 1.0)
        }

        fn initialize(
            &mut self,
            _native_window: *mut c_void,
            _width: i32,
            _height: i32,
        ) -> Result<()> {
            panic!("foreign thread must not call the native context")
        }

        fn resize(&mut self, _width: i32, _height: i32) -> Result<()> {
            panic!("foreign thread must not call the native context")
        }

        fn make_current(&mut self) -> Result<()> {
            panic!("foreign thread must not call the native context")
        }

        fn swap_buffers(&mut self, _damage: PresentDamage) -> Result<()> {
            panic!("foreign thread must not call the native context")
        }

        fn shutdown(&mut self) {}

        fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Vec<u32> {
            Vec::new()
        }

        fn width(&self) -> i32 {
            1
        }

        fn height(&self) -> i32 {
            1
        }

        fn present(&mut self, _frame: &PresentFrame) -> Result<()> {
            panic!("foreign thread must not call the native context")
        }
    }

    #[test]
    fn wrong_owner_returns_typed_errors_before_native_calls() {
        let foreign_owner = std::thread::spawn(|| std::thread::current().id())
            .join()
            .expect("thread id");
        let mut context =
            ThreadBoundGraphicsContext::with_test_owner(Box::new(PanicIfCalled), foreign_owner);

        let initialize = context
            .initialize(std::ptr::null_mut(), 1, 1)
            .expect_err("owner mismatch");
        assert_eq!(initialize.code(), Errc::InvalidState);
        assert!(initialize.message().contains("initialize"));

        let present = context
            .present(&PresentFrame::Swapchain {
                damage: PresentDamage::Full,
            })
            .expect_err("owner mismatch");
        assert_eq!(present.code(), Errc::InvalidState);
        assert!(present.message().contains("present"));

        let offscreen = context
            .create_offscreen_target(4, 4)
            .expect_err("owner mismatch");
        assert_eq!(offscreen.code(), Errc::InvalidState);
        assert!(offscreen.message().contains("create_offscreen_target"));

        // Legacy metadata access cannot return a typed failure, so it must be
        // served from the creation-thread snapshot rather than touch native
        // state on this foreign thread.
        assert_eq!(context.caps().backend, GraphicsBackend::D3d11);
        assert_eq!(context.width(), 1);
        assert_eq!(context.height(), 1);
        assert_eq!(context.device_pixel_ratio(), 1.0);
    }
}

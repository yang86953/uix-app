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
//! The former non-fallible lifecycle operations now have checked `try_*`
//! counterparts.  The legacy hooks remain compatibility adapters while each
//! platform implementation migrates; they log and return a safe empty value
//! on an owner violation rather than panicking or reaching native state.

use std::cell::Cell;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::thread::{self, ThreadId};

use crate::core::{Errc, Error, Result};
use crate::native::traits::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSolidMesh,
    GpuSolidRect, GpuStrokeRect, GraphicsContextCaps, IGraphicsContext, NativeRasterCaps,
    OffscreenTargetId, PresentDamage, PresentFrame, PresentTestResult, SoftFallbackTile,
};

pub(crate) fn bind_to_current_thread(
    inner: Box<dyn IGraphicsContext>,
) -> Box<dyn IGraphicsContext> {
    Box::new(ThreadBoundGraphicsContext::new(inner))
}

pub(crate) struct ThreadBoundGraphicsContext {
    owner_thread: ThreadId,
    inner: ManuallyDrop<Box<dyn IGraphicsContext>>,
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
            inner: ManuallyDrop::new(inner),
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

    fn log_legacy_rejection(operation: &str, error: &Error) {
        crate::core::log::error_fn(format!(
            "legacy graphics context {operation} rejected: {}",
            error.what()
        ));
    }

    pub(crate) fn with_test_owner(
        inner: Box<dyn IGraphicsContext>,
        owner_thread: ThreadId,
    ) -> Self {
        let mut bound = Self::new(inner);
        bound.owner_thread = owner_thread;
        bound
    }
}

impl Drop for ThreadBoundGraphicsContext {
    fn drop(&mut self) {
        // SAFETY: Drop runs once; the inner Box is taken exactly once here.
        let mut inner = unsafe { ManuallyDrop::take(&mut self.inner) };
        if self.require_owner("drop").is_err() {
            // Wrong-thread Drop must not tear down native API objects. Leak the
            // context so its Drop cannot run on this foreign thread.
            Self::log_legacy_rejection(
                "drop",
                &Error::new(
                    Errc::InvalidState,
                    format!(
                        "graphics context operation drop must run on creation thread {:?}; current thread is {:?}",
                        self.owner_thread,
                        thread::current().id()
                    ),
                ),
            );
            std::mem::forget(inner);
            return;
        }
        if let Err(error) = inner.try_shutdown() {
            Self::log_legacy_rejection("drop", &error);
        }
        drop(inner);
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

    fn try_shutdown(&mut self) -> Result<()> {
        self.with_owner("try_shutdown", |inner| inner.try_shutdown())
    }

    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u32>> {
        self.with_owner("read_pixels", |inner| {
            inner.read_pixels(x, y, width, height)
        })
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
    forward_result!(test_present() -> PresentTestResult);

    fn device_pixel_ratio(&self) -> f32 {
        self.device_pixel_ratio
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
    forward_result!(blit_soft_fallback_tile(pixels: &[u32], tile: SoftFallbackTile) -> ());
    forward_result!(clear_rects(viewport_w: f32, viewport_h: f32, rects: &[GpuSolidRect]) -> ());
    forward_result!(create_offscreen_target(width: i32, height: i32) -> OffscreenTargetId);

    fn try_destroy_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<()> {
        self.with_owner("try_destroy_offscreen_target", |inner| {
            inner.try_destroy_offscreen_target(id)
        })
    }

    fn destroy_offscreen_target(&mut self, id: OffscreenTargetId) {
        if let Err(error) = self.try_destroy_offscreen_target(id) {
            Self::log_legacy_rejection("destroy_offscreen_target", &error);
        }
    }

    forward_result!(bind_offscreen_target(id: OffscreenTargetId) -> ());
    forward_result!(bind_swapchain_target() -> ());
    forward_result!(blit_offscreen_target(id: OffscreenTargetId, src: crate::core::Rect, dst: crate::core::Rect) -> ());
}

//! 渲染后端模块。

pub mod cpu;
#[cfg(feature = "opengles")]
pub mod gpu;
pub mod native_gpu;
pub mod null;
pub mod offscreen_pool;
pub mod registry;
pub mod traits;

pub use crate::core::DamageRegion;
pub use cpu::CpuBackend;
#[cfg(feature = "opengles")]
pub use gpu::GpuBackend;
pub use native_gpu::NativeGpuBackend;
pub use null::NullBackend;
pub use traits::{BackendCapabilities, BackendKind, DrawSurface, RenderBackend};

use crate::core::{Errc, Error};
use crate::native::traits::present::IGraphicsContext;

/// 按种类创建后端实例。
pub(crate) fn create_backend(
    kind: BackendKind,
    gpu_ctx: Option<Box<dyn IGraphicsContext>>,
) -> Result<Box<dyn RenderBackend>, Error> {
    match kind {
        BackendKind::Cpu | BackendKind::Auto => Ok(Box::new(CpuBackend::new())),
        BackendKind::Null => Ok(Box::new(NullBackend::new())),
        BackendKind::Gpu => {
            let ctx = gpu_ctx.ok_or_else(|| {
                Error::new(Errc::InvalidArgument, "Gpu 后端需要 IGraphicsContext")
            })?;
            registry::create_native_raster_backend(ctx)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::traits::present::{
        GraphicsBackend, GraphicsContextCaps, NativeRasterCaps, PresentDamage,
    };
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct FakeGpuContext {
        backend: GraphicsBackend,
        native_caps: NativeRasterCaps,
        gl_compatible: bool,
        shutdowns: Arc<AtomicUsize>,
    }

    impl IGraphicsContext for FakeGpuContext {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::gpu_native_swapchain(self.backend, false, 1.0)
        }

        fn native_raster_caps(&self) -> NativeRasterCaps {
            self.native_caps
        }

        fn supports_gl_proc_address(&self) -> bool {
            self.gl_compatible
        }

        fn initialize(
            &mut self,
            _native_window: *mut c_void,
            _width: i32,
            _height: i32,
        ) -> crate::core::Result<()> {
            Ok(())
        }

        fn resize(&mut self, _width: i32, _height: i32) -> crate::core::Result<()> {
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

        fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Vec<u32> {
            Vec::new()
        }

        fn width(&self) -> i32 {
            1
        }

        fn height(&self) -> i32 {
            1
        }
    }

    #[test]
    fn gpu_factory_routes_d3d11_context_to_native_gpu_backend() {
        let context = FakeGpuContext {
            backend: GraphicsBackend::D3d11,
            native_caps: NativeRasterCaps::d3d11_full(),
            gl_compatible: false,
            shutdowns: Arc::new(AtomicUsize::new(0)),
        };

        let backend = create_backend(BackendKind::Gpu, Some(Box::new(context)))
            .expect("D3D11 context should use NativeGpuBackend");

        assert!(backend.as_any().is::<NativeGpuBackend>());
    }

    #[cfg(feature = "opengles")]
    #[test]
    fn gpu_factory_routes_opengles_context_to_gl_backend() {
        let shutdowns = Arc::new(AtomicUsize::new(0));
        let context = FakeGpuContext {
            backend: GraphicsBackend::OpenGlEs,
            native_caps: NativeRasterCaps::default(),
            // Stop before glow initialization while retaining a GL-specific
            // observable error, so the test does not need a live GL driver.
            gl_compatible: false,
            shutdowns: Arc::clone(&shutdowns),
        };

        let error = match create_backend(BackendKind::Gpu, Some(Box::new(context))) {
            Ok(_) => panic!("incompatible fake GL context must fail GL backend construction"),
            Err(error) => error,
        };

        assert!(error.message().contains("requires a GL-compatible"));
        assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
    }
}

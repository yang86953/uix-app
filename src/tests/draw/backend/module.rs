use crate::draw::backend::*;
use crate::tests::common::*;
use std::ffi::c_void;

struct FakeGpuContext {
    backend: GraphicsBackend,
    native_caps: NativeRasterCaps,
    shutdowns: Arc<AtomicUsize>,
}

impl IGraphicsContext for FakeGpuContext {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(self.backend, PresentCoherency::FullOnly, 1.0)
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        self.native_caps
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

    fn try_shutdown(&mut self) -> crate::core::Result<()> {
        self.shutdowns.fetch_add(1, Ordering::SeqCst);
        Ok(())
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
        shutdowns: Arc::new(AtomicUsize::new(0)),
    };

    let backend = create_backend(BackendKind::Gpu, Some(Box::new(context)))
        .expect("D3D11 context should use NativeGpuBackend");

    assert!(backend.as_any().is::<NativeGpuBackend>());
}

#[cfg(feature = "opengles")]
#[test]
fn gpu_factory_rejects_opengles_context_without_native_hybrid_baseline() {
    let shutdowns = Arc::new(AtomicUsize::new(0));
    let context = FakeGpuContext {
        backend: GraphicsBackend::OpenGlEs,
        native_caps: NativeRasterCaps::default(),
        shutdowns: Arc::clone(&shutdowns),
    };

    let error = match create_backend(BackendKind::Gpu, Some(Box::new(context))) {
        Ok(_) => panic!("context without hybrid baseline must be rejected"),
        Err(error) => error,
    };

    assert!(error.message().contains("requires GpuNative"));
    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
}

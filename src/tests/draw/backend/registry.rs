use crate::core::Result;
use crate::draw::backend::registry::*;
use crate::tests::common::*;
use std::ffi::c_void;

struct UploadOnlyContext;

impl IGraphicsContext for UploadOnlyContext {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::cpu_pixel_upload(GraphicsBackend::D3d11, 1.0)
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
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
        Ok(())
    }
    fn read_pixels(&mut self, _x: i32, _y: i32, _w: i32, _h: i32) -> crate::core::Result<Vec<u32>> {
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
fn rejects_non_gpu_native_raster() {
    let err = match create_native_raster_backend(Box::new(UploadOnlyContext)) {
        Ok(_) => panic!("expected error"),
        Err(err) => err,
    };
    assert!(err.message().contains("GpuNative"));
}

struct FakeD3d11GpuNative;

impl IGraphicsContext for FakeD3d11GpuNative {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(
            GraphicsBackend::D3d11,
            PresentCoherency::FullOnly,
            1.0,
        )
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        NativeRasterCaps::d3d11_full()
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
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
        Ok(())
    }
    fn read_pixels(&mut self, _x: i32, _y: i32, _w: i32, _h: i32) -> crate::core::Result<Vec<u32>> {
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
fn creates_d3d11_backend_for_gpu_native_caps() {
    let backend = create_native_raster_backend(Box::new(FakeD3d11GpuNative))
        .expect("D3D11 GpuNative should assemble NativeGpuBackend");
    assert_eq!(backend.kind(), crate::draw::backend::BackendKind::Gpu);
}

struct FakeD3d12GpuNative {
    caps: NativeRasterCaps,
    shutdowns: Rc<Cell<usize>>,
}

impl IGraphicsContext for FakeD3d12GpuNative {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(
            GraphicsBackend::D3d12,
            PresentCoherency::FullOnly,
            1.0,
        )
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        self.caps
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
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
        self.shutdowns.set(self.shutdowns.get() + 1);
        Ok(())
    }
    fn read_pixels(&mut self, _x: i32, _y: i32, _w: i32, _h: i32) -> crate::core::Result<Vec<u32>> {
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
fn creates_shared_native_backend_for_d3d12_hybrid_caps() {
    let shutdowns = Rc::new(Cell::new(0));
    let backend = create_native_raster_backend(Box::new(FakeD3d12GpuNative {
        caps: NativeRasterCaps {
            clear_target: true,
            soft_blit: true,
            solid_rects: true,
            ..NativeRasterCaps::default()
        },
        shutdowns: Rc::clone(&shutdowns),
    }))
    .expect("D3D12 should assemble the shared NativeGpuBackend");
    assert_eq!(backend.kind(), crate::draw::backend::BackendKind::Gpu);
    drop(backend);
    assert_eq!(shutdowns.get(), 1);
}

#[test]
fn rejects_d3d12_without_soft_blit_and_shuts_down_once() {
    let shutdowns = Rc::new(Cell::new(0));
    let error = match create_native_raster_backend(Box::new(FakeD3d12GpuNative {
        caps: NativeRasterCaps {
            clear_target: true,
            solid_rects: true,
            ..NativeRasterCaps::default()
        },
        shutdowns: Rc::clone(&shutdowns),
    })) {
        Ok(_) => panic!("missing soft blit must reject D3D12 native backend"),
        Err(error) => error,
    };
    assert_eq!(error.code(), Errc::InvalidArgument);
    assert_eq!(shutdowns.get(), 1);
}

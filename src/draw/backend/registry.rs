//! Table-driven pairing of [`GraphicsBackend`] to [`RenderBackend`] (P6.7 M6 / P6.8).

use crate::core::{Errc, Error, Result};
use crate::draw::backend::gpu::GpuBackend;
use crate::draw::backend::native_gpu::NativeGpuBackend;
use crate::draw::backend::traits::RenderBackend;
use crate::native::traits::present::{GraphicsBackend, IGraphicsContext, RasterMode};

/// Creates the GPU raster backend for a native graphics context.
///
/// Only used when `caps().raster == GpuNative`; upload-present paths
/// keep a fixed CPU raster backend.
pub fn create_native_raster_backend(
    mut ctx: Box<dyn IGraphicsContext>,
) -> Result<Box<dyn RenderBackend>, Error> {
    if ctx.caps().raster != RasterMode::GpuNative {
        let raster = ctx.caps().raster;
        ctx.shutdown();
        return Err(Error::new(
            Errc::InvalidArgument,
            format!("RenderBackendRegistry: expected RasterMode::GpuNative, got {raster}"),
        ));
    }

    match ctx.caps().backend {
        GraphicsBackend::OpenGlEs => GpuBackend::new(ctx).map(|backend| Box::new(backend) as _),
        GraphicsBackend::D3d11 => NativeGpuBackend::new(ctx).map(|backend| Box::new(backend) as _),
        GraphicsBackend::Metal => {
            ctx.shutdown();
            Err(Error::new(
                Errc::NotImplemented,
                "RenderBackendRegistry: Metal native raster is planned (use Cpu × PixelUpload context)",
            ))
        }
        GraphicsBackend::D3d12 => {
            ctx.shutdown();
            Err(Error::new(
                Errc::NotImplemented,
                "RenderBackendRegistry: D3D12 native raster is planned",
            ))
        }
        other => {
            ctx.shutdown();
            Err(Error::new(
                Errc::InvalidArgument,
                format!("RenderBackendRegistry: no native raster backend for {other}"),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::traits::present::{GraphicsContextCaps, NativeRasterCaps, PresentDamage};
    use std::ffi::c_void;

    struct UploadOnlyContext;

    impl IGraphicsContext for UploadOnlyContext {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::cpu_pixel_upload(GraphicsBackend::D3d11, 1.0)
        }

        fn initialize(
            &mut self,
            _native_window: *mut c_void,
            _width: i32,
            _height: i32,
        ) -> Result<()> {
            Ok(())
        }

        fn resize(&mut self, _width: i32, _height: i32) {}
        fn make_current(&mut self) {}
        fn swap_buffers(&mut self, _damage: PresentDamage) {}
        fn shutdown(&mut self) {}
        fn read_pixels(&mut self, _x: i32, _y: i32, _w: i32, _h: i32) -> Vec<u32> {
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
        ) -> Result<()> {
            Ok(())
        }

        fn resize(&mut self, _width: i32, _height: i32) {}
        fn make_current(&mut self) {}
        fn swap_buffers(&mut self, _damage: PresentDamage) {}
        fn shutdown(&mut self) {}
        fn read_pixels(&mut self, _x: i32, _y: i32, _w: i32, _h: i32) -> Vec<u32> {
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
    fn creates_d3d11_backend_for_gpu_native_caps() {
        let backend = create_native_raster_backend(Box::new(FakeD3d11GpuNative))
            .expect("D3D11 GpuNative should assemble NativeGpuBackend");
        assert_eq!(backend.kind(), crate::draw::backend::BackendKind::Gpu);
    }
}

//! Table-driven pairing of [`GraphicsBackend`] to [`RenderBackend`] (P6.7 M6).

use crate::core::{Errc, Error, Result};
use crate::draw::backend::gpu::GpuBackend;
use crate::draw::backend::traits::RenderBackend;
use crate::native::traits::present::{GraphicsBackend, IGraphicsContext, RenderPipelineProfile};

/// Creates the GPU raster backend for a native graphics context.
///
/// Only used when `caps().pipeline == NativeGpuRaster`; upload-present paths
/// keep a fixed CPU raster backend.
pub fn create_native_raster_backend(
    mut ctx: Box<dyn IGraphicsContext>,
) -> Result<Box<dyn RenderBackend>, Error> {
    match ctx.caps().pipeline {
        RenderPipelineProfile::NativeGpuRaster => match ctx.caps().backend {
            GraphicsBackend::OpenGlEs => GpuBackend::new(ctx).map(|backend| Box::new(backend) as _),
            GraphicsBackend::Metal => {
                ctx.shutdown();
                Err(Error::new(
                    Errc::NotImplemented,
                    "RenderBackendRegistry: Metal native raster is planned (use CpuUploadPresent context)",
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
                    format!(
                        "RenderBackendRegistry: no native raster backend for {other}"
                    ),
                ))
            }
        },
        other => {
            ctx.shutdown();
            Err(Error::new(
                Errc::InvalidArgument,
                format!("RenderBackendRegistry: expected NativeGpuRaster, got {other}"),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::traits::present::{
        GraphicsContextCaps, PresentDamage,
    };
    use std::ffi::c_void;

    struct UploadOnlyContext;

    impl IGraphicsContext for UploadOnlyContext {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::cpu_upload_present(GraphicsBackend::D3d11, 1.0)
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
    fn rejects_non_native_gpu_pipeline() {
        let err = match create_native_raster_backend(Box::new(UploadOnlyContext)) {
            Ok(_) => panic!("expected error"),
            Err(err) => err,
        };
        assert!(err.message().contains("NativeGpuRaster"));
    }
}

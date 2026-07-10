//! Maps a native [`IGraphicsContext`] to the draw [`GraphicsEngine`] via orthogonal axes.

use crate::core::{Errc, Error, Result};
use crate::draw::engine::present_upload::PresentUploadEngine;
use crate::draw::gpu_engine::GpuEngine;
use crate::draw::traits::GraphicsEngine;
use crate::native::traits::present::{IGraphicsContext, PresentMode, RasterMode};

/// Creates the draw engine that matches `context.caps().raster` × `present`.
///
/// Legal combinations (table-driven):
/// - `GpuNative` × `Swapchain` → [`GpuEngine`]
/// - `Cpu` × `PixelUpload` → [`PresentUploadEngine`]
///
/// `PresentMode::CpuPresenter` has no [`IGraphicsContext`] and is rejected here
/// (app builds [`crate::draw::SoftwareEngine`] + [`crate::native::traits::IPresenter`]).
///
/// On failure the context is shut down before the error is returned (M3).
pub fn create_graphics_engine(
    mut context: Box<dyn IGraphicsContext>,
) -> Result<Box<dyn GraphicsEngine>, Error> {
    let caps = context.caps();
    match (caps.raster, caps.present) {
        (RasterMode::Cpu, PresentMode::PixelUpload) => PresentUploadEngine::new(context)
            .map(|engine| Box::new(engine) as Box<dyn GraphicsEngine>),
        (RasterMode::GpuNative, PresentMode::Swapchain) => {
            GpuEngine::new(context).map(|engine| Box::new(engine) as Box<dyn GraphicsEngine>)
        }
        (raster, PresentMode::CpuPresenter) => {
            context.shutdown();
            Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "create_graphics_engine: PresentMode::CpuPresenter has no IGraphicsContext \
                     (raster={raster})"
                ),
            ))
        }
        (raster, present) => {
            context.shutdown();
            Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "create_graphics_engine: unsupported RasterMode × PresentMode \
                     combination: {raster} × {present}"
                ),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::traits::present::{
        GraphicsBackend, GraphicsContextCaps, IGraphicsContext, NativeRasterCaps, PresentDamage,
    };
    use std::ffi::c_void;

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
            64
        }
        fn height(&self) -> i32 {
            48
        }
    }

    #[test]
    fn gpu_native_swapchain_assembles_gpu_engine_for_d3d11() {
        let mut engine =
            create_graphics_engine(Box::new(FakeD3d11GpuNative)).expect("GpuEngine for D3D11");
        engine.initialize(64, 48).expect("initialize");
        engine.shutdown();
    }
}

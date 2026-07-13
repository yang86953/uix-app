//! Metal identity PixelUpload context — CAMetalLayer CPU upload path.
//!
//! Native GPU raster via Metal remains planned. This context declares
//! [`RasterMode::Cpu`] × [`PresentMode::PixelUpload`] and does not own a Metal
//! device, command queue, or raster pipeline.

use std::ffi::c_void;

use crate::core::{Errc, Error, Result};
use crate::native::backends::macos::platform;
use crate::native::traits::present::{
    GraphicsBackend, GraphicsContextCaps, IGraphicsContext, PresentDamage, PresentFrame,
    validate_pixel_buffer,
};

type LayerId = *mut c_void;

pub struct MetalPixelUploadContext {
    layer: LayerId,
    width: i32,
    height: i32,
    device_pixel_ratio: f32,
    initialized: bool,
}

impl MetalPixelUploadContext {
    pub(crate) fn new(native_surface: *mut c_void, width: i32, height: i32) -> Result<Self> {
        if native_surface.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "MetalPixelUploadContext: native CAMetalLayer surface is null",
            ));
        }
        Ok(Self {
            layer: native_surface,
            width: width.max(1),
            height: height.max(1),
            device_pixel_ratio: 1.0,
            initialized: false,
        })
    }

    fn shutdown_result(&mut self) -> Result<()> {
        self.initialized = false;
        Ok(())
    }
}

impl IGraphicsContext for MetalPixelUploadContext {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::cpu_pixel_upload(GraphicsBackend::Metal, self.device_pixel_ratio)
    }

    fn initialize(&mut self, _native_window: *mut c_void, width: i32, height: i32) -> Result<()> {
        self.width = width.max(1);
        self.height = height.max(1);
        self.initialized = true;
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        self.width = width.max(1);
        self.height = height.max(1);
        Ok(())
    }

    fn make_current(&mut self) -> Result<()> {
        Ok(())
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> Result<()> {
        Err(Error::new(
            Errc::NotImplemented,
            "MetalPixelUploadContext: swapchain present requires native Metal raster (planned)",
        ))
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
    }

    fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Result<Vec<u32>> {
        Err(Error::new(
            Errc::NotImplemented,
            "MetalPixelUploadContext: native readback is not supported",
        ))
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
        if !self.initialized {
            return Err(Error::new(
                Errc::InvalidArgument,
                "MetalPixelUploadContext: present before initialize",
            ));
        }
        validate_pixel_buffer(pixels, width, height)?;
        // SAFETY: layer pointer comes from AppKit-owned CAMetalLayer on the UI thread.
        unsafe {
            platform::present_layer_pixels(self.layer, pixels, width, height, damage)?;
        }
        Ok(())
    }

    fn present(&mut self, frame: &PresentFrame) -> Result<()> {
        match frame {
            PresentFrame::PixelBuffer {
                pixels,
                width,
                height,
                damage,
            } => self.present_pixels(pixels, *width, *height, damage.clone()),
            PresentFrame::Swapchain { .. } => Err(Error::new(
                Errc::NotImplemented,
                "MetalPixelUploadContext: swapchain present requires native Metal raster (planned)",
            )),
        }
    }
}

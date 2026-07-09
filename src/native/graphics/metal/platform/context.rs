//! Metal graphics context — CALayer CPU upload path.
//!
//! Native GPU raster via Metal RenderBackend is planned; this context declares
//! [`RasterMode::Cpu`] × [`PresentMode::PixelUpload`] so macOS can bootstrap a
//! GPU present path without staying on pure SoftwareEngine.

use std::ffi::c_void;

use crate::core::{Errc, Error, Result};
use crate::native::backends::macos::platform;
use crate::native::traits::present::{
    GraphicsBackend, GraphicsContextCaps, IGraphicsContext, PresentDamage, PresentFrame,
};

type LayerId = *mut c_void;

pub struct MetalContext {
    layer: LayerId,
    width: i32,
    height: i32,
    device_pixel_ratio: f32,
    initialized: bool,
}

impl MetalContext {
    pub fn new(native_surface: *mut c_void, width: i32, height: i32) -> Result<Self> {
        if native_surface.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "MetalContext: native CALayer surface is null",
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
}

impl IGraphicsContext for MetalContext {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::cpu_pixel_upload(GraphicsBackend::Metal, self.device_pixel_ratio)
    }

    fn initialize(&mut self, _native_window: *mut c_void, width: i32, height: i32) -> Result<()> {
        self.width = width.max(1);
        self.height = height.max(1);
        self.initialized = true;
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) {
        self.width = width.max(1);
        self.height = height.max(1);
    }

    fn make_current(&mut self) {}

    fn swap_buffers(&mut self, _damage: PresentDamage) {}

    fn shutdown(&mut self) {
        self.initialized = false;
    }

    fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Vec<u32> {
        Vec::new()
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
                "MetalContext: present before initialize",
            ));
        }
        // SAFETY: layer pointer comes from AppKit-owned CALayer on the UI thread.
        unsafe {
            platform::present_layer_pixels(self.layer, pixels, width, height, damage);
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
                "MetalContext: swapchain present requires native Metal raster (planned)",
            )),
        }
    }
}

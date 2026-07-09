//! Presentation contracts for CPU presenters and GPU graphics contexts.

use crate::core::error::{Error, Result};
pub use crate::core::PresentDamage;
use std::fmt;
use std::str::FromStr;

/// Unified present payload for [`IGraphicsContext::present`] (M7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresentFrame<'a> {
    /// GPU swapchain / equivalent (native raster path).
    Swapchain {
        damage: PresentDamage,
    },
    /// CPU raster upload (upload-present path).
    PixelBuffer {
        pixels: &'a [u32],
        width: i32,
        height: i32,
        damage: PresentDamage,
    },
}

/// CPU pixel presenter.
pub trait IPresenter {
    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        damage: PresentDamage,
    ) -> Result<(), Error>;

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error>;
}

/// How a graphics context participates in the render/present pipeline.
///
/// **Deprecated for removal** ([#169](docs/decisions.md#d169)): bundled preset bundling
/// raster + present. Target dispatch is `RasterMode` × `PresentMode` only — do not
/// add variants or aliases.
///
/// Distinct from [`GraphicsBackend`] (which API) — answers how frames are rasterized
/// and presented. Defined in native so `caps()` does not depend on draw types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderPipelineProfile {
    /// GPU-native raster + swapchain / equivalent present.
    NativeGpuRaster,
    /// CPU Canvas2D raster uploaded via [`IGraphicsContext::present_pixels`].
    CpuUploadPresent,
    /// Pure CPU + [`IPresenter`]; no [`IGraphicsContext`] (app/bootstrap only).
    CpuPresenter,
}

impl fmt::Display for RenderPipelineProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NativeGpuRaster => "native_gpu_raster",
            Self::CpuUploadPresent => "cpu_upload_present",
            Self::CpuPresenter => "cpu_presenter",
        })
    }
}

/// Native-side capability snapshot for a live [`IGraphicsContext`].
///
/// Does not replace draw's `GraphicsCapabilities`; engine code derives
/// presentation scheduling from [`RenderPipelineProfile`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphicsContextCaps {
    pub backend: GraphicsBackend,
    pub pipeline: RenderPipelineProfile,
    /// Whether this context can present partial damage regions natively.
    pub partial_present: bool,
    pub device_pixel_ratio: f32,
}

impl GraphicsContextCaps {
    pub fn native_gpu_raster(
        backend: GraphicsBackend,
        partial_present: bool,
        device_pixel_ratio: f32,
    ) -> Self {
        Self {
            backend,
            pipeline: RenderPipelineProfile::NativeGpuRaster,
            partial_present,
            device_pixel_ratio,
        }
    }

    pub fn cpu_upload_present(backend: GraphicsBackend, device_pixel_ratio: f32) -> Self {
        Self {
            backend,
            pipeline: RenderPipelineProfile::CpuUploadPresent,
            partial_present: false,
            device_pixel_ratio,
        }
    }
}

/// Concrete GPU API selected by the native factory.
///
/// This is diagnostic and init-time selection data; draw continues to expose
/// `BackendKind::Gpu` as the stable rendering-pipeline choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphicsBackend {
    Auto,
    D3d12,
    D3d11,
    Vulkan,
    Metal,
    OpenGlEs,
}

impl GraphicsBackend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::D3d12 => "d3d12",
            Self::D3d11 => "d3d11",
            Self::Vulkan => "vulkan",
            Self::Metal => "metal",
            Self::OpenGlEs => "opengles",
        }
    }
}

impl fmt::Display for GraphicsBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for GraphicsBackend {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '_', ' '], "");
        match normalized.as_str() {
            "" | "auto" => Ok(Self::Auto),
            "d3d12" | "direct3d12" | "directx12" => Ok(Self::D3d12),
            "d3d11" | "direct3d11" | "directx11" => Ok(Self::D3d11),
            "vulkan" | "vk" => Ok(Self::Vulkan),
            "metal" => Ok(Self::Metal),
            "opengles" | "gles" | "gl" => Ok(Self::OpenGlEs),
            _ => Err(Error::new(
                crate::core::error::Errc::InvalidArgument,
                format!("unknown graphics backend: {value}"),
            )),
        }
    }
}

/// GPU graphics context lifecycle and presentation contract.
pub trait IGraphicsContext {
    fn caps(&self) -> GraphicsContextCaps;

    fn graphics_backend(&self) -> GraphicsBackend {
        self.caps().backend
    }

    fn initialize(
        &mut self,
        native_window: *mut std::ffi::c_void,
        width: i32,
        height: i32,
    ) -> Result<(), Error>;

    fn resize(&mut self, width: i32, height: i32);
    fn make_current(&mut self);
    fn swap_buffers(&mut self, damage: PresentDamage);
    fn shutdown(&mut self);
    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Vec<u32>;
    fn width(&self) -> i32;
    fn height(&self) -> i32;

    fn supports_gl_proc_address(&self) -> bool {
        self.caps().pipeline == RenderPipelineProfile::NativeGpuRaster
    }

    fn supports_pixel_present(&self) -> bool {
        self.caps().pipeline == RenderPipelineProfile::CpuUploadPresent
    }

    fn present_pixels(
        &mut self,
        _pixels: &[u32],
        _width: i32,
        _height: i32,
        _damage: PresentDamage,
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support CPU pixel present",
                self.graphics_backend()
            ),
        ))
    }

    /// Unified present entry (M7). Default forwards to legacy methods.
    fn present(&mut self, frame: &PresentFrame) -> Result<(), Error> {
        match frame {
            PresentFrame::Swapchain { damage } => {
                self.make_current();
                self.swap_buffers(damage.clone());
                Ok(())
            }
            PresentFrame::PixelBuffer {
                pixels,
                width,
                height,
                damage,
            } => self.present_pixels(pixels, *width, *height, damage.clone()),
        }
    }

    /// Drawable pixels per logical client pixel (HiDPI). Default `1.0`.
    fn device_pixel_ratio(&self) -> f32 {
        1.0
    }

    fn get_proc_address(&self, name: &str) -> Option<*const std::ffi::c_void> {
        let _ = name;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::GraphicsBackend;
    use std::str::FromStr;

    #[test]
    fn graphics_backend_parses_common_aliases() {
        assert_eq!(
            GraphicsBackend::from_str("direct3d-12").unwrap(),
            GraphicsBackend::D3d12
        );
        assert_eq!(
            GraphicsBackend::from_str("directx_11").unwrap(),
            GraphicsBackend::D3d11
        );
        assert_eq!(
            GraphicsBackend::from_str("OpenGL ES").unwrap(),
            GraphicsBackend::OpenGlEs
        );
        assert_eq!(
            GraphicsBackend::from_str("vk").unwrap(),
            GraphicsBackend::Vulkan
        );
    }

    #[test]
    fn graphics_backend_display_uses_config_tokens() {
        assert_eq!(GraphicsBackend::Auto.to_string(), "auto");
        assert_eq!(GraphicsBackend::OpenGlEs.to_string(), "opengles");
        assert_eq!(GraphicsBackend::Metal.as_str(), "metal");
    }
}

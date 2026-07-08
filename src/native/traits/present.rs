//! Presentation contracts for CPU presenters and GPU graphics contexts.

use crate::core::error::{Error, Result};
pub use crate::core::PresentDamage;
use std::fmt;
use std::str::FromStr;

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
    fn graphics_backend(&self) -> GraphicsBackend;

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

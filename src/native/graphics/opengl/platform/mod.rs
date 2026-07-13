//! Platform selection for OpenGL ES contexts.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::traits::present::IGraphicsContext;

/// The damage extension alone does not prove buffer preservation or buffer-age
/// semantics, so EGL must not advertise partial present yet.
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) const EGL_PARTIAL_PRESENT: bool = false;

#[cfg(all(unix, not(target_os = "macos")))]
pub mod egl;
#[cfg(windows)]
pub mod wgl;

#[cfg(all(unix, not(target_os = "macos")))]
pub use egl::EglContext;
#[cfg(windows)]
pub use wgl::WglContext;

#[cfg(windows)]
pub(super) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    WglContext::new(surface, width, height).map(|ctx| Box::new(ctx) as _)
}

#[cfg(all(test, unix, not(target_os = "macos")))]
#[path = "../../../../tests/native/graphics/opengl/platform/mod.rs"]
mod tests;

#[cfg(all(unix, not(target_os = "macos")))]
pub(super) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    EglContext::new(surface, width, height).map(|ctx| Box::new(ctx) as _)
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
pub(super) fn create(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    use crate::core::Errc;

    Err(Error::new(
        Errc::PlatformError,
        "GraphicsBackend opengles is not supported on this platform",
    ))
}

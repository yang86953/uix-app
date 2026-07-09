//! OpenGL ES graphics contexts (WGL on Windows, EGL on Linux).

#[cfg(windows)]
pub mod wgl;

#[cfg(all(unix, not(target_os = "macos")))]
pub mod egl;

#[cfg(windows)]
pub use wgl::WglContext;

#[cfg(all(unix, not(target_os = "macos")))]
pub use egl::EglContext;

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::traits::present::IGraphicsContext;

#[cfg(windows)]
pub fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    WglContext::new(surface, width, height).map(|ctx| Box::new(ctx) as _)
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    EglContext::new(surface, width, height).map(|ctx| Box::new(ctx) as _)
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
pub fn create(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(Error::new(
        Errc::PlatformError,
        "GraphicsBackend opengles is not supported on this platform",
    ))
}

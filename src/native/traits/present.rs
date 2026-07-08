//! Presentation contracts for CPU presenters and GPU graphics contexts.

use crate::core::error::{Error, Result};
pub use crate::core::PresentDamage;

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

/// GPU graphics context lifecycle and presentation contract.
pub trait IGraphicsContext {
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

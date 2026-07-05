//! Presentation contracts for CPU presenters and GPU graphics contexts.

use crate::core::error::{Error, Result};

/// Screen damage submitted with a CPU present or GPU buffer swap.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PresentDamage {
    /// Redraw the full surface.
    #[default]
    Full,
    /// One or more damaged device-pixel rectangles `(x, y, w, h)`.
    Partial(Vec<(i32, i32, i32, i32)>),
}

impl PresentDamage {
    pub fn single(x: i32, y: i32, w: i32, h: i32) -> Self {
        if w <= 0 || h <= 0 {
            Self::Full
        } else {
            Self::Partial(vec![(x, y, w, h)])
        }
    }

    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }
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

    fn get_proc_address(&self, name: &str) -> Option<*const std::ffi::c_void> {
        let _ = name;
        None
    }
}

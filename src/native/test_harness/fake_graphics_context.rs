//! Fake 图形上下文 — 空操作实现，记录调用。

use crate::core::error::Result;
use crate::native::traits::present::{IGraphicsContext, PresentDamage};
use std::cell::Cell;

#[derive(Debug, Clone)]
pub struct FakeGraphicsContextState {
    pub width: Cell<i32>,
    pub height: Cell<i32>,
    pub initialized: bool,
    pub make_current_calls: usize,
    pub swap_buffers_calls: usize,
    pub last_swap_damage: Option<PresentDamage>,
    pub shutdown_called: bool,
}

impl FakeGraphicsContextState {
    fn new(width: i32, height: i32) -> Self {
        Self {
            width: Cell::new(width),
            height: Cell::new(height),
            initialized: false,
            make_current_calls: 0,
            swap_buffers_calls: 0,
            last_swap_damage: None,
            shutdown_called: false,
        }
    }
}

#[derive(Debug)]
pub struct FakeGraphicsContext {
    pub state: FakeGraphicsContextState,
}

impl FakeGraphicsContext {
    pub fn new() -> Self {
        Self {
            state: FakeGraphicsContextState::new(0, 0),
        }
    }

    pub fn with_size(width: i32, height: i32) -> Self {
        Self {
            state: FakeGraphicsContextState::new(width, height),
        }
    }
}

impl Default for FakeGraphicsContext {
    fn default() -> Self {
        Self::new()
    }
}

impl IGraphicsContext for FakeGraphicsContext {
    fn graphics_backend(&self) -> crate::native::traits::present::GraphicsBackend {
        crate::native::traits::present::GraphicsBackend::OpenGlEs
    }

    fn initialize(&mut self, _native_window: *mut std::ffi::c_void, w: i32, h: i32) -> Result<()> {
        self.state.width.set(w);
        self.state.height.set(h);
        self.state.initialized = true;
        Ok(())
    }

    fn resize(&mut self, w: i32, h: i32) {
        self.state.width.set(w);
        self.state.height.set(h);
    }

    fn make_current(&mut self) {
        self.state.make_current_calls += 1;
    }

    fn swap_buffers(&mut self, damage: PresentDamage) {
        self.state.swap_buffers_calls += 1;
        self.state.last_swap_damage = Some(damage);
    }

    fn shutdown(&mut self) {
        self.state.shutdown_called = true;
    }

    fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Vec<u32> {
        Vec::new()
    }

    fn width(&self) -> i32 {
        self.state.width.get()
    }

    fn height(&self) -> i32 {
        self.state.height.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_graphics_context_records_swap_damage() {
        let mut context = FakeGraphicsContext::new();
        let damage = PresentDamage::Partial(vec![(1, 2, 3, 4)]);

        context.swap_buffers(damage.clone());

        assert_eq!(context.state.swap_buffers_calls, 1);
        assert_eq!(context.state.last_swap_damage, Some(damage));
    }
}

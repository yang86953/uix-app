//! Fake 图形上下文 — 空操作实现，记录调用。

use std::cell::Cell;
use crate::api::traits::IGraphicsContext;
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct FakeGraphicsContextState {
    pub width: Cell<i32>,
    pub height: Cell<i32>,
    pub initialized: bool,
    pub make_current_calls: usize,
    pub swap_buffers_calls: usize,
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
        Self { state: FakeGraphicsContextState::new(0, 0) }
    }

    pub fn with_size(width: i32, height: i32) -> Self {
        Self { state: FakeGraphicsContextState::new(width, height) }
    }
}

impl IGraphicsContext for FakeGraphicsContext {
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

    fn swap_buffers(&mut self) {
        self.state.swap_buffers_calls += 1;
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

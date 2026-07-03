//! Fake 呈现器 — 记录每次像素输出。

use crate::api::traits::IPresenter;
use crate::error::Result;
use crate::PresentDamage;

#[derive(Debug, Clone)]
pub struct PresentCall {
    pub width: i32,
    pub height: i32,
    pub pixels_len: usize,
    pub damage: PresentDamage,
}

#[derive(Debug, Clone)]
pub struct FakePresenterState {
    /// `present` 调用历史
    pub present_calls: Vec<PresentCall>,
    /// `resize` 调用历史
    pub resize_calls: Vec<(i32, i32)>,
    /// 最近一次呈现的像素（拷贝）
    pub last_pixels: Vec<u32>,
    /// 当前宽高
    pub width: i32,
    pub height: i32,
}

impl Default for FakePresenterState {
    fn default() -> Self {
        Self {
            present_calls: Vec::new(),
            resize_calls: Vec::new(),
            last_pixels: Vec::new(),
            width: 0,
            height: 0,
        }
    }
}

#[derive(Debug)]
pub struct FakePresenter {
    pub state: FakePresenterState,
}

impl FakePresenter {
    pub fn new() -> Self {
        Self {
            state: FakePresenterState::default(),
        }
    }

    pub fn present_count(&self) -> usize {
        self.state.present_calls.len()
    }

    pub fn resize_count(&self) -> usize {
        self.state.resize_calls.len()
    }

    pub fn clear_history(&mut self) {
        self.state.present_calls.clear();
        self.state.resize_calls.clear();
        self.state.last_pixels.clear();
    }
}

impl IPresenter for FakePresenter {
    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        damage: PresentDamage,
    ) -> Result<()> {
        self.state.last_pixels = pixels.to_vec();
        self.state.present_calls.push(PresentCall {
            width,
            height,
            pixels_len: pixels.len(),
            damage,
        });
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        self.state.width = width;
        self.state.height = height;
        self.state.resize_calls.push((width, height));
        Ok(())
    }
}

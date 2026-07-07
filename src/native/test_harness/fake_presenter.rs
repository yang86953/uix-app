//! Fake 呈现器 — 记录每次像素输出。

use crate::core::error::Result;
use crate::native::traits::present::IPresenter;
use crate::native::traits::present::PresentDamage;

#[derive(Debug, Clone)]
pub struct PresentCall {
    pub width: i32,
    pub height: i32,
    pub pixels_len: usize,
    pub damage: PresentDamage,
}

#[derive(Debug, Clone, Default)]
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

impl Default for FakePresenter {
    fn default() -> Self {
        Self::new()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot_hash(pixels: &[u32]) -> u64 {
        pixels.iter().fold(0xcbf29ce484222325, |hash, pixel| {
            (hash ^ u64::from(*pixel)).wrapping_mul(0x100000001b3)
        })
    }

    #[test]
    fn fake_presenter_records_framebuffer_and_damage_snapshot() {
        let mut presenter = FakePresenter::new();
        let pixels = [0xff000000, 0xffff0000, 0xff00ff00, 0xff0000ff];

        presenter
            .present(&pixels, 2, 2, PresentDamage::single(1, 0, 1, 2))
            .expect("fake present should succeed");

        assert_eq!(presenter.present_count(), 1);
        assert_eq!(presenter.state.last_pixels, pixels);
        assert_eq!(
            snapshot_hash(&presenter.state.last_pixels),
            0x03e6c7ef4e14a058
        );
        assert_eq!(presenter.state.present_calls[0].width, 2);
        assert_eq!(presenter.state.present_calls[0].height, 2);
        assert_eq!(presenter.state.present_calls[0].pixels_len, 4);
        assert_eq!(
            presenter.state.present_calls[0].damage,
            PresentDamage::Partial(vec![(1, 0, 1, 2)])
        );
    }
}

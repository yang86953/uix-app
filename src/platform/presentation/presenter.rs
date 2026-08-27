//! 平台中立的 CPU 像素呈现契约。

use crate::core::error::{Error, Result};
use crate::core::{PresentCoherency, PresentDamage, PresentImage, PresentSurface};

/// CPU 像素呈现器。
pub(crate) trait IPresenter {
    /// 返回允许窄 damage 的像素保留证明。
    fn present_coherency(&self) -> PresentCoherency {
        PresentCoherency::FullOnly
    }

    /// 返回当前目标元数据；同尺寸重建目标时，实现必须推进 generation。
    fn present_surface(
        &self,
        drawable_width: i32,
        drawable_height: i32,
        device_pixel_ratio: f32,
    ) -> PresentSurface {
        PresentSurface::identity(drawable_width, drawable_height, device_pixel_ratio, 0)
    }

    /// 返回 [`PresentCoherency::TrackedSwapchain`] 当前取得的图像身份。
    fn present_image(&self) -> Option<PresentImage> {
        None
    }

    /// 把 CPU 像素及其 damage 提交给当前呈现目标。
    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        damage: PresentDamage,
    ) -> Result<(), Error>;

    /// 调整 CPU 像素呈现目标尺寸。
    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error>;
}

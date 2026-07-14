//! Preset theme definitions for DesignTokens.
//!
//! Provides `antd_light()` and `antd_dark()` factory functions that return
//! fully-populated `DesignTokens` instances by deriving from ThemePrimitives.

use super::primitives::ThemePrimitives;
use super::DesignTokens;
use crate::draw::Color;
use crate::ui::theme::{PrimaryHue, PRIMARY_HUE_COUNT};

pub const PRIMARY_COUNT: usize = PRIMARY_HUE_COUNT;
pub const PRIMARY_BLUE_INDEX: usize = PrimaryHue::Blue.index();

impl DesignTokens {
    /// Ant Design 5 亮色主题预设。
    pub fn antd_light() -> Self {
        ThemePrimitives::antd_light().into_design_tokens(false)
    }

    /// Ant Design 5 暗色主题预设。
    pub fn antd_dark() -> Self {
        ThemePrimitives::antd_dark().into_design_tokens(true)
    }

    /// 从自定义基色生成主题。
    pub fn from_primitives(primitives: ThemePrimitives, is_dark: bool) -> Self {
        primitives.into_design_tokens(is_dark)
    }

    /// Replace the default brand primary seed while preserving the mode.
    pub fn with_brand_primary(self, primary: Color) -> Self {
        let mut primitives = if self.is_dark {
            ThemePrimitives::antd_dark()
        } else {
            ThemePrimitives::antd_light()
        };
        primitives.primary = primary;
        primitives.info = primary;
        primitives.into_design_tokens(self.is_dark)
    }

    /// 从 12 个 Ant Design 主色种子构造主题。
    ///
    /// 当前令牌面只有一个活动品牌主色，因此仅 Blue 种子进入 `DesignTokens`；
    /// 其余色相由 `PrimaryHue` / `ColorScale` 独立提供。
    pub fn from_primaries(primaries: [Color; PRIMARY_COUNT], is_dark: bool) -> Self {
        let mut primitives = if is_dark {
            ThemePrimitives::antd_dark()
        } else {
            ThemePrimitives::antd_light()
        };
        let primary = primaries[PRIMARY_BLUE_INDEX];
        primitives.primary = primary;
        primitives.info = primary;
        primitives.into_design_tokens(is_dark)
    }
}

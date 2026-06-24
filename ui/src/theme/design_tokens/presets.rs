//! Preset theme definitions for DesignTokens.
//!
//! Provides `antd_light()` and `antd_dark()` factory functions that return
//! fully-populated `DesignTokens` instances by deriving from ThemePrimitives.

use super::primitives::ThemePrimitives;
use super::DesignTokens;

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
}

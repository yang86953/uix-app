//! StyleVariant — 按交互状态区分的样式集合。

use super::Style;

/// 携带交互状态的样式集合——让 widget 根据 normal / hover / active / disabled
/// 自动选择对应的视觉颜色。
///
/// 使用方式：widget 在 `render` 中根据自身状态（hovered/pressed/disabled）
/// 从 variant 中取色，回退到 Style 基础色。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StyleVariant {
    /// 正常状态基础样式
    pub normal: Style,
    /// 悬停样式覆盖（None = 使用 normal）
    pub hover: Option<Style>,
    /// 按下样式覆盖
    pub active: Option<Style>,
    /// 禁用样式覆盖
    pub disabled: Option<Style>,
}

impl StyleVariant {
    pub fn new(base: Style) -> Self {
        Self {
            normal: base,
            ..Self::default()
        }
    }

    /// 链式设置悬停样式。
    pub fn hover(mut self, s: Style) -> Self {
        self.hover = Some(s);
        self
    }
    /// 链式设置按下样式。
    pub fn active(mut self, s: Style) -> Self {
        self.active = Some(s);
        self
    }
    /// 链式设置禁用样式。
    pub fn disabled(mut self, s: Style) -> Self {
        self.disabled = Some(s);
        self
    }

    /// 根据 widget 状态获取当前有效的 Style。
    pub fn resolve(&self, hovered: bool, pressed: bool, disabled: bool) -> &Style {
        if disabled {
            self.disabled.as_ref().unwrap_or(&self.normal)
        } else if pressed {
            self.active.as_ref().unwrap_or(&self.normal)
        } else if hovered {
            self.hover.as_ref().unwrap_or(&self.normal)
        } else {
            &self.normal
        }
    }
}

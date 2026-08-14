//! StyleSet — normal/hover/pressed/focused/disabled 五态样式集合。

use super::Style;
use super::{ColorValue, PaletteColor, TypographyToken};
use crate::core::EdgeInsets;
use crate::ui::theme::NeutralRole;

/// 组件交互态，用于从 `StyleSet` 中解析最终样式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StyleState {
    /// 指示指针当前是否悬停在组件上。
    pub hovered: bool,
    /// 指示组件当前是否处于按压状态。
    pub pressed: bool,
    /// 指示组件当前是否拥有输入焦点。
    pub focused: bool,
    /// 指示组件当前是否不可交互。
    pub disabled: bool,
}

/// 五态样式集合。
///
/// 状态层只保存差异字段，解析时继承 `normal`。优先级：
/// disabled > focused > pressed > hover > normal。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StyleSet {
    /// 未命中任何交互态时使用的基础样式。
    pub normal: Style,
    /// 悬停态叠加到基础样式上的差异样式。
    pub hover: Option<Style>,
    /// 按压态叠加到基础样式上的差异样式。
    pub pressed: Option<Style>,
    /// 焦点态叠加到基础样式上的差异样式。
    pub focused: Option<Style>,
    /// 禁用态叠加到基础样式上的差异样式。
    pub disabled: Option<Style>,
}

impl StyleSet {
    /// 使用给定的基础样式创建不含交互态覆盖的样式集合。
    pub fn new(base: Style) -> Self {
        Self {
            normal: base,
            ..Self::default()
        }
    }

    /// 设置悬停态的差异样式。
    pub fn hover(mut self, s: Style) -> Self {
        self.hover = Some(s);
        self
    }

    /// 设置按压态的差异样式。
    pub fn pressed(mut self, s: Style) -> Self {
        self.pressed = Some(s);
        self
    }

    /// 设置焦点态的差异样式。
    pub fn focused(mut self, s: Style) -> Self {
        self.focused = Some(s);
        self
    }

    /// 设置禁用态的差异样式。
    pub fn disabled(mut self, s: Style) -> Self {
        self.disabled = Some(s);
        self
    }

    /// 将活动态作为按压态的别名进行设置。
    pub fn active(self, s: Style) -> Self {
        self.pressed(s)
    }

    /// 按禁用、焦点、按压、悬停的优先级解析最终样式。
    ///
    /// 命中的状态样式会通过 [`Style::apply`] 叠加到基础样式上。
    pub fn resolve(&self, state: StyleState) -> Style {
        let mut style = self.normal.clone();
        let overlay = if state.disabled {
            self.disabled.as_ref()
        } else if state.focused {
            self.focused.as_ref()
        } else if state.pressed {
            self.pressed.as_ref()
        } else if state.hovered {
            self.hover.as_ref()
        } else {
            None
        };
        if let Some(overlay) = overlay {
            style = style.apply(overlay.clone());
        }
        style
    }

    /// 根据各交互态标志解析最终样式。
    ///
    /// 当多个标志同时为真时，采用与 [`Self::resolve`] 相同的优先级。
    pub fn resolve_flags(
        &self,
        hovered: bool,
        pressed: bool,
        focused: bool,
        disabled: bool,
    ) -> Style {
        self.resolve(StyleState {
            hovered,
            pressed,
            focused,
            disabled,
        })
    }

    /// 创建默认按钮的五态样式集合。
    pub fn button_default() -> Self {
        // focused/pressed 不改 fill/border/text（#176）：点击反馈仅 Material ripple。
        let normal = Style::button_default();
        Self::new(normal.clone())
            .hover(Style {
                background: Some(ColorValue::Palette(PaletteColor::PrimaryBg)),
                border_color: Some(ColorValue::Palette(PaletteColor::Primary)),
                color: ColorValue::Palette(PaletteColor::Primary),
                ..Style::default()
            })
            .disabled(Style {
                border_color: Some(ColorValue::Neutral(NeutralRole::Border)),
                color: ColorValue::Neutral(NeutralRole::TextQuaternary),
                opacity: 0.45,
                ..Style::default()
            })
    }

    /// 创建主按钮的五态样式集合。
    pub fn button_primary() -> Self {
        let normal = Style::button_primary();
        Self::new(normal.clone())
            .hover(Style {
                background: Some(ColorValue::Palette(PaletteColor::PrimaryHover)),
                border_color: Some(ColorValue::Palette(PaletteColor::PrimaryHover)),
                color: ColorValue::Palette(PaletteColor::White),
                ..Style::default()
            })
            .disabled(Style {
                background: Some(ColorValue::Neutral(NeutralRole::FillTertiary)),
                border_color: Some(ColorValue::Neutral(NeutralRole::Border)),
                color: ColorValue::Neutral(NeutralRole::TextQuaternary),
                opacity: 0.45,
                ..Style::default()
            })
    }

    /// 创建幽灵按钮的五态样式集合。
    pub fn button_ghost() -> Self {
        Self::new(Style {
            background: None,
            border_color: Some(ColorValue::Palette(PaletteColor::Primary)),
            border_width: EdgeInsets::uniform(1.0),
            border_radius: 6.0,
            padding: EdgeInsets::new(15.0, 0.0, 15.0, 0.0),
            color: ColorValue::Palette(PaletteColor::Primary),
            font_size: TypographyToken::Body,
            ..Style::default()
        })
        .hover(Style {
            background: Some(ColorValue::Palette(PaletteColor::PrimaryBg)),
            ..Style::default()
        })
        .disabled(Style {
            border_color: Some(ColorValue::Neutral(NeutralRole::Border)),
            color: ColorValue::Neutral(NeutralRole::TextQuaternary),
            opacity: 0.45,
            ..Style::default()
        })
    }

    /// 创建危险操作按钮的五态样式集合。
    pub fn button_danger() -> Self {
        Self::new(Style {
            background: Some(ColorValue::Palette(PaletteColor::Error)),
            border_color: Some(ColorValue::Palette(PaletteColor::Error)),
            border_width: EdgeInsets::uniform(1.0),
            border_radius: 6.0,
            padding: EdgeInsets::new(15.0, 0.0, 15.0, 0.0),
            color: ColorValue::Palette(PaletteColor::White),
            font_size: TypographyToken::Body,
            ..Style::default()
        })
        .hover(Style {
            background: Some(ColorValue::Palette(PaletteColor::ErrorBorder)),
            border_color: Some(ColorValue::Palette(PaletteColor::ErrorBorder)),
            ..Style::default()
        })
        .disabled(Style {
            background: Some(ColorValue::Neutral(NeutralRole::FillTertiary)),
            border_color: Some(ColorValue::Neutral(NeutralRole::Border)),
            color: ColorValue::Neutral(NeutralRole::TextQuaternary),
            opacity: 0.45,
            ..Style::default()
        })
    }
}

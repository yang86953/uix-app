//! StyleSet — normal/hover/pressed/focused/disabled 五态样式集合。

use super::Style;
use super::{ColorValue, PaletteColor, TypographyToken};
use crate::core::EdgeInsets;
use crate::draw::Color;
use crate::ui::theme::NeutralRole;

/// 组件交互态，用于从 `StyleSet` 中解析最终样式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StyleState {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub disabled: bool,
}

/// 五态样式集合。
///
/// 状态层只保存差异字段，解析时继承 `normal`。优先级：
/// disabled > focused > pressed > hover > normal。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StyleSet {
    pub normal: Style,
    pub hover: Option<Style>,
    pub pressed: Option<Style>,
    pub focused: Option<Style>,
    pub disabled: Option<Style>,
}

impl StyleSet {
    pub fn new(base: Style) -> Self {
        Self {
            normal: base,
            ..Self::default()
        }
    }

    pub fn hover(mut self, s: Style) -> Self {
        self.hover = Some(s);
        self
    }

    pub fn pressed(mut self, s: Style) -> Self {
        self.pressed = Some(s);
        self
    }

    pub fn focused(mut self, s: Style) -> Self {
        self.focused = Some(s);
        self
    }

    pub fn disabled(mut self, s: Style) -> Self {
        self.disabled = Some(s);
        self
    }

    pub fn active(self, s: Style) -> Self {
        self.pressed(s)
    }

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

    pub fn button_default() -> Self {
        let normal = Style::button_default();
        Self::new(normal.clone())
            .hover(Style {
                border_color: Some(ColorValue::Palette(PaletteColor::Primary)),
                color: ColorValue::Palette(PaletteColor::Primary),
                ..Style::default()
            })
            .pressed(Style {
                border_color: Some(ColorValue::Palette(PaletteColor::PrimaryActive)),
                color: ColorValue::Palette(PaletteColor::PrimaryActive),
                ..Style::default()
            })
            .focused(Style {
                border_color: Some(ColorValue::Palette(PaletteColor::Primary)),
                box_shadow: Some(super::BoxShadowDef::new(
                    Color::from_rgba(22, 119, 255, 80),
                    4.0,
                    0.0,
                    0.0,
                )),
                ..Style::default()
            })
            .disabled(Style {
                border_color: Some(ColorValue::Neutral(NeutralRole::Border)),
                color: ColorValue::Neutral(NeutralRole::TextQuaternary),
                opacity: 0.45,
                ..Style::default()
            })
    }

    pub fn button_primary() -> Self {
        let normal = Style::button_primary();
        Self::new(normal.clone())
            .hover(Style {
                background: Some(ColorValue::Palette(PaletteColor::PrimaryHover)),
                border_color: Some(ColorValue::Palette(PaletteColor::PrimaryHover)),
                color: ColorValue::Palette(PaletteColor::White),
                ..Style::default()
            })
            .pressed(Style {
                background: Some(ColorValue::Palette(PaletteColor::PrimaryActive)),
                border_color: Some(ColorValue::Palette(PaletteColor::PrimaryActive)),
                color: ColorValue::Palette(PaletteColor::White),
                ..Style::default()
            })
            .focused(Style {
                box_shadow: Some(super::BoxShadowDef::new(
                    Color::from_rgba(22, 119, 255, 90),
                    4.0,
                    0.0,
                    0.0,
                )),
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
        .pressed(Style {
            border_color: Some(ColorValue::Palette(PaletteColor::PrimaryActive)),
            color: ColorValue::Palette(PaletteColor::PrimaryActive),
            ..Style::default()
        })
    }

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
        .pressed(Style {
            background: Some(ColorValue::Palette(PaletteColor::Error)),
            border_color: Some(ColorValue::Palette(PaletteColor::Error)),
            ..Style::default()
        })
    }
}

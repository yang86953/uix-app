//! 主题与设计令牌契约。

use crate::core::EdgeInsets;
use crate::draw::painting::ThemeTokens;
use crate::ui::style::{ColorValue, PaletteColor, Style, TypographyToken};
use crate::ui::theme::NeutralRole;

/// 抽象设计令牌提供者 — 聚合 ThemeTokens 与 UI 域 style 助手。
pub trait TokenProvider: ThemeTokens + Send + Sync {
    fn style_container(&self) -> Style {
        Style {
            background: None,
            border_color: None,
            border_width: EdgeInsets::zero(),
            border_radius: self.border_radius(),
            color: ColorValue::Neutral(NeutralRole::Text),
            font_size: TypographyToken::Body,
            ..Style::default()
        }
    }

    fn style_label(&self) -> Style {
        Style {
            color: ColorValue::Neutral(NeutralRole::Text),
            font_size: TypographyToken::Body,
            padding: EdgeInsets::new(2.0, 0.0, 0.0, 0.0),
            ..Style::default()
        }
    }

    fn style_button_default(&self) -> Style {
        Style {
            background: None,
            border_color: Some(ColorValue::Neutral(NeutralRole::Border)),
            border_width: EdgeInsets::uniform(1.0),
            border_radius: self.border_radius(),
            padding: EdgeInsets::new(15.0, 0.0, 15.0, 0.0),
            color: ColorValue::Neutral(NeutralRole::Text),
            font_size: TypographyToken::Body,
            ..Style::default()
        }
    }

    fn style_button_primary(&self) -> Style {
        Style {
            background: Some(ColorValue::Palette(PaletteColor::Primary)),
            border_color: Some(ColorValue::Palette(PaletteColor::Primary)),
            border_width: EdgeInsets::uniform(1.0),
            border_radius: self.border_radius(),
            padding: EdgeInsets::new(15.0, 0.0, 15.0, 0.0),
            color: ColorValue::Palette(PaletteColor::White),
            font_size: TypographyToken::Body,
            ..Style::default()
        }
    }
}

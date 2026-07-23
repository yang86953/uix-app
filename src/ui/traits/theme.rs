//! 主题与设计令牌契约。

use crate::core::EdgeInsets;
use crate::ui::style::{ColorValue, PaletteColor, Style, TypographyToken};
use crate::ui::theme::{IColorTokens, NeutralRole, ShadowToken};

/// 排版设计令牌。
pub trait ITypographyTokens: Send + Sync {
    fn font_family(&self) -> &str;
    fn font_size_sm(&self) -> f32 {
        12.0
    }
    fn font_size(&self) -> f32 {
        14.0
    }
    fn font_size_lg(&self) -> f32 {
        16.0
    }
    fn font_size_xl(&self) -> f32 {
        20.0
    }
    fn font_size_heading_1(&self) -> f32 {
        38.0
    }
    fn font_size_heading_2(&self) -> f32 {
        30.0
    }
    fn font_size_heading_3(&self) -> f32 {
        24.0
    }
    fn font_size_heading_4(&self) -> f32 {
        20.0
    }
    fn font_size_heading_5(&self) -> f32 {
        16.0
    }
    fn font_weight_regular(&self) -> f32 {
        400.0
    }
    fn font_weight_medium(&self) -> f32 {
        500.0
    }
    fn font_weight_semibold(&self) -> f32 {
        600.0
    }
    fn font_weight_bold(&self) -> f32 {
        700.0
    }
    fn line_height(&self) -> f32 {
        1.5715
    }
}

/// 间距与尺寸设计令牌。
pub trait ISpacingTokens: Send + Sync {
    fn padding_xss(&self) -> f32 {
        4.0
    }
    fn padding_xs(&self) -> f32 {
        8.0
    }
    fn padding_sm(&self) -> f32 {
        12.0
    }
    fn padding(&self) -> f32 {
        16.0
    }
    fn padding_md(&self) -> f32 {
        20.0
    }
    fn padding_lg(&self) -> f32 {
        24.0
    }
    fn padding_xl(&self) -> f32 {
        32.0
    }
    fn border_radius(&self) -> f32 {
        6.0
    }
    fn border_radius_sm(&self) -> f32 {
        4.0
    }
    fn border_radius_lg(&self) -> f32 {
        8.0
    }
    fn border_radius_xl(&self) -> f32 {
        12.0
    }
    fn border_radius_round(&self) -> f32 {
        999.0
    }
    fn control_height_sm(&self) -> f32 {
        24.0
    }
    fn control_height(&self) -> f32 {
        32.0
    }
    fn control_height_lg(&self) -> f32 {
        40.0
    }
    fn motion_duration_fast(&self) -> f32 {
        0.1
    }
    fn motion_duration_mid(&self) -> f32 {
        0.2
    }
    fn motion_duration_slow(&self) -> f32 {
        0.3
    }
    fn motion_easing_default(&self) -> &str {
        "cubic-bezier(0.25, 0.1, 0.25, 1)"
    }
    fn motion_easing_in(&self) -> &str {
        "cubic-bezier(0.42, 0, 1, 1)"
    }
    fn motion_easing_out(&self) -> &str {
        "cubic-bezier(0, 0, 0.58, 1)"
    }
    fn motion_easing_in_out(&self) -> &str {
        "cubic-bezier(0.42, 0, 0.58, 1)"
    }
    fn screen_xs(&self) -> f32 {
        480.0
    }
    fn screen_sm(&self) -> f32 {
        576.0
    }
    fn screen_md(&self) -> f32 {
        768.0
    }
    fn screen_lg(&self) -> f32 {
        992.0
    }
    fn screen_xl(&self) -> f32 {
        1200.0
    }
    fn screen_xxl(&self) -> f32 {
        1600.0
    }
}

/// 结构化多层阴影令牌。
pub trait IBoxShadowTokens: Send + Sync {
    fn box_shadow(&self) -> ShadowToken;
    fn box_shadow_secondary(&self) -> ShadowToken;
}

/// UI 主题设计令牌聚合。
pub trait ThemeTokens:
    IColorTokens + ITypographyTokens + ISpacingTokens + IBoxShadowTokens + Send + Sync
{
    fn is_dark(&self) -> bool {
        false
    }
}

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

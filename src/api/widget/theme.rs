//! # 主题与设计令牌契约
//!
//! 设计令牌 trait 聚合与具体主题类型 re-export。

use crate::widget::style::Style;
use crate::platform::EdgeInsets;

pub use crate::render::painting::{
    IBoxShadowTokens, IColorTokens, ISpacingTokens, ITypographyTokens, ShadowToken, ThemeTokens,
};

/// 抽象设计令牌提供者 — 聚合 ThemeTokens 与 UI 域 style 助手。
pub trait TokenProvider: ThemeTokens + Send + Sync {
    fn style_container(&self) -> Style {
        Style {
            background: None,
            border_color: None,
            border_width: 0.0,
            border_radius: self.border_radius(),
            color: self.color_text(),
            font_size: self.font_size(),
            ..Style::default()
        }
    }

    fn style_label(&self) -> Style {
        Style {
            color: self.color_text(),
            font_size: self.font_size(),
            padding: EdgeInsets::new(2.0, 0.0, 0.0, 0.0),
            ..Style::default()
        }
    }

    fn style_button_default(&self) -> Style {
        Style {
            background: None,
            border_color: Some(self.color_border()),
            border_width: 1.0,
            border_radius: self.border_radius(),
            padding: EdgeInsets::new(15.0, 0.0, 15.0, 0.0),
            color: self.color_text(),
            font_size: self.font_size(),
            ..Style::default()
        }
    }

    fn style_button_primary(&self) -> Style {
        Style {
            background: Some(self.color_primary()),
            border_color: Some(self.color_primary()),
            border_width: 1.0,
            border_radius: self.border_radius(),
            padding: EdgeInsets::new(15.0, 0.0, 15.0, 0.0),
            color: self.color_white(),
            font_size: self.font_size(),
            ..Style::default()
        }
    }
}

// ── 主题实现 re-export ──
pub use crate::widget::theme::{DesignTokens, Theme};

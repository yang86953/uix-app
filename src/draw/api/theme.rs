//! 主题快照 — 每帧注入绘图层，与 UI 主题域解耦。

use crate::draw::Color;
use crate::ui::theme::{IColorTokens, ShadowToken};
use crate::ui::traits::{IBoxShadowTokens, ISpacingTokens, ITypographyTokens, ThemeTokens};

/// 每帧注入的主题快照。
pub struct ThemeSnapshot<'a> {
    tokens: &'a dyn ThemeTokens,
}

impl<'a> ThemeSnapshot<'a> {
    pub fn new(tokens: &'a dyn ThemeTokens) -> Self {
        Self { tokens }
    }

    pub fn tokens(&self) -> &dyn ThemeTokens {
        self.tokens
    }
}

impl ThemeTokens for ThemeSnapshot<'_> {
    fn is_dark(&self) -> bool {
        self.tokens.is_dark()
    }
}

macro_rules! delegate_theme_tokens {
    ($($method:ident $( ( $( $arg:ident : $ty:ty ),* ) )? -> $ret:ty;)*) => {
        $(
            fn $method(&self $(, $($arg: $ty),*)*) -> $ret {
                self.tokens.$method($($($arg),*)*)
            }
        )*
    };
}

impl IColorTokens for ThemeSnapshot<'_> {
    delegate_theme_tokens! {
        color_primary() -> Color;
        color_primary_hover() -> Color;
        color_primary_active() -> Color;
        color_primary_bg() -> Color;
        color_primary_border() -> Color;
        color_bg_container() -> Color;
        color_bg_elevated() -> Color;
        color_bg_raised() -> Color;
        color_bg_overlay() -> Color;
        color_bg_layout() -> Color;
        color_bg_spotlight() -> Color;
        color_bg_mask() -> Color;
        color_border() -> Color;
        color_border_secondary() -> Color;
        color_fill() -> Color;
        color_fill_secondary() -> Color;
        color_fill_tertiary() -> Color;
        color_fill_quaternary() -> Color;
        color_text() -> Color;
        color_text_secondary() -> Color;
        color_text_tertiary() -> Color;
        color_text_quaternary() -> Color;
        color_white() -> Color;
        color_black() -> Color;
        color_shadow() -> Color;
        color_shadow_secondary() -> Color;
        color_success() -> Color;
        color_success_bg() -> Color;
        color_success_border() -> Color;
        color_warning() -> Color;
        color_warning_bg() -> Color;
        color_warning_border() -> Color;
        color_error() -> Color;
        color_error_bg() -> Color;
        color_error_border() -> Color;
        color_info() -> Color;
        color_info_bg() -> Color;
        color_info_border() -> Color;
        color_link() -> Color;
        color_link_hover() -> Color;
        color_link_active() -> Color;
    }
}

impl ITypographyTokens for ThemeSnapshot<'_> {
    delegate_theme_tokens! {
        font_family() -> &str;
        font_size_sm() -> f32;
        font_size() -> f32;
        font_size_lg() -> f32;
        font_size_xl() -> f32;
        font_size_heading_1() -> f32;
        font_size_heading_2() -> f32;
        font_size_heading_3() -> f32;
        font_size_heading_4() -> f32;
        font_size_heading_5() -> f32;
        font_weight_regular() -> f32;
        font_weight_medium() -> f32;
        font_weight_semibold() -> f32;
        font_weight_bold() -> f32;
        line_height() -> f32;
    }
}

impl ISpacingTokens for ThemeSnapshot<'_> {}

impl IBoxShadowTokens for ThemeSnapshot<'_> {
    delegate_theme_tokens! {
        box_shadow() -> ShadowToken;
        box_shadow_secondary() -> ShadowToken;
    }
}

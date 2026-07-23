//! 主题快照 — 每帧注入绘图层，与 UI 主题域解耦。

use crate::draw::Color;

/// 盒阴影令牌（Ant Design 5 多层阴影）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowToken {
    pub layer_1: (f32, f32, f32, Color),
    pub layer_2: (f32, f32, f32, Color),
    pub layer_3: (f32, f32, f32, Color),
}

impl ShadowToken {
    pub const fn none() -> Self {
        Self {
            layer_1: (0.0, 0.0, 0.0, Color::transparent()),
            layer_2: (0.0, 0.0, 0.0, Color::transparent()),
            layer_3: (0.0, 0.0, 0.0, Color::transparent()),
        }
    }
}

/// 颜色设计令牌。
pub trait IColorTokens: Send + Sync {
    fn color_primary(&self) -> Color;
    fn color_primary_hover(&self) -> Color;
    fn color_primary_active(&self) -> Color;
    fn color_primary_bg(&self) -> Color;
    fn color_primary_border(&self) -> Color;
    fn color_bg_container(&self) -> Color;
    fn color_bg_elevated(&self) -> Color;
    fn color_bg_raised(&self) -> Color;
    fn color_bg_overlay(&self) -> Color;
    fn color_bg_layout(&self) -> Color;
    fn color_bg_spotlight(&self) -> Color;
    fn color_bg_mask(&self) -> Color;
    fn color_border(&self) -> Color;
    fn color_border_secondary(&self) -> Color;
    fn color_fill(&self) -> Color;
    fn color_fill_secondary(&self) -> Color;
    fn color_fill_tertiary(&self) -> Color;
    fn color_fill_quaternary(&self) -> Color;
    fn color_text(&self) -> Color;
    fn color_text_secondary(&self) -> Color;
    fn color_text_tertiary(&self) -> Color;
    fn color_text_quaternary(&self) -> Color;
    fn color_white(&self) -> Color;
    fn color_black(&self) -> Color;
    fn color_shadow(&self) -> Color;
    fn color_shadow_secondary(&self) -> Color;
    fn color_success(&self) -> Color;
    fn color_success_bg(&self) -> Color;
    fn color_success_border(&self) -> Color;
    fn color_warning(&self) -> Color;
    fn color_warning_bg(&self) -> Color;
    fn color_warning_border(&self) -> Color;
    fn color_error(&self) -> Color;
    fn color_error_bg(&self) -> Color;
    fn color_error_border(&self) -> Color;
    fn color_info(&self) -> Color;
    fn color_info_bg(&self) -> Color;
    fn color_info_border(&self) -> Color;
    fn color_link(&self) -> Color;
    fn color_link_hover(&self) -> Color;
    fn color_link_active(&self) -> Color;
}

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

/// 绘图层主题令牌聚合 — UI 层 `TokenProvider` 实现此 trait。
pub trait ThemeTokens:
    IColorTokens + ITypographyTokens + ISpacingTokens + IBoxShadowTokens + Send + Sync
{
    fn is_dark(&self) -> bool {
        false
    }
}

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

use std::sync::Arc;

use crate::draw::Color;
use crate::ui::theme::{IColorTokens, ShadowToken};
use crate::ui::traits::{IBoxShadowTokens, ISpacingTokens, ITypographyTokens, ThemeTokens};

/// A typed, partial override for design tokens.
///
/// Unset fields delegate to the active subtree theme. The struct is pure data,
/// so patches can be cloned, compared, and installed in a `ConfigProvider`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TokenPatch {
    pub color_primary: Option<Color>,
    pub color_primary_hover: Option<Color>,
    pub color_primary_active: Option<Color>,
    pub color_primary_bg: Option<Color>,
    pub color_primary_border: Option<Color>,
    pub color_bg_container: Option<Color>,
    pub color_bg_elevated: Option<Color>,
    pub color_bg_raised: Option<Color>,
    pub color_bg_overlay: Option<Color>,
    pub color_bg_layout: Option<Color>,
    pub color_bg_spotlight: Option<Color>,
    pub color_bg_mask: Option<Color>,
    pub color_border: Option<Color>,
    pub color_border_secondary: Option<Color>,
    pub color_fill: Option<Color>,
    pub color_fill_secondary: Option<Color>,
    pub color_fill_tertiary: Option<Color>,
    pub color_fill_quaternary: Option<Color>,
    pub color_text: Option<Color>,
    pub color_text_secondary: Option<Color>,
    pub color_text_tertiary: Option<Color>,
    pub color_text_quaternary: Option<Color>,
    pub color_white: Option<Color>,
    pub color_black: Option<Color>,
    pub color_shadow: Option<Color>,
    pub color_shadow_secondary: Option<Color>,
    pub color_success: Option<Color>,
    pub color_success_bg: Option<Color>,
    pub color_success_border: Option<Color>,
    pub color_warning: Option<Color>,
    pub color_warning_bg: Option<Color>,
    pub color_warning_border: Option<Color>,
    pub color_error: Option<Color>,
    pub color_error_bg: Option<Color>,
    pub color_error_border: Option<Color>,
    pub color_info: Option<Color>,
    pub color_info_bg: Option<Color>,
    pub color_info_border: Option<Color>,
    pub color_link: Option<Color>,
    pub color_link_hover: Option<Color>,
    pub color_link_active: Option<Color>,
    pub font_family: Option<&'static str>,
    pub font_size_sm: Option<f32>,
    pub font_size: Option<f32>,
    pub font_size_lg: Option<f32>,
    pub font_size_xl: Option<f32>,
    pub font_size_heading_1: Option<f32>,
    pub font_size_heading_2: Option<f32>,
    pub font_size_heading_3: Option<f32>,
    pub font_size_heading_4: Option<f32>,
    pub font_size_heading_5: Option<f32>,
    pub font_weight_regular: Option<f32>,
    pub font_weight_medium: Option<f32>,
    pub font_weight_semibold: Option<f32>,
    pub font_weight_bold: Option<f32>,
    pub line_height: Option<f32>,
    pub padding_xss: Option<f32>,
    pub padding_xs: Option<f32>,
    pub padding_sm: Option<f32>,
    pub padding: Option<f32>,
    pub padding_md: Option<f32>,
    pub padding_lg: Option<f32>,
    pub padding_xl: Option<f32>,
    pub border_radius: Option<f32>,
    pub border_radius_sm: Option<f32>,
    pub border_radius_lg: Option<f32>,
    pub border_radius_xl: Option<f32>,
    pub border_radius_round: Option<f32>,
    pub control_height_sm: Option<f32>,
    pub control_height: Option<f32>,
    pub control_height_lg: Option<f32>,
    pub box_shadow: Option<ShadowToken>,
    pub box_shadow_secondary: Option<ShadowToken>,
    pub motion_duration_fast: Option<f32>,
    pub motion_duration_mid: Option<f32>,
    pub motion_duration_slow: Option<f32>,
    pub motion_easing_default: Option<&'static str>,
    pub motion_easing_in: Option<&'static str>,
    pub motion_easing_out: Option<&'static str>,
    pub motion_easing_in_out: Option<&'static str>,
    pub screen_xs: Option<f32>,
    pub screen_sm: Option<f32>,
    pub screen_md: Option<f32>,
    pub screen_lg: Option<f32>,
    pub screen_xl: Option<f32>,
    pub screen_xxl: Option<f32>,
    pub is_dark: Option<bool>,
}

pub(crate) struct ScopedThemeTokens<'a> {
    root: &'a dyn ThemeTokens,
    theme: Option<Arc<dyn ThemeTokens>>,
    patch: Option<Arc<TokenPatch>>,
}

pub(crate) struct TokenScope {
    pub theme: Option<Arc<dyn ThemeTokens>>,
    pub patch: Option<Arc<TokenPatch>>,
}

impl<'a> ScopedThemeTokens<'a> {
    pub fn new(root: &'a dyn ThemeTokens) -> Self {
        Self {
            root,
            theme: None,
            patch: None,
        }
    }

    pub fn replace_scope(
        &mut self,
        theme: Option<Arc<dyn ThemeTokens>>,
        patch: Option<Arc<TokenPatch>>,
    ) -> TokenScope {
        TokenScope {
            theme: std::mem::replace(&mut self.theme, theme),
            patch: std::mem::replace(&mut self.patch, patch),
        }
    }

    pub fn restore_scope(&mut self, scope: TokenScope) {
        self.theme = scope.theme;
        self.patch = scope.patch;
    }

    fn base(&self) -> &dyn ThemeTokens {
        self.theme.as_deref().unwrap_or(self.root)
    }
}

macro_rules! patched_copy {
    ($self:ident, $field:ident) => {
        $self
            .patch
            .as_ref()
            .and_then(|patch| patch.$field)
            .unwrap_or_else(|| $self.base().$field())
    };
}

macro_rules! copy_methods {
    ($($field:ident -> $ty:ty;)*) => {
        $(
            fn $field(&self) -> $ty {
                patched_copy!(self, $field)
            }
        )*
    };
}

impl IColorTokens for ScopedThemeTokens<'_> {
    copy_methods! {
        color_primary -> Color;
        color_primary_hover -> Color;
        color_primary_active -> Color;
        color_primary_bg -> Color;
        color_primary_border -> Color;
        color_bg_container -> Color;
        color_bg_elevated -> Color;
        color_bg_raised -> Color;
        color_bg_overlay -> Color;
        color_bg_layout -> Color;
        color_bg_spotlight -> Color;
        color_bg_mask -> Color;
        color_border -> Color;
        color_border_secondary -> Color;
        color_fill -> Color;
        color_fill_secondary -> Color;
        color_fill_tertiary -> Color;
        color_fill_quaternary -> Color;
        color_text -> Color;
        color_text_secondary -> Color;
        color_text_tertiary -> Color;
        color_text_quaternary -> Color;
        color_white -> Color;
        color_black -> Color;
        color_shadow -> Color;
        color_shadow_secondary -> Color;
        color_success -> Color;
        color_success_bg -> Color;
        color_success_border -> Color;
        color_warning -> Color;
        color_warning_bg -> Color;
        color_warning_border -> Color;
        color_error -> Color;
        color_error_bg -> Color;
        color_error_border -> Color;
        color_info -> Color;
        color_info_bg -> Color;
        color_info_border -> Color;
        color_link -> Color;
        color_link_hover -> Color;
        color_link_active -> Color;
    }
}

impl ITypographyTokens for ScopedThemeTokens<'_> {
    fn font_family(&self) -> &str {
        self.patch
            .as_ref()
            .and_then(|patch| patch.font_family)
            .unwrap_or_else(|| self.base().font_family())
    }

    copy_methods! {
        font_size_sm -> f32;
        font_size -> f32;
        font_size_lg -> f32;
        font_size_xl -> f32;
        font_size_heading_1 -> f32;
        font_size_heading_2 -> f32;
        font_size_heading_3 -> f32;
        font_size_heading_4 -> f32;
        font_size_heading_5 -> f32;
        font_weight_regular -> f32;
        font_weight_medium -> f32;
        font_weight_semibold -> f32;
        font_weight_bold -> f32;
        line_height -> f32;
    }
}

impl ISpacingTokens for ScopedThemeTokens<'_> {
    copy_methods! {
        padding_xss -> f32;
        padding_xs -> f32;
        padding_sm -> f32;
        padding -> f32;
        padding_md -> f32;
        padding_lg -> f32;
        padding_xl -> f32;
        border_radius -> f32;
        border_radius_sm -> f32;
        border_radius_lg -> f32;
        border_radius_xl -> f32;
        border_radius_round -> f32;
        control_height_sm -> f32;
        control_height -> f32;
        control_height_lg -> f32;
        motion_duration_fast -> f32;
        motion_duration_mid -> f32;
        motion_duration_slow -> f32;
        screen_xs -> f32;
        screen_sm -> f32;
        screen_md -> f32;
        screen_lg -> f32;
        screen_xl -> f32;
        screen_xxl -> f32;
    }

    fn motion_easing_default(&self) -> &str {
        self.patch
            .as_ref()
            .and_then(|patch| patch.motion_easing_default)
            .unwrap_or_else(|| self.base().motion_easing_default())
    }

    fn motion_easing_in(&self) -> &str {
        self.patch
            .as_ref()
            .and_then(|patch| patch.motion_easing_in)
            .unwrap_or_else(|| self.base().motion_easing_in())
    }

    fn motion_easing_out(&self) -> &str {
        self.patch
            .as_ref()
            .and_then(|patch| patch.motion_easing_out)
            .unwrap_or_else(|| self.base().motion_easing_out())
    }

    fn motion_easing_in_out(&self) -> &str {
        self.patch
            .as_ref()
            .and_then(|patch| patch.motion_easing_in_out)
            .unwrap_or_else(|| self.base().motion_easing_in_out())
    }
}

impl IBoxShadowTokens for ScopedThemeTokens<'_> {
    copy_methods! {
        box_shadow -> ShadowToken;
        box_shadow_secondary -> ShadowToken;
    }
}

impl ThemeTokens for ScopedThemeTokens<'_> {
    fn is_dark(&self) -> bool {
        patched_copy!(self, is_dark)
    }
}

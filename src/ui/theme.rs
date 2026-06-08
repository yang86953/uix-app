//! Ant Design 5 Design Token System
//!
//! Complete design tokens matching Ant Design 5.x specification.
//! Provides light and dark theme presets with full color, typography,
//! spacing, border radius, and shadow tokens.
//!
//! ## Extensibility
//!
//! The `TokenProvider` trait enables custom theme injection.
//! `DesignTokens` implements `TokenProvider` as the default Ant Design provider.
//! Users implement `TokenProvider` to inject custom brand color systems
//! without modifying framework source.

use crate::graphics::Color;

/// Abstract token provider — enables custom theme injection.
/// Implement this trait to supply custom design tokens to the widget tree.
pub trait TokenProvider: Send + Sync {
    fn color_primary(&self) -> Color;
    fn color_primary_hover(&self) -> Color;
    fn color_primary_active(&self) -> Color;
    fn color_primary_bg(&self) -> Color;
    fn color_primary_border(&self) -> Color;

    fn color_bg_container(&self) -> Color;
    fn color_bg_elevated(&self) -> Color;
    fn color_bg_raised(&self) -> Color;
    fn color_bg_overlay(&self) -> Color;

    fn color_shadow(&self) -> Color;
    fn color_shadow_secondary(&self) -> Color;

    fn color_border(&self) -> Color;
    fn color_border_secondary(&self) -> Color;

    fn color_fill(&self) -> Color;
    fn color_fill_secondary(&self) -> Color;
    fn color_fill_tertiary(&self) -> Color;

    fn color_text(&self) -> Color;
    fn color_text_secondary(&self) -> Color;
    fn color_text_tertiary(&self) -> Color;
    fn color_text_quaternary(&self) -> Color;

    fn color_success(&self) -> Color;
    fn color_warning(&self) -> Color;
    fn color_error(&self) -> Color;
    fn color_info(&self) -> Color;

    fn border_radius(&self) -> f32 { 6.0 }
    fn border_radius_sm(&self) -> f32 { 4.0 }
    fn border_radius_lg(&self) -> f32 { 8.0 }

    fn font_size(&self) -> f32 { 14.0 }
    fn font_size_sm(&self) -> f32 { 12.0 }
    fn font_size_lg(&self) -> f32 { 16.0 }

    fn control_height_sm(&self) -> f32 { 24.0 }
    fn control_height(&self) -> f32 { 32.0 }
    fn control_height_lg(&self) -> f32 { 40.0 }

    fn is_dark(&self) -> bool { false }
}

// ──────────────────────────────────────────────────────────────────────────
// Ant Design 5 Color Palette
// ──────────────────────────────────────────────────────────────────────────

/// Ant Design 5 design tokens.
#[derive(Debug, Clone)]
pub struct DesignTokens {
    // ── Seed / Brand Colors ──
    pub color_primary: Color,
    pub color_primary_hover: Color,
    pub color_primary_active: Color,
    pub color_primary_bg: Color,
    pub color_primary_border: Color,

    // ── Neutral Colors ──
    pub color_bg_container: Color,
    pub color_bg_elevated: Color,
    pub color_bg_raised: Color,
    pub color_bg_overlay: Color,
    pub color_bg_layout: Color,
    pub color_bg_spotlight: Color,
    pub color_bg_mask: Color,

    pub color_border: Color,
    pub color_border_secondary: Color,

    pub color_fill: Color,
    pub color_fill_secondary: Color,
    pub color_fill_tertiary: Color,
    pub color_fill_quaternary: Color,

    pub color_text: Color,
    pub color_text_secondary: Color,
    pub color_text_tertiary: Color,
    pub color_text_quaternary: Color,
    pub color_white: Color,
    pub color_black: Color,

    // ── Shadow ──
    pub color_shadow: Color,
    pub color_shadow_secondary: Color,

    // ── Semantic Colors ──
    pub color_success: Color,
    pub color_success_bg: Color,
    pub color_success_border: Color,

    pub color_warning: Color,
    pub color_warning_bg: Color,
    pub color_warning_border: Color,

    pub color_error: Color,
    pub color_error_bg: Color,
    pub color_error_border: Color,

    pub color_info: Color,
    pub color_info_bg: Color,
    pub color_info_border: Color,

    pub color_link: Color,
    pub color_link_hover: Color,
    pub color_link_active: Color,

    // ── Typography ──
    pub font_family: &'static str,
    pub font_size_sm: f32,
    pub font_size: f32,
    pub font_size_lg: f32,
    pub font_size_xl: f32,
    pub font_size_heading_1: f32,
    pub font_size_heading_2: f32,
    pub font_size_heading_3: f32,
    pub font_size_heading_4: f32,
    pub font_size_heading_5: f32,
    pub font_weight_regular: f32,
    pub font_weight_medium: f32,
    pub font_weight_semibold: f32,
    pub font_weight_bold: f32,
    pub line_height: f32,

    // ── Spacing (4px grid) ──
    pub padding_xss: f32, // 4px
    pub padding_xs: f32,  // 8px
    pub padding_sm: f32,  // 12px
    pub padding: f32,     // 16px
    pub padding_md: f32,  // 20px
    pub padding_lg: f32,  // 24px
    pub padding_xl: f32,  // 32px

    // ── Border Radius ──
    pub border_radius: f32,       // 6px (default)
    pub border_radius_sm: f32,    // 4px
    pub border_radius_lg: f32,    // 8px
    pub border_radius_xl: f32,    // 12px
    pub border_radius_round: f32, // 999px (pill)

    // ── Control Sizes ──
    pub control_height_sm: f32, // 24px
    pub control_height: f32,    // 32px
    pub control_height_lg: f32, // 40px

    // ── Shadow ──
    pub box_shadow: ShadowToken,
    pub box_shadow_secondary: ShadowToken,

    // ── Misc ──
    pub is_dark: bool,
}

/// Box shadow token (layered shadows as in Ant Design 5).
#[derive(Debug, Clone, Copy)]
pub struct ShadowToken {
    pub layer_1: (f32, f32, f32, Color), // offset_x, offset_y, blur, color
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

// ──────────────────────────────────────────────────────────────────────────
// Theme (trait-based, extensible)
// ──────────────────────────────────────────────────────────────────────────

/// Theme wraps a token provider with light/dark mode.
/// Custom themes are injected by providing a `TokenProvider` implementation.
#[derive(Clone)]
pub struct Theme {
    provider: std::sync::Arc<dyn TokenProvider>,
}

impl Theme {
    /// Create a theme from a custom token provider.
    pub fn new(provider: impl TokenProvider + 'static) -> Self {
        Self {
            provider: std::sync::Arc::new(provider),
        }
    }

    /// Ant Design 5 Light theme.
    pub fn antd_light() -> Self {
        Self::new(DesignTokens::antd_light())
    }

    /// Ant Design 5 Dark theme.
    pub fn antd_dark() -> Self {
        Self::new(DesignTokens::antd_dark())
    }

    /// Access the underlying token provider.
    pub fn tokens(&self) -> &dyn TokenProvider {
        &*self.provider
    }

    pub fn is_dark(&self) -> bool {
        self.provider.is_dark()
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::antd_light()
    }
}

impl std::fmt::Debug for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Theme")
            .field("is_dark", &self.is_dark())
            .finish()
    }
}

// ──────────────────────────────────────────────────────────────────────────
// DesignTokens presets
// ──────────────────────────────────────────────────────────────────────────

impl DesignTokens {
    /// Ant Design 5 Light mode tokens.
    pub fn antd_light() -> Self {
        Self {
            // ── Brand ──
            color_primary: Color::from_rgb(22, 119, 255),
            color_primary_hover: Color::from_rgb(64, 150, 255),
            color_primary_active: Color::from_rgb(9, 88, 217),
            color_primary_bg: Color::from_rgb(230, 244, 255),
            color_primary_border: Color::from_rgb(186, 224, 255),

            // ── Neutral ──
            color_bg_container: Color::from_rgb(250, 250, 252),
            color_bg_elevated: Color::from_rgb(255, 255, 255),
            color_bg_raised: Color::from_rgb(245, 245, 247),
            color_bg_overlay: Color::from_rgb(255, 255, 255),
            color_bg_layout: Color::from_rgb(242, 242, 245),
            color_bg_spotlight: Color::from_rgb(0, 0, 0),
            color_bg_mask: Color::from_rgba(0, 0, 0, 115), // 45%

            color_border: Color::from_rgb(228, 228, 231),
            color_border_secondary: Color::from_rgb(240, 240, 242),

            color_fill: Color::from_rgba(0, 0, 0, 31),   // 12%
            color_fill_secondary: Color::from_rgba(0, 0, 0, 15),  // 6%
            color_fill_tertiary: Color::from_rgba(0, 0, 0, 10),   // 4%
            color_fill_quaternary: Color::from_rgba(0, 0, 0, 5),   // 2%,

            color_text: Color::from_rgba(0, 0, 0, 224),  // 88%
            color_text_secondary: Color::from_rgba(0, 0, 0, 153), // 60%
            color_text_tertiary: Color::from_rgba(0, 0, 0, 102),  // 40%
            color_text_quaternary: Color::from_rgba(0, 0, 0, 51),  // 20%,
            color_white: Color::white(),
            color_black: Color::black(),

            // ── Shadow ──
            color_shadow: Color::from_rgba(0, 0, 0, 20),   // 8%
            color_shadow_secondary: Color::from_rgba(0, 0, 0, 10),  // 4%,

            // ── Semantic ──
            color_success: Color::from_rgb(82, 196, 26),
            color_success_bg: Color::from_rgb(246, 255, 237),
            color_success_border: Color::from_rgb(183, 235, 143),

            color_warning: Color::from_rgb(250, 173, 20),
            color_warning_bg: Color::from_rgb(255, 251, 230),
            color_warning_border: Color::from_rgb(255, 229, 143),

            color_error: Color::from_rgb(255, 77, 79),
            color_error_bg: Color::from_rgb(255, 242, 240),
            color_error_border: Color::from_rgb(255, 188, 185),

            color_info: Color::from_rgb(22, 119, 255),
            color_info_bg: Color::from_rgb(230, 244, 255),
            color_info_border: Color::from_rgb(186, 224, 255),

            color_link: Color::from_rgb(22, 119, 255),
            color_link_hover: Color::from_rgb(64, 150, 255),
            color_link_active: Color::from_rgb(9, 88, 217),

            // ── Typography ──
            font_family:
                "-apple-system, BlinkMacSystemFont, Segoe UI, Roboto, Helvetica Neue, Arial",
            font_size_sm: 12.0,
            font_size: 14.0,
            font_size_lg: 16.0,
            font_size_xl: 20.0,
            font_size_heading_1: 38.0,
            font_size_heading_2: 30.0,
            font_size_heading_3: 24.0,
            font_size_heading_4: 20.0,
            font_size_heading_5: 16.0,
            font_weight_regular: 400.0,
            font_weight_medium: 500.0,
            font_weight_semibold: 600.0,
            font_weight_bold: 700.0,
            line_height: 1.5715,

            // ── Spacing ──
            padding_xss: 4.0,
            padding_xs: 8.0,
            padding_sm: 12.0,
            padding: 16.0,
            padding_md: 20.0,
            padding_lg: 24.0,
            padding_xl: 32.0,

            // ── Border Radius ──
            border_radius: 6.0,
            border_radius_sm: 4.0,
            border_radius_lg: 8.0,
            border_radius_xl: 12.0,
            border_radius_round: 999.0,

            // ── Control Sizes ──
            control_height_sm: 24.0,
            control_height: 32.0,
            control_height_lg: 40.0,

            // ── Shadows ──
            box_shadow: ShadowToken {
                layer_1: (0.0, 1.0, 2.0, Color::from_rgba(0, 0, 0, 15)),  // 6%
                layer_2: (0.0, 1.0, 6.0, Color::from_rgba(0, 0, 0, 10)),  // 4%
                layer_3: (0.0, 2.0, 16.0, Color::from_rgba(0, 0, 0, 8)),  // 3%,
            },
            box_shadow_secondary: ShadowToken {
                layer_1: (0.0, 6.0, 16.0, Color::from_rgba(0, 0, 0, 20)),  // 8%
                layer_2: (0.0, 3.0, 6.0, Color::from_rgba(0, 0, 0, 10)),  // 4%
                layer_3: (0.0, 9.0, 28.0, Color::from_rgba(0, 0, 0, 15)),  // 6%,
            },

            is_dark: false,
        }
    }

    /// Ant Design 5 Dark mode tokens.
    pub fn antd_dark() -> Self {
        Self {
            // ── Brand ──
            color_primary: Color::from_rgb(22, 119, 255),
            color_primary_hover: Color::from_rgb(64, 150, 255),
            color_primary_active: Color::from_rgb(9, 88, 217),
            color_primary_bg: Color::from_rgb(17, 33, 58),
            color_primary_border: Color::from_rgb(40, 61, 92),

            // ── Neutral ──
            color_bg_container: Color::from_rgb(30, 30, 30),
            color_bg_elevated: Color::from_rgb(42, 42, 45),
            color_bg_raised: Color::from_rgb(36, 36, 38),
            color_bg_overlay: Color::from_rgb(48, 48, 50),
            color_bg_layout: Color::from_rgb(21, 21, 21),
            color_bg_spotlight: Color::from_rgb(0, 0, 0),
            color_bg_mask: Color::from_rgba(0, 0, 0, 166), // 65%

            color_border: Color::from_rgb(56, 56, 58),
            color_border_secondary: Color::from_rgb(48, 48, 48),

            color_fill: Color::from_rgba(255, 255, 255, 31),  // 12%
            color_fill_secondary: Color::from_rgba(255, 255, 255, 20), // 8%
            color_fill_tertiary: Color::from_rgba(255, 255, 255, 10),  // 4%
            color_fill_quaternary: Color::from_rgba(255, 255, 255, 5),  // 2%

            color_text: Color::from_rgba(255, 255, 255, 224), // 88%
            color_text_secondary: Color::from_rgba(255, 255, 255, 166), // 65%
            color_text_tertiary: Color::from_rgba(255, 255, 255, 115),  // 45%
            color_text_quaternary: Color::from_rgba(255, 255, 255, 64),  // 25%,
            color_white: Color::white(),
            color_black: Color::black(),

            // ── Shadow ──
            color_shadow: Color::from_rgba(0, 0, 0, 140),  // 55%
            color_shadow_secondary: Color::from_rgba(0, 0, 0, 89),  // 35%,

            // ── Semantic ──
            color_success: Color::from_rgb(73, 185, 22),
            color_success_bg: Color::from_rgb(24, 42, 23),
            color_success_border: Color::from_rgb(50, 82, 40),

            color_warning: Color::from_rgb(250, 173, 20),
            color_warning_bg: Color::from_rgb(49, 39, 18),
            color_warning_border: Color::from_rgb(87, 67, 30),

            color_error: Color::from_rgb(255, 77, 79),
            color_error_bg: Color::from_rgb(46, 27, 29),
            color_error_border: Color::from_rgb(82, 45, 47),

            color_info: Color::from_rgb(22, 119, 255),
            color_info_bg: Color::from_rgb(17, 33, 58),
            color_info_border: Color::from_rgb(40, 61, 92),

            color_link: Color::from_rgb(22, 119, 255),
            color_link_hover: Color::from_rgb(64, 150, 255),
            color_link_active: Color::from_rgb(9, 88, 217),

            // ── Typography ──
            font_family:
                "-apple-system, BlinkMacSystemFont, Segoe UI, Roboto, Helvetica Neue, Arial",
            font_size_sm: 12.0,
            font_size: 14.0,
            font_size_lg: 16.0,
            font_size_xl: 20.0,
            font_size_heading_1: 38.0,
            font_size_heading_2: 30.0,
            font_size_heading_3: 24.0,
            font_size_heading_4: 20.0,
            font_size_heading_5: 16.0,
            font_weight_regular: 400.0,
            font_weight_medium: 500.0,
            font_weight_semibold: 600.0,
            font_weight_bold: 700.0,
            line_height: 1.5715,

            // ── Spacing ──
            padding_xss: 4.0,
            padding_xs: 8.0,
            padding_sm: 12.0,
            padding: 16.0,
            padding_md: 20.0,
            padding_lg: 24.0,
            padding_xl: 32.0,

            // ── Border Radius ──
            border_radius: 6.0,
            border_radius_sm: 4.0,
            border_radius_lg: 8.0,
            border_radius_xl: 12.0,
            border_radius_round: 999.0,

            // ── Control Sizes ──
            control_height_sm: 24.0,
            control_height: 32.0,
            control_height_lg: 40.0,

            // ── Shadows (dark mode — brighter overlay) ──
            box_shadow: ShadowToken {
                layer_1: (0.0, 1.0, 2.0, Color::from_rgba(0, 0, 0, 45)),
                layer_2: (0.0, 1.0, 6.0, Color::from_rgba(0, 0, 0, 35)),
                layer_3: (0.0, 2.0, 16.0, Color::from_rgba(0, 0, 0, 25)),
            },
            box_shadow_secondary: ShadowToken {
                layer_1: (0.0, 6.0, 16.0, Color::from_rgba(0, 0, 0, 55)),
                layer_2: (0.0, 3.0, 6.0, Color::from_rgba(0, 0, 0, 35)),
                layer_3: (0.0, 9.0, 28.0, Color::from_rgba(0, 0, 0, 40)),
            },

            is_dark: true,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// TokenProvider impl for DesignTokens
// ──────────────────────────────────────────────────────────────────────────

impl TokenProvider for DesignTokens {
    fn color_primary(&self) -> Color { self.color_primary }
    fn color_primary_hover(&self) -> Color { self.color_primary_hover }
    fn color_primary_active(&self) -> Color { self.color_primary_active }
    fn color_primary_bg(&self) -> Color { self.color_primary_bg }
    fn color_primary_border(&self) -> Color { self.color_primary_border }

    fn color_bg_container(&self) -> Color { self.color_bg_container }
    fn color_bg_elevated(&self) -> Color { self.color_bg_elevated }
    fn color_bg_raised(&self) -> Color { self.color_bg_raised }
    fn color_bg_overlay(&self) -> Color { self.color_bg_overlay }

    fn color_shadow(&self) -> Color { self.color_shadow }
    fn color_shadow_secondary(&self) -> Color { self.color_shadow_secondary }

    fn color_border(&self) -> Color { self.color_border }
    fn color_border_secondary(&self) -> Color { self.color_border_secondary }

    fn color_fill(&self) -> Color { self.color_fill }
    fn color_fill_secondary(&self) -> Color { self.color_fill_secondary }
    fn color_fill_tertiary(&self) -> Color { self.color_fill_tertiary }

    fn color_text(&self) -> Color { self.color_text }
    fn color_text_secondary(&self) -> Color { self.color_text_secondary }
    fn color_text_tertiary(&self) -> Color { self.color_text_tertiary }
    fn color_text_quaternary(&self) -> Color { self.color_text_quaternary }

    fn color_success(&self) -> Color { self.color_success }
    fn color_warning(&self) -> Color { self.color_warning }
    fn color_error(&self) -> Color { self.color_error }
    fn color_info(&self) -> Color { self.color_info }

    fn border_radius(&self) -> f32 { self.border_radius }
    fn border_radius_sm(&self) -> f32 { self.border_radius_sm }
    fn border_radius_lg(&self) -> f32 { self.border_radius_lg }

    fn font_size(&self) -> f32 { self.font_size }
    fn font_size_sm(&self) -> f32 { self.font_size_sm }
    fn font_size_lg(&self) -> f32 { self.font_size_lg }

    fn control_height_sm(&self) -> f32 { self.control_height_sm }
    fn control_height(&self) -> f32 { self.control_height }
    fn control_height_lg(&self) -> f32 { self.control_height_lg }

    fn is_dark(&self) -> bool { self.is_dark }
}

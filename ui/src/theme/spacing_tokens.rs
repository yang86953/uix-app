//! Spacing, motion, and shadow token traits — sizing, border radius,
//! animations, easing curves, breakpoints, and structured box shadows.
//!
//! Domain sub-traits of the Ant Design 5 token system. Combined via
//! `TokenProvider` supertrait in `theme.rs`.

use super::color_tokens::ShadowToken;

// ════════════════════════════════════════════════════════════════════════════
// ISpacingTokens
// ════════════════════════════════════════════════════════════════════════════

/// Spacing & sizing design tokens — padding, border radius, control heights,
/// motion durations, easing curves, and screen breakpoints.
///
/// All methods provide Ant Design 5 light mode defaults.
pub trait ISpacingTokens: Send + Sync {
    // ── Spacing (4 px grid) ──
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

    // ── Border Radius ──
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

    // ── Control Sizes ──
    fn control_height_sm(&self) -> f32 {
        24.0
    }
    fn control_height(&self) -> f32 {
        32.0
    }
    fn control_height_lg(&self) -> f32 {
        40.0
    }

    // ── Motion ──
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

    // ── Screen Breakpoints ──
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

// ════════════════════════════════════════════════════════════════════════════
// IBoxShadowTokens
// ════════════════════════════════════════════════════════════════════════════

/// Structured multi-layer box shadow tokens.
pub trait IBoxShadowTokens: Send + Sync {
    fn box_shadow(&self) -> ShadowToken;
    fn box_shadow_secondary(&self) -> ShadowToken;
}

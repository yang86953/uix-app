//! E-09 主题品牌种子与 `Theme::custom` 契约。
//!
//! 覆盖：品牌色种子声明式切换、PrimaryHue 种子 → 主色、明暗模式判定、
//! 色板自动生成。

use uix::prelude::*;

#[test]
fn theme_custom_from_light_primitives_is_light() {
    let theme = Theme::custom(ThemePrimitives::antd_light().with_brand_primary(PrimaryHue::Purple));
    assert!(!theme.is_dark(), "亮色基色 → 亮色主题");
    assert_eq!(
        theme.tokens().color_primary(),
        PrimaryHue::Purple.primary(),
        "品牌种子进入色板主色"
    );
}

#[test]
fn theme_custom_from_dark_primitives_is_dark() {
    let theme = Theme::custom(ThemePrimitives::antd_dark().with_brand_primary(PrimaryHue::Green));
    assert!(theme.is_dark(), "暗色基色 → 暗色主题");
    assert_eq!(theme.tokens().color_primary(), PrimaryHue::Green.primary());
}

#[test]
fn primary_hue_converts_to_seed_color() {
    let violet: Color = PrimaryHue::Purple.into();
    assert_eq!(violet, PrimaryHue::Purple.primary());
    assert_ne!(violet, PrimaryHue::Blue.primary(), "不同种子不同主色");
}

#[test]
fn with_brand_primary_updates_primary_and_info() {
    let primitives = ThemePrimitives::antd_light().with_brand_primary(PrimaryHue::Cyan);
    assert_eq!(primitives.primary, PrimaryHue::Cyan.primary());
    assert_eq!(primitives.info, PrimaryHue::Cyan.primary());
}

#[test]
fn custom_theme_equivalent_to_from_primitives() {
    let primitives = ThemePrimitives::antd_light().with_brand_primary(PrimaryHue::Purple);
    let custom = Theme::custom(primitives.clone());
    let direct = Theme::new(DesignTokens::from_primitives(primitives, false));
    assert_eq!(custom.is_dark(), direct.is_dark());
    assert_eq!(
        custom.tokens().color_primary(),
        direct.tokens().color_primary(),
        "custom 与显式 from_primitives 结果一致"
    );
}

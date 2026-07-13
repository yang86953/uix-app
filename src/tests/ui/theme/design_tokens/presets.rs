use crate::tests::common::*;
use crate::ui::theme::design_tokens::presets::*;

#[test]
fn with_brand_primary_replaces_primary_seed() {
    let brand = Color::from_rgb(120, 42, 210);
    let tokens = DesignTokens::antd_light().with_brand_primary(brand);

    assert_eq!(tokens.color_primary, brand);
    assert_eq!(tokens.color_info, brand);
    assert!(!tokens.is_dark);
}

#[test]
fn from_primaries_uses_blue_seed_slot() {
    let mut primaries = [Color::from_rgb(1, 2, 3); PRIMARY_COUNT];
    let blue = Color::from_rgb(22, 119, 255);
    primaries[PRIMARY_BLUE_INDEX] = blue;

    let tokens = DesignTokens::from_primaries(primaries, true);

    assert_eq!(tokens.color_primary, blue);
    assert_eq!(tokens.color_info, blue);
    assert!(tokens.is_dark);
}

#[test]
fn dark_semantic_surfaces_stay_dark_and_borders_remain_subtle() {
    let tokens = DesignTokens::antd_dark();

    for color in [
        tokens.color_primary_bg,
        tokens.color_success_bg,
        tokens.color_warning_bg,
        tokens.color_error_bg,
        tokens.color_info_bg,
    ] {
        assert!(!color.is_light(), "dark semantic surface was {color}");
        assert!(color.luminance() > tokens.color_bg_layout.luminance());
    }
    assert!(tokens.color_border_secondary.luminance() < 80);
}

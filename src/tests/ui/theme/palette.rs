use crate::tests::common::*;
use crate::ui::theme::{
    generate_color_scale, FunctionalColorRole, PrimaryHue, BLUE_PALETTE, COLOR_SCALE_LEN,
    DATA_VISUALIZATION_COLOR_COUNT, DATA_VISUALIZATION_PALETTE, NEUTRAL_PALETTE, NEUTRAL_SCALE_LEN,
    PRIMARY_HUE_COUNT, PRIMARY_SHADE_INDEX,
};

#[test]
fn preset_hues_have_stable_order_and_primary_slot() {
    let expected = [
        ("red", Color::hex("#f5222d")),
        ("volcano", Color::hex("#fa541c")),
        ("orange", Color::hex("#fa8c16")),
        ("gold", Color::hex("#faad14")),
        ("yellow", Color::hex("#fadb14")),
        ("lime", Color::hex("#a0d911")),
        ("green", Color::hex("#52c41a")),
        ("cyan", Color::hex("#13c2c2")),
        ("blue", Color::hex("#1677ff")),
        ("geekblue", Color::hex("#2f54eb")),
        ("purple", Color::hex("#722ed1")),
        ("magenta", Color::hex("#eb2f96")),
    ];

    assert_eq!(PrimaryHue::ALL.len(), PRIMARY_HUE_COUNT);
    for (index, (hue, (name, primary))) in PrimaryHue::ALL.iter().copied().zip(expected).enumerate()
    {
        assert_eq!(hue.index(), index);
        assert_eq!(hue.name(), name);
        assert_eq!(hue.primary(), primary);
        assert_eq!(hue.palette().colors().len(), COLOR_SCALE_LEN);
        assert_eq!(hue.palette().shade(PRIMARY_SHADE_INDEX), Some(primary));
    }
}

#[test]
fn blue_palette_matches_ant_design_five() {
    let expected = [
        "#e6f4ff", "#bae0ff", "#91caff", "#69b1ff", "#4096ff", "#1677ff", "#0958d9", "#003eb3",
        "#002c8c", "#001d66",
    ]
    .map(Color::hex);

    assert_eq!(BLUE_PALETTE.colors(), &expected);
    assert_eq!(BLUE_PALETTE.shade(COLOR_SCALE_LEN), None);
}

#[test]
fn generator_reproduces_every_preset_palette() {
    for hue in PrimaryHue::ALL {
        assert_eq!(
            generate_color_scale(hue.primary()),
            *hue.palette(),
            "{hue:?}"
        );
    }
}

#[test]
fn generator_ignores_alpha_and_keeps_seed_in_primary_slot() {
    let seed = Color::from_rgba(102, 102, 102, 12);
    let palette = generate_color_scale(seed);
    let expected = [
        "#a6a6a6", "#999999", "#8c8c8c", "#808080", "#737373", "#666666", "#404040", "#1a1a1a",
        "#000000", "#000000",
    ]
    .map(Color::hex);

    assert_eq!(palette.colors(), &expected);
    assert_eq!(palette.primary(), Color::from_rgb(102, 102, 102));
}

#[test]
fn neutral_palette_exposes_thirteen_ordered_shades() {
    let expected = [
        "#ffffff", "#fafafa", "#f5f5f5", "#f0f0f0", "#d9d9d9", "#bfbfbf", "#8c8c8c", "#595959",
        "#434343", "#262626", "#1f1f1f", "#141414", "#000000",
    ]
    .map(Color::hex);

    assert_eq!(NEUTRAL_PALETTE.colors(), &expected);
    assert_eq!(NEUTRAL_PALETTE.colors().len(), NEUTRAL_SCALE_LEN);
    assert_eq!(NEUTRAL_PALETTE.shade(NEUTRAL_SCALE_LEN), None);
}

#[test]
fn functional_palettes_follow_active_theme_seeds() {
    let mut tokens = DesignTokens::antd_light();
    tokens.color_success = Color::hex("#00b96b");

    for role in [
        FunctionalColorRole::Success,
        FunctionalColorRole::Warning,
        FunctionalColorRole::Error,
        FunctionalColorRole::Info,
    ] {
        let expected_seed = match role {
            FunctionalColorRole::Success => tokens.color_success,
            FunctionalColorRole::Warning => tokens.color_warning,
            FunctionalColorRole::Error => tokens.color_error,
            FunctionalColorRole::Info => tokens.color_info,
        };
        assert_eq!(tokens.functional_color_scale(role).primary(), expected_seed);
    }

    assert_eq!(
        FunctionalColorRole::Error.default_palette().primary(),
        Color::hex("#ff4d4f")
    );
}

#[test]
fn data_visualization_palette_matches_antv_category_order() {
    let expected = [
        "#5b8ff9", "#61ddaa", "#65789b", "#f6bd16", "#7262fd", "#78d3f8", "#9661bc", "#f6903d",
        "#008685", "#f08bb4",
    ]
    .map(Color::hex);

    assert_eq!(DATA_VISUALIZATION_PALETTE.colors(), &expected);
    assert_eq!(
        DATA_VISUALIZATION_PALETTE.colors().len(),
        DATA_VISUALIZATION_COLOR_COUNT
    );
    assert_eq!(
        DATA_VISUALIZATION_PALETTE.color(DATA_VISUALIZATION_COLOR_COUNT),
        None
    );
}

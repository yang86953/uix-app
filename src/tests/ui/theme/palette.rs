use crate::tests::common::*;
use crate::ui::theme::{
    PrimaryHue, BLUE_PALETTE, COLOR_SCALE_LEN, PRIMARY_HUE_COUNT, PRIMARY_SHADE_INDEX,
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

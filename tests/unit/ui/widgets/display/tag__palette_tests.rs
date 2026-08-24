// 引入被测颜色解析入口与预设枚举。
use super::{TAG_VISUAL, TagColor, resolve_tag_colors};
// 引入标准明暗主题。
use crate::ui::{PrimaryHue, Theme};

// 品牌 token 与扩展色阶必须分别响应主题变化。
#[test]
fn tag_palette_follows_theme_mode() {
    // 构造标准亮色主题。
    let light = Theme::antd_light();
    // 构造标准暗色主题。
    let dark = Theme::antd_dark();
    // 蓝色预设必须直接使用亮色主题品牌色对。
    let light_blue = resolve_tag_colors(TagColor::Blue, &TAG_VISUAL.palette, light.tokens());
    // 核对品牌背景 token。
    assert_eq!(light_blue.0, light.tokens().color_primary_bg());
    // 核对品牌前景 token。
    assert_eq!(light_blue.1, light.tokens().color_primary());
    // 分别解析扩展青色的明暗色对。
    let light_cyan = resolve_tag_colors(TagColor::Cyan, &TAG_VISUAL.palette, light.tokens());
    // 解析暗色主题下的同一扩展色。
    let dark_cyan = resolve_tag_colors(TagColor::Cyan, &TAG_VISUAL.palette, dark.tokens());
    // 扩展色背景必须随明暗模式变化。
    assert_ne!(light_cyan.0, dark_cyan.0);
    // 亮色背景必须保持低强调的浅色表面。
    assert!(light_cyan.0.is_light());
    // 暗色背景必须切换为深色表面。
    assert!(!dark_cyan.0.is_light());
}

// UIX 色表顺序必须逐项对应全部公开 TagColor 变体。
#[test]
fn tag_palette_keeps_all_public_variant_mappings() {
    let theme = Theme::antd_light();
    let tokens = theme.tokens();
    let cases = [
        (
            TagColor::Default,
            (tokens.color_fill_tertiary(), tokens.color_text()),
        ),
        (
            TagColor::Success,
            (tokens.color_success_bg(), tokens.color_success()),
        ),
        (
            TagColor::Info,
            (tokens.color_info_bg(), tokens.color_info()),
        ),
        (
            TagColor::Warning,
            (tokens.color_warning_bg(), tokens.color_warning()),
        ),
        (
            TagColor::Error,
            (tokens.color_error_bg(), tokens.color_error()),
        ),
        (
            TagColor::Blue,
            (tokens.color_primary_bg(), tokens.color_primary()),
        ),
        (
            TagColor::Cyan,
            PrimaryHue::Cyan.palette().subtle_pair(false),
        ),
        (
            TagColor::Geekblue,
            PrimaryHue::Geekblue.palette().subtle_pair(false),
        ),
        (
            TagColor::Purple,
            PrimaryHue::Purple.palette().subtle_pair(false),
        ),
        (
            TagColor::Magenta,
            PrimaryHue::Magenta.palette().subtle_pair(false),
        ),
        (TagColor::Red, PrimaryHue::Red.palette().subtle_pair(false)),
        (
            TagColor::Orange,
            PrimaryHue::Orange.palette().subtle_pair(false),
        ),
        (
            TagColor::Gold,
            (tokens.color_warning_bg(), tokens.color_warning()),
        ),
        (
            TagColor::Lime,
            PrimaryHue::Lime.palette().subtle_pair(false),
        ),
        (
            TagColor::Green,
            (tokens.color_success_bg(), tokens.color_success()),
        ),
    ];
    for (color, expected) in cases {
        assert_eq!(
            resolve_tag_colors(color, &TAG_VISUAL.palette, tokens),
            expected,
            "{color:?}"
        );
    }
}

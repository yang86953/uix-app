use crate::core::EdgeInsets;
use crate::draw::Color;
use crate::ui::foundation::style::*;
use crate::ui::theme::NeutralRole;

// 鈹€鈹€ 榛樿鍊?/ 鏋勯€犲櫒 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn style_default_all_fields() {
    let s = Style::default();
    assert_eq!(s.margin, EdgeInsets::zero());
    assert_eq!(s.padding, EdgeInsets::zero());
    assert_eq!(s.border_color, None);
    assert_eq!(s.border_width, EdgeInsets::zero());
    assert_eq!(s.border_radius, 0.0);
    assert_eq!(s.width, None);
    assert_eq!(s.height, None);
    assert_eq!(s.display, DisplayMode::Flex);
    assert_eq!(s.flex_direction, FlexDirection::default());
    assert!(!s.flex_wrap);
    assert!(!s.overflow_content);
    assert_eq!(s.justify_content, JustifyContent::default());
    assert_eq!(s.align_items, AlignItems::default());
    assert_eq!(s.gap, 0.0);
    assert!(s.grid_template_columns.is_empty());
    assert!(s.grid_template_rows.is_empty());
    assert_eq!(s.grid_column_gap, 0.0);
    assert_eq!(s.grid_row_gap, 0.0);
    assert_eq!(s.flex_grow, 0.0);
    assert_eq!(s.flex_shrink, 1.0);
    assert_eq!(s.align_self, None);
    assert_eq!(s.background, None);
    assert_eq!(s.background_hover, None);
    assert_eq!(s.background_active, None);
    assert_eq!(s.color, ColorValue::Neutral(NeutralRole::Text));
    assert_eq!(s.font_size, TypographyToken::Body);
    assert_eq!(s.opacity, 1.0);
    assert_eq!(s.box_shadow, None);
    assert!(s.visible);
}

// 鈹€鈹€ 渚挎嵎棰勮 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn style_button_default() {
    let s = Style::button_default();
    assert_eq!(s.background, None);
    assert_eq!(
        s.border_color,
        Some(ColorValue::Neutral(NeutralRole::Border))
    );
    assert_eq!(s.border_width, EdgeInsets::uniform(1.0));
    assert_eq!(s.border_radius, 6.0);
    assert_eq!(s.padding, EdgeInsets::new(15.0, 0.0, 15.0, 0.0));
    assert_eq!(s.color, ColorValue::Neutral(NeutralRole::Text));
    assert_eq!(s.font_size, TypographyToken::Body);
}

#[test]
fn style_button_primary() {
    let s = Style::button_primary();
    assert_eq!(
        s.background,
        Some(ColorValue::Palette(PaletteColor::Primary))
    );
    assert_eq!(
        s.border_color,
        Some(ColorValue::Palette(PaletteColor::Primary))
    );
    assert_eq!(s.border_width, EdgeInsets::uniform(1.0));
    assert_eq!(s.border_radius, 6.0);
    assert_eq!(s.padding, EdgeInsets::new(15.0, 0.0, 15.0, 0.0));
    assert_eq!(s.color, ColorValue::Palette(PaletteColor::White));
    assert_eq!(s.font_size, TypographyToken::Body);
}

#[test]
fn style_grid_template_builders() {
    let s = Style::default()
        .with_grid_columns(vec![GridTrack::Fr(1.0), GridTrack::Px(120.0)])
        .with_grid_rows(vec![GridTrack::Auto])
        .with_grid_gap(8.0, 12.0);

    assert_eq!(
        s.grid_template_columns,
        vec![GridTrack::Fr(1.0), GridTrack::Px(120.0)]
    );
    assert_eq!(s.grid_template_rows, vec![GridTrack::Auto]);
    assert_eq!(s.grid_column_gap, 8.0);
    assert_eq!(s.grid_row_gap, 12.0);
}

// 鈹€鈹€ effective_bg 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn effective_bg_pressed_returns_active() {
    let s = Style {
        background: Some(ColorValue::Custom(Color::red())),
        background_hover: Some(ColorValue::Custom(Color::green())),
        background_active: Some(ColorValue::Custom(Color::blue())),
        ..Style::default()
    };
    assert_eq!(
        s.effective_bg(true, true),
        Some(ColorValue::Custom(Color::blue()))
    );
}

#[test]
fn effective_bg_pressed_fallback_to_background() {
    let s = Style {
        background: Some(ColorValue::Custom(Color::red())),
        background_hover: None,
        background_active: None,
        ..Style::default()
    };
    assert_eq!(
        s.effective_bg(false, true),
        Some(ColorValue::Custom(Color::red()))
    );
}

#[test]
fn effective_bg_hovered_returns_hover() {
    let s = Style {
        background: Some(ColorValue::Custom(Color::red())),
        background_hover: Some(ColorValue::Custom(Color::green())),
        background_active: None,
        ..Style::default()
    };
    assert_eq!(
        s.effective_bg(true, false),
        Some(ColorValue::Custom(Color::green()))
    );
}

#[test]
fn effective_bg_hovered_no_hover_fallback() {
    let s = Style {
        background: Some(ColorValue::Custom(Color::red())),
        background_hover: None,
        background_active: None,
        ..Style::default()
    };
    assert_eq!(
        s.effective_bg(true, false),
        Some(ColorValue::Custom(Color::red()))
    );
}

#[test]
fn effective_bg_normal_returns_background() {
    let s = Style {
        background: Some(ColorValue::Custom(Color::red())),
        ..Style::default()
    };
    assert_eq!(
        s.effective_bg(false, false),
        Some(ColorValue::Custom(Color::red()))
    );
}

#[test]
fn effective_bg_all_none_returns_none() {
    let s = Style {
        background: None,
        background_hover: None,
        background_active: None,
        ..Style::default()
    };
    assert_eq!(s.effective_bg(true, true), None);
    assert_eq!(s.effective_bg(true, false), None);
    assert_eq!(s.effective_bg(false, false), None);
}

// 鈹€鈹€ 閾惧紡 Builder 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn chain_builder_all_methods() {
    let shadow = BoxShadowDef::new(Color::black(), 4.0, 2.0, 2.0);
    let s = Style::default()
        .with_margin(EdgeInsets::uniform(8.0))
        .with_padding(EdgeInsets::uniform(4.0))
        .with_border(Color::red(), 2.0)
        .with_rounded(6.0)
        .with_width(100.0)
        .with_height(50.0)
        .with_size(200.0, 100.0)
        .with_display(DisplayMode::None)
        .with_direction(FlexDirection::Column)
        .with_wrap(true)
        .with_justify(JustifyContent::Center)
        .with_align(AlignItems::Center)
        .with_gap(8.0)
        .with_grow(1.0)
        .with_shrink(0.0)
        .with_bg(Color::blue())
        .with_bg_hover(Color::green())
        .with_bg_active(Color::red())
        .with_color(Color::white())
        .with_font_size(16.0)
        .with_opacity(0.5)
        .with_shadow(shadow)
        .with_visible(false);

    assert_eq!(s.margin, EdgeInsets::uniform(8.0));
    assert_eq!(s.padding, EdgeInsets::uniform(4.0));
    assert_eq!(s.border_color, Some(ColorValue::Custom(Color::red())));
    assert_eq!(s.border_width, EdgeInsets::uniform(2.0));
    assert_eq!(s.border_radius, 6.0);
    assert_eq!(s.width, Some(200.0)); // with_size overrides with_width
    assert_eq!(s.height, Some(100.0));
    assert_eq!(s.display, DisplayMode::None);
    assert_eq!(s.flex_direction, FlexDirection::Column);
    assert!(s.flex_wrap);
    assert_eq!(s.justify_content, JustifyContent::Center);
    assert_eq!(s.align_items, AlignItems::Center);
    assert_eq!(s.gap, 8.0);
    assert_eq!(s.flex_grow, 1.0);
    assert_eq!(s.flex_shrink, 0.0);
    assert_eq!(s.background, Some(ColorValue::Custom(Color::blue())));
    assert_eq!(s.background_hover, Some(ColorValue::Custom(Color::green())));
    assert_eq!(s.background_active, Some(ColorValue::Custom(Color::red())));
    assert_eq!(s.color, ColorValue::Custom(Color::white()));
    assert_eq!(s.font_size, TypographyToken::Custom(16.0));
    assert_eq!(s.opacity, 0.5);
    assert_eq!(
        s.box_shadow,
        Some(BoxShadowDef::new(Color::black(), 4.0, 2.0, 2.0))
    );
    assert!(!s.visible);
}

// 鈹€鈹€ apply 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn apply_non_default_overrides_default_preserved() {
    let base = Style::default();
    let patch = Style {
        background: Some(ColorValue::Custom(Color::red())),
        border_color: Some(ColorValue::Custom(Color::blue())),
        width: Some(100.0),
        ..Style::default()
    };
    let result = base.apply(patch);
    assert_eq!(result.background, Some(ColorValue::Custom(Color::red())));
    assert_eq!(result.border_color, Some(ColorValue::Custom(Color::blue())));
    assert_eq!(result.width, Some(100.0));
    // 榛樿瀛楁搴斾繚鐣?    assert_eq!(result.margin, EdgeInsets::zero());
    assert_eq!(result.padding, EdgeInsets::zero());
    assert_eq!(result.height, None);
    assert_eq!(result.display, DisplayMode::Flex);
    assert_eq!(result.flex_grow, 0.0);
    assert_eq!(result.flex_shrink, 1.0);
    assert_eq!(result.color, ColorValue::Neutral(NeutralRole::Text));
    assert_eq!(result.font_size, TypographyToken::Body);
    assert!(result.visible);
}

#[test]
fn apply_empty_patch_preserves_all() {
    let base = Style {
        background: Some(ColorValue::Custom(Color::red())),
        border_color: Some(ColorValue::Custom(Color::blue())),
        width: Some(100.0),
        ..Style::default()
    };
    let patch = Style::default();
    let result = base.apply(patch);
    assert_eq!(result.background, Some(ColorValue::Custom(Color::red())));
    assert_eq!(result.border_color, Some(ColorValue::Custom(Color::blue())));
    assert_eq!(result.width, Some(100.0));
}

#[test]
fn apply_edge_cases_zero_and_none() {
    let base = Style {
        margin: EdgeInsets::uniform(5.0),
        gap: 10.0,
        flex_shrink: 0.0,
        visible: false,
        ..Style::default()
    };
    // A zero-valued patch does not override gap, because 0.0 is the default.
    let patch = Style {
        margin: EdgeInsets::zero(), // Does not apply because it equals zero.
        gap: 0.0,                   // Does not override.
        ..Style::default()
    };
    let result = base.apply(patch);
    assert_eq!(result.margin, EdgeInsets::uniform(5.0)); // preserved
    assert_eq!(result.gap, 10.0); // preserved
    assert!(!result.visible); // false is preserved
}

// with_style

#[test]
fn with_style_full_replace() {
    let original = Style::default()
        .with_bg(Color::red())
        .with_color(Color::white());
    let replacement = Style::default().with_bg(Color::blue()).with_font_size(20.0);
    let result = original.with_style(replacement);
    // 鎵€鏈夊瓧娈靛簲鍏ㄩ儴鏇挎崲
    assert_eq!(result.background, Some(ColorValue::Custom(Color::blue())));
    assert_eq!(result.color, ColorValue::Neutral(NeutralRole::Text)); // default
    assert_eq!(result.font_size, TypographyToken::Custom(20.0));
}

// 鈹€鈹€ StyleSet 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn style_set_resolve_priority() {
    let normal = Style::default().with_bg(Color::red());
    let hover = Style::default().with_bg(Color::green());
    let active = Style::default().with_bg(Color::blue());
    let focused = Style::default().with_bg(Color::from_rgb(255, 255, 0));
    let disabled = Style::default().with_bg(Color::from_rgb(128, 128, 128));
    let v = StyleSet::new(normal.clone())
        .hover(hover)
        .active(active)
        .focused(focused)
        .disabled(disabled);
    // 浼樺厛绾э細disabled > active > hover > normal
    assert_eq!(
        v.resolve_flags(true, true, true, true).background,
        Some(ColorValue::Custom(Color::from_rgb(128, 128, 128)))
    );
    assert_eq!(
        v.resolve_flags(true, true, true, false).background,
        Some(ColorValue::Custom(Color::from_rgb(255, 255, 0)))
    );
    assert_eq!(
        v.resolve_flags(true, true, false, false).background,
        Some(ColorValue::Custom(Color::blue()))
    );
    assert_eq!(
        v.resolve_flags(true, false, false, false).background,
        Some(ColorValue::Custom(Color::green()))
    );
    assert_eq!(
        v.resolve_flags(false, false, false, false).background,
        Some(ColorValue::Custom(Color::red()))
    );
}

#[test]
fn style_set_resolve_fallback() {
    let normal = Style::default().with_bg(Color::red());
    let v = StyleSet::new(normal.clone());
    // Falls back to normal when hover/active/disabled styles are absent.
    assert_eq!(
        v.resolve_flags(true, true, false, false).background,
        Some(ColorValue::Custom(Color::red()))
    );
    assert_eq!(
        v.resolve_flags(true, false, false, false).background,
        Some(ColorValue::Custom(Color::red()))
    );
    assert_eq!(
        v.resolve_flags(false, true, false, false).background,
        Some(ColorValue::Custom(Color::red()))
    );
    assert_eq!(
        v.resolve_flags(false, false, false, true).background,
        Some(ColorValue::Custom(Color::red()))
    );
}

#[test]
fn button_presets_cover_focused_and_disabled_states() {
    for preset in [StyleSet::button_ghost(), StyleSet::button_danger()] {
        let focused = preset.resolve_flags(false, false, true, false);
        assert!(focused.box_shadow.is_some());

        let disabled = preset.resolve_flags(true, true, true, true);
        assert_eq!(disabled.opacity, 0.45);
        assert_eq!(
            disabled.border_color,
            Some(ColorValue::Neutral(NeutralRole::Border))
        );
        assert_eq!(
            disabled.color,
            ColorValue::Neutral(NeutralRole::TextQuaternary)
        );
    }
}

// 鈹€鈹€ style! 瀹?鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn style_macro_basic() {
    let s = crate::style! {
        bg: Color::red(),
        color: Color::white(),
        rounded: 6,
        margin: EdgeInsets::uniform(8.0),
    };
    assert_eq!(s.background, Some(ColorValue::Custom(Color::red())));
    assert_eq!(s.color, ColorValue::Custom(Color::white()));
    assert_eq!(s.border_radius, 6.0);
    assert_eq!(s.margin, EdgeInsets::uniform(8.0));
}

#[test]
fn style_macro_grid_template_fields() {
    let s = crate::style! {
        grid_columns: vec![GridTrack::Fr(1.0), GridTrack::Px(96.0)],
        grid_rows: vec![GridTrack::Auto],
        grid_gap: (6.0, 10.0),
    };

    assert_eq!(
        s.grid_template_columns,
        vec![GridTrack::Fr(1.0), GridTrack::Px(96.0)]
    );
    assert_eq!(s.grid_template_rows, vec![GridTrack::Auto]);
    assert_eq!(s.grid_column_gap, 6.0);
    assert_eq!(s.grid_row_gap, 10.0);
}

#[test]
fn style_macro_aliases_bare() {
    let s = crate::style! {
        bg: Color::blue(),
        background_hover: Color::from_rgb(173, 216, 255),
        background_active: Color::from_rgb(0, 0, 139),
        fs: 16,
        opacity: 0.8,
        visible: true,
        w: 100,
        h: 50,
        padding: EdgeInsets::uniform(4.0),
        direction: FlexDirection::Row,
        wrap: true,
        justify: JustifyContent::Center,
        align: AlignItems::Center,
        gap: 8,
        grow: 1,
        shrink: 0,
    };
    assert_eq!(s.background, Some(ColorValue::Custom(Color::blue())));
    assert_eq!(
        s.background_hover,
        Some(ColorValue::Custom(Color::from_rgb(173, 216, 255)))
    );
    assert_eq!(
        s.background_active,
        Some(ColorValue::Custom(Color::from_rgb(0, 0, 139)))
    );
    assert_eq!(s.font_size, TypographyToken::Custom(16.0));
    assert_eq!(s.opacity, 0.8);
    assert!(s.visible);
    assert_eq!(s.width, Some(100.0));
    assert_eq!(s.height, Some(50.0));
    assert_eq!(s.padding, EdgeInsets::uniform(4.0));
    assert_eq!(s.flex_direction, FlexDirection::Row);
    assert!(s.flex_wrap);
    assert_eq!(s.justify_content, JustifyContent::Center);
    assert_eq!(s.align_items, AlignItems::Center);
    assert_eq!(s.gap, 8.0);
    assert_eq!(s.flex_grow, 1.0);
    assert_eq!(s.flex_shrink, 0.0);
}

#[test]
fn style_macro_border_and_shadow() {
    // Tuple syntax: `border: (color, width)`, `shadow: (color, blur, ox, oy)`.
    let s = crate::style! {
        border: (Color::red(), 2.0),
        shadow: (Color::black(), 4.0, 2.0, 2.0),
    };
    assert_eq!(s.border_color, Some(ColorValue::Custom(Color::red())));
    assert_eq!(s.border_width, EdgeInsets::uniform(2.0));
    assert_eq!(
        s.box_shadow,
        Some(BoxShadowDef::new(Color::black(), 4.0, 2.0, 2.0))
    );
}

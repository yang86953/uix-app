use super::*;
use crate::draw::Color;
use crate::native::EdgeInsets;

// ── 默认值 / 构造器 ────────────────────────────────────

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
    assert_eq!(s.flex_grow, 0.0);
    assert_eq!(s.flex_shrink, 1.0);
    assert_eq!(s.align_self, None);
    assert_eq!(s.background, None);
    assert_eq!(s.background_hover, None);
    assert_eq!(s.background_active, None);
    assert_eq!(s.color, Color::black());
    assert_eq!(s.font_size, 14.0);
    assert_eq!(s.opacity, 1.0);
    assert_eq!(s.box_shadow, None);
    assert!(s.visible);
}

#[test]
fn style_new_equals_default() {
    assert_eq!(Style::new(), Style::default());
}

// ── 便捷预设 ──────────────────────────────────────────

#[test]
fn style_row() {
    let s = Style::row();
    assert_eq!(s.display, DisplayMode::Flex);
    assert_eq!(s.flex_direction, FlexDirection::Row);
    // 其余字段应与 default 一致
    let mut expected = Style::default();
    expected.display = DisplayMode::Flex;
    expected.flex_direction = FlexDirection::Row;
    assert_eq!(s, expected);
}

#[test]
fn style_column() {
    let s = Style::column();
    assert_eq!(s.display, DisplayMode::Flex);
    assert_eq!(s.flex_direction, FlexDirection::default()); // Column
                                                            // 其余字段应与 default 一致
    let mut expected = Style::default();
    expected.display = DisplayMode::Flex;
    assert_eq!(s, expected);
}

#[test]
fn style_button_default() {
    let s = Style::button_default();
    assert_eq!(s.background, None);
    assert_eq!(s.border_color, Some(Color::from_rgba(217, 217, 217, 255)));
    assert_eq!(s.border_width, EdgeInsets::uniform(1.0));
    assert_eq!(s.border_radius, 6.0);
    assert_eq!(s.padding, EdgeInsets::new(15.0, 0.0, 15.0, 0.0));
    assert_eq!(s.color, Color::from_rgb(0, 0, 0));
    assert_eq!(s.font_size, 14.0);
}

#[test]
fn style_button_primary() {
    let s = Style::button_primary();
    assert_eq!(s.background, Some(Color::from_rgba(22, 119, 255, 255)));
    assert_eq!(s.border_color, Some(Color::from_rgba(22, 119, 255, 255)));
    assert_eq!(s.border_width, EdgeInsets::uniform(1.0));
    assert_eq!(s.border_radius, 6.0);
    assert_eq!(s.padding, EdgeInsets::new(15.0, 0.0, 15.0, 0.0));
    assert_eq!(s.color, Color::white());
    assert_eq!(s.font_size, 14.0);
}

#[test]
fn style_container() {
    let s = Style::container();
    assert_eq!(s.display, DisplayMode::Flex);
    // 其余字段应与 default 一致
    let mut expected = Style::default();
    expected.display = DisplayMode::Flex;
    assert_eq!(s, expected);
}

// ── effective_bg ───────────────────────────────────────

#[test]
fn effective_bg_pressed_returns_active() {
    let s = Style {
        background: Some(Color::red()),
        background_hover: Some(Color::green()),
        background_active: Some(Color::blue()),
        ..Style::default()
    };
    assert_eq!(s.effective_bg(true, true), Some(Color::blue()));
}

#[test]
fn effective_bg_pressed_fallback_to_background() {
    let s = Style {
        background: Some(Color::red()),
        background_hover: None,
        background_active: None,
        ..Style::default()
    };
    assert_eq!(s.effective_bg(false, true), Some(Color::red()));
}

#[test]
fn effective_bg_hovered_returns_hover() {
    let s = Style {
        background: Some(Color::red()),
        background_hover: Some(Color::green()),
        background_active: None,
        ..Style::default()
    };
    assert_eq!(s.effective_bg(true, false), Some(Color::green()));
}

#[test]
fn effective_bg_hovered_no_hover_fallback() {
    let s = Style {
        background: Some(Color::red()),
        background_hover: None,
        background_active: None,
        ..Style::default()
    };
    assert_eq!(s.effective_bg(true, false), Some(Color::red()));
}

#[test]
fn effective_bg_normal_returns_background() {
    let s = Style {
        background: Some(Color::red()),
        ..Style::default()
    };
    assert_eq!(s.effective_bg(false, false), Some(Color::red()));
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

// ── 链式 Builder ──────────────────────────────────────

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
    assert_eq!(s.border_color, Some(Color::red()));
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
    assert_eq!(s.background, Some(Color::blue()));
    assert_eq!(s.background_hover, Some(Color::green()));
    assert_eq!(s.background_active, Some(Color::red()));
    assert_eq!(s.color, Color::white());
    assert_eq!(s.font_size, 16.0);
    assert_eq!(s.opacity, 0.5);
    assert_eq!(
        s.box_shadow,
        Some(BoxShadowDef::new(Color::black(), 4.0, 2.0, 2.0))
    );
    assert!(!s.visible);
}

// ── apply ────────────────────────────────────────────

#[test]
fn apply_non_default_overrides_default_preserved() {
    let base = Style::default();
    let patch = Style {
        background: Some(Color::red()),
        border_color: Some(Color::blue()),
        width: Some(100.0),
        ..Style::default()
    };
    let result = base.apply(patch);
    assert_eq!(result.background, Some(Color::red()));
    assert_eq!(result.border_color, Some(Color::blue()));
    assert_eq!(result.width, Some(100.0));
    // 默认字段应保留
    assert_eq!(result.margin, EdgeInsets::zero());
    assert_eq!(result.padding, EdgeInsets::zero());
    assert_eq!(result.height, None);
    assert_eq!(result.display, DisplayMode::Flex);
    assert_eq!(result.flex_grow, 0.0);
    assert_eq!(result.flex_shrink, 1.0);
    assert_eq!(result.color, Color::black());
    assert_eq!(result.font_size, 14.0);
    assert!(result.visible);
}

#[test]
fn apply_empty_patch_preserves_all() {
    let base = Style {
        background: Some(Color::red()),
        border_color: Some(Color::blue()),
        width: Some(100.0),
        ..Style::default()
    };
    let patch = Style::default();
    let result = base.apply(patch);
    assert_eq!(result.background, Some(Color::red()));
    assert_eq!(result.border_color, Some(Color::blue()));
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
    // patch 中 gap=0.0 不会覆盖（因为 gap 默认是 0.0，不触发覆盖）
    let patch = Style {
        margin: EdgeInsets::zero(), // 不会被 apply，因为等于 zero()
        gap: 0.0,                   // 不会覆盖
        ..Style::default()
    };
    let result = base.apply(patch);
    assert_eq!(result.margin, EdgeInsets::uniform(5.0)); // 保留
    assert_eq!(result.gap, 10.0); // 保留
    assert!(!result.visible); // false 被保留（patch visible=true，但 only if !other.visible）
}

// ── with_style ───────────────────────────────────────

#[test]
fn with_style_full_replace() {
    let original = Style::default()
        .with_bg(Color::red())
        .with_color(Color::white());
    let replacement = Style::default().with_bg(Color::blue()).with_font_size(20.0);
    let result = original.with_style(replacement);
    // 所有字段应全部替换
    assert_eq!(result.background, Some(Color::blue()));
    assert_eq!(result.color, Color::black()); // default
    assert_eq!(result.font_size, 20.0);
}

// ── BoxShadowDef ─────────────────────────────────────

#[test]
fn box_shadow_def_new() {
    let s = BoxShadowDef::new(Color::red(), 4.0, 2.0, 1.0);
    assert_eq!(s.color, Color::red());
    assert_eq!(s.blur, 4.0);
    assert_eq!(s.offset_x, 2.0);
    assert_eq!(s.offset_y, 1.0);
}

// ── StyleSet ─────────────────────────────────────

#[test]
fn style_set_new() {
    let base = Style::default().with_bg(Color::red());
    let v = StyleSet::new(base.clone());
    assert_eq!(v.normal, base);
    assert_eq!(v.hover, None);
    assert_eq!(v.pressed, None);
    assert_eq!(v.disabled, None);
}

#[test]
fn style_set_resolve_priority() {
    let normal = Style::default().with_bg(Color::red());
    let hover = Style::default().with_bg(Color::green());
    let active = Style::default().with_bg(Color::blue());
    let disabled = Style::default().with_bg(Color::from_rgb(128, 128, 128));
    let v = StyleSet::new(normal.clone())
        .hover(hover)
        .active(active)
        .disabled(disabled);
    // 优先级：disabled > active > hover > normal
    assert_eq!(
        v.resolve_flags(false, false, false, true).background,
        Some(Color::from_rgb(128, 128, 128))
    );
    assert_eq!(
        v.resolve_flags(false, true, false, false).background,
        Some(Color::blue())
    );
    assert_eq!(
        v.resolve_flags(true, false, false, false).background,
        Some(Color::green())
    );
    assert_eq!(
        v.resolve_flags(false, false, false, false).background,
        Some(Color::red())
    );
}

#[test]
fn style_set_resolve_fallback() {
    let normal = Style::default().with_bg(Color::red());
    let v = StyleSet::new(normal.clone());
    // 无 hover/active/disabled 时回退到 normal
    assert_eq!(
        v.resolve_flags(true, true, false, false).background,
        Some(Color::red())
    );
    assert_eq!(
        v.resolve_flags(true, false, false, false).background,
        Some(Color::red())
    );
    assert_eq!(
        v.resolve_flags(false, true, false, false).background,
        Some(Color::red())
    );
    assert_eq!(
        v.resolve_flags(false, false, false, true).background,
        Some(Color::red())
    );
}

#[test]
fn style_set_chain_methods() {
    let normal = Style::default().with_bg(Color::red());
    let hover = Style::default().with_bg(Color::green());
    let active = Style::default().with_bg(Color::blue());
    let disabled = Style::default().with_bg(Color::from_rgb(128, 128, 128));
    let v = StyleSet::new(normal)
        .hover(hover.clone())
        .active(active.clone())
        .disabled(disabled.clone());
    assert_eq!(v.hover, Some(hover));
    assert_eq!(v.pressed, Some(active));
    assert_eq!(v.disabled, Some(disabled));
}

// ── edge_insets 辅助函数 ─────────────────────────────

#[test]
fn edge_insets_all_uniform() {
    let insets = edge_insets::all(8.0);
    assert_eq!(insets, EdgeInsets::uniform(8.0));
    assert_eq!(insets.left, 8.0);
    assert_eq!(insets.top, 8.0);
    assert_eq!(insets.right, 8.0);
    assert_eq!(insets.bottom, 8.0);
}

#[test]
fn edge_insets_symmetric() {
    let insets = edge_insets::symmetric(10.0, 20.0);
    assert_eq!(insets.left, 20.0);
    assert_eq!(insets.top, 10.0);
    assert_eq!(insets.right, 20.0);
    assert_eq!(insets.bottom, 10.0);
}

#[test]
fn edge_insets_trbl() {
    let insets = edge_insets::trbl(1.0, 2.0, 3.0, 4.0);
    assert_eq!(insets.left, 4.0);
    assert_eq!(insets.top, 1.0);
    assert_eq!(insets.right, 2.0);
    assert_eq!(insets.bottom, 3.0);
}

// ── style! 宏 ────────────────────────────────────────

#[test]
fn style_macro_basic() {
    let s = crate::style! {
        bg: Color::red(),
        color: Color::white(),
        rounded: 6,
        margin: EdgeInsets::uniform(8.0),
    };
    assert_eq!(s.background, Some(Color::red()));
    assert_eq!(s.color, Color::white());
    assert_eq!(s.border_radius, 6.0);
    assert_eq!(s.margin, EdgeInsets::uniform(8.0));
}

#[test]
fn style_macro_empty() {
    let s = crate::style! {};
    assert_eq!(s, Style::default());
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
    assert_eq!(s.background, Some(Color::blue()));
    assert_eq!(s.background_hover, Some(Color::from_rgb(173, 216, 255)));
    assert_eq!(s.background_active, Some(Color::from_rgb(0, 0, 139)));
    assert_eq!(s.font_size, 16.0);
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
    // 元组语法：`border: (color, width)`, `shadow: (color, blur, ox, oy)`
    let s = crate::style! {
        border: (Color::red(), 2.0),
        shadow: (Color::black(), 4.0, 2.0, 2.0),
    };
    assert_eq!(s.border_color, Some(Color::red()));
    assert_eq!(s.border_width, EdgeInsets::uniform(2.0));
    assert_eq!(
        s.box_shadow,
        Some(BoxShadowDef::new(Color::black(), 4.0, 2.0, 2.0))
    );
}

#[test]
fn uniform_insets_helper() {
    let insets = uniform_insets(12.0);
    assert_eq!(insets, EdgeInsets::uniform(12.0));
}

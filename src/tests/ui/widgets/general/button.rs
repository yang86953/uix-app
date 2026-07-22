use crate::tests::common::*;
use crate::ui::style::{ColorValue, StyleSet};
use crate::ui::view::View;
use crate::ui::widgets::general::button::*;
use crate::ui::widgets::general::ButtonGroup;

fn render_button(button: &Button) -> Vec<u32> {
    render_button_with_focus_visibility(button, true)
}

fn render_button_with_focus_visibility(button: &Button, focus_visible: bool) -> Vec<u32> {
    let mut canvas = crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer::new(
        crate::draw::engine::cpu::pixel_surface::PixelSurface::new(180, 48),
    );
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut tree = WidgetTree::new();
    tree.set_keyboard_focus_visible(focus_visible);
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        font,
        &fonts,
        &images,
        &tokens,
        96.0,
        1.0,
        crate::draw::spatial::Orientation::YDown,
        180,
        48,
    );
    WidgetRender::render(button, Rect::new(4.0, 8.0, 172.0, 32.0), &mut ctx, &tree);
    canvas.surface().pixels().to_vec()
}

#[test]
fn custom_focus_style_only_renders_for_keyboard_visible_focus() {
    let base = Style::button_default();
    let focused = Style {
        border_color: Some(ColorValue::custom(Color::from_rgb(255, 0, 0))),
        border_width: EdgeInsets::uniform(3.0),
        ..Style::default()
    };
    let style_set = StyleSet::new(base).focused(focused);
    let idle = Button::new("Action").style_set(style_set.clone());
    let mut focused = Button::new("Action").style_set(style_set);
    focused.on_event(&SystemEvent::FocusIn);

    assert_eq!(
        render_button_with_focus_visibility(&focused, false),
        render_button(&idle),
        "pointer focus must keep the idle button border"
    );
    assert_ne!(
        render_button_with_focus_visibility(&focused, true),
        render_button(&idle),
        "keyboard focus must retain the custom focus border"
    );
}

#[test]
fn measure_clamps_button_size() {
    let measured = Button::new("abcdef").measure(Constraints::loose(Size::new(40.0, 24.0)));

    assert_eq!(measured, Size::new(40.0, 24.0));
}

#[test]
fn measure_reserves_full_width_for_cjk_button_text() {
    let measured = Button::new("保存所有更改").measure(Constraints::unconstrained());

    assert_eq!(measured.w, 114.0);
}

#[test]
fn measure_uses_custom_style_font_size_for_cjk_button_text() {
    let measured = Button::new("保存所有更改")
        .style(Style::default().with_font_size(20.0))
        .measure(Constraints::unconstrained());

    assert_eq!(measured.w, 150.0);
}

#[test]
fn invalid_style_font_sizes_fall_back_in_measure_and_render() {
    let expected_size = Button::new("保存所有更改").measure(Constraints::unconstrained());
    let expected_pixels = render_button(&Button::new("Status"));

    for invalid in [f32::NAN, f32::INFINITY, 0.0, -4.0] {
        let invalid_style = Style::default().with_font_size(invalid);
        let measured = Button::new("保存所有更改")
            .style(invalid_style.clone())
            .measure(Constraints::unconstrained());
        assert_eq!(measured, expected_size);
        assert_eq!(
            render_button(&Button::new("Status").style(invalid_style)),
            expected_pixels
        );
    }
}

#[test]
fn focused_and_pressed_keep_idle_colors() {
    // #176：点击/聚焦不改 fill/border/text；反馈仅 ripple。
    let mut btn = Button::new("ok").primary();
    let idle = btn.resolve_style();
    btn.focused = true;
    btn.pressed = true;
    let pressed = btn.resolve_style();
    assert_eq!(pressed.background, idle.background);
    assert_eq!(pressed.border_color, idle.border_color);
    assert_eq!(pressed.color, idle.color);
    assert_eq!(pressed.opacity, 1.0);

    btn.pressed = false;
    let focused = btn.resolve_style();
    assert_eq!(focused.background, idle.background);
    assert_eq!(focused.border_color, idle.border_color);
    assert_eq!(focused.color, idle.color);
    assert_eq!(focused.opacity, 1.0);
}

#[test]
fn default_pressed_keeps_idle_colors() {
    let mut btn = Button::new("ok");
    let idle = btn.resolve_style();
    btn.pressed = true;
    let style = btn.resolve_style();
    assert_eq!(style.background, idle.background);
    assert_eq!(style.border_color, idle.border_color);
    assert_eq!(style.color, idle.color);
}

#[test]
fn pointer_down_starts_ripple_from_local_pos() {
    let mut btn = Button::new("ok").primary();
    let result = btn.on_event(&SystemEvent::PointerDown {
        pos: Point::new(12.0, 8.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(result, EventResult::Handled);
    assert!(btn.pressed);
    assert_eq!(
        btn.ripple.as_ref().expect("ripple started").origin,
        Point::new(12.0, 8.0)
    );
    assert!(btn.update_animation(0.016));
    assert!(btn.ripple.as_ref().expect("ripple").expand.progress() > 0.0);
}

#[test]
fn secondary_pointer_does_not_press_or_start_ripple() {
    let mut btn = Button::new("ok").primary();
    let down = btn.on_event(&SystemEvent::PointerDown {
        pos: Point::new(12.0, 8.0),
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    });
    let up = btn.on_event(&SystemEvent::PointerUp {
        pos: Point::new(12.0, 8.0),
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    });

    assert_eq!(down, EventResult::NotHandled);
    assert_eq!(up, EventResult::NotHandled);
    assert!(!btn.pressed);
    assert!(btn.ripple.is_none());
}

#[test]
fn ripple_settles_while_held_then_fades_on_release() {
    let mut btn = Button::new("ok");
    let _ = btn.on_event(&SystemEvent::PointerDown {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    // 扩满后按住：不再需要动画帧。
    assert!(!btn.update_animation(ButtonRipple::EXPAND_SECS + 0.01));
    assert!(btn.ripple.is_some());

    let _ = btn.on_event(&SystemEvent::PointerUp {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert!(!btn.pressed);
    assert!(btn.update_animation(0.016));

    assert!(!btn.update_animation(ButtonRipple::FADE_SECS + 0.01));
    assert!(btn.ripple.is_none());
}

#[test]
fn cover_radius_reaches_farthest_corner() {
    let origin = Point::new(10.0, 5.0);
    let size = Size::new(100.0, 40.0);
    let r = cover_radius(origin, size);
    let expected = (90.0_f32).hypot(35.0);
    assert!((r - expected).abs() < 0.01);
}

#[test]
fn ripple_is_clipped_to_the_button_rounded_rect() {
    let idle = Button::new("ok");
    let mut pressed = Button::new("ok");
    let _ = pressed.on_event(&SystemEvent::PointerDown {
        pos: Point::new(2.0, 2.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert!(!pressed.update_animation(ButtonRipple::EXPAND_SECS + 0.01));

    let idle_pixels = render_button(&idle);
    let pressed_pixels = render_button(&pressed);
    let at = |pixels: &[u32], x: usize, y: usize| pixels[y * 180 + x];

    for (x, y) in [(4, 8), (5, 8), (4, 9), (175, 8), (4, 39)] {
        assert_eq!(
            at(&pressed_pixels, x, y),
            at(&idle_pixels, x, y),
            "ripple must not paint the clipped rounded corner at ({x}, {y})"
        );
    }
    assert_ne!(
        at(&pressed_pixels, 12, 16),
        at(&idle_pixels, 12, 16),
        "ripple must remain visible inside the rounded button"
    );

    let path = rounded_rect_circle_intersection(
        Rect::new(4.0, 8.0, 172.0, 32.0),
        pressed.resolve_style().border_radius,
        Point::new(6.0, 10.0),
        cover_radius(Point::new(2.0, 2.0), Size::new(172.0, 32.0)),
    )
    .expect("expanded ripple intersects the button");
    let vertices = crate::draw::primitives::tessellator::tessellate_fill(
        &path,
        crate::draw::primitives::path::FillRule::NonZero,
    )
    .expect("native GPU path tessellation");
    assert!(vertices.len() >= 6);
}

#[test]
fn dirty_bounds_only_while_ripple_active() {
    let mut btn = Button::new("ok");
    let frame = Rect::new(0.0, 0.0, 80.0, 32.0);
    assert_eq!(btn.dirty_bounds(frame), Rect::zero());

    let _ = btn.on_event(&SystemEvent::PointerDown {
        pos: Point::new(1.0, 1.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    // 事件路径由树 invalidate；动画脏区仅在 update 推进后置位。
    assert_eq!(btn.dirty_bounds(frame), Rect::zero());

    assert!(btn.update_animation(0.016));
    assert_eq!(btn.dirty_bounds(frame), frame);

    // 扩满当帧仍标脏；再 tick 无变化则零脏（按住静止，#105）。
    assert!(!btn.update_animation(ButtonRipple::EXPAND_SECS));
    assert_eq!(btn.dirty_bounds(frame), frame);
    assert!(!btn.update_animation(0.016));
    assert_eq!(btn.dirty_bounds(frame), Rect::zero());
}

#[test]
fn keyboard_activation_starts_centered_ripple() {
    let mut btn = Button::new("ok");
    let _ = btn.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    let ripple = btn.ripple.as_ref().expect("ripple");
    assert_eq!(ripple.origin, Button::CENTER_ORIGIN);
    assert!(btn.pressed);
}

#[test]
fn loading_button_animates_only_while_loading_and_rejects_input() {
    let mut button = Button::new("Save").loading(true);
    let frame = Rect::new(0.0, 0.0, 80.0, 32.0);

    assert_eq!(
        button.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    let before = render_button(&button);
    assert!(button.update_animation(0.1));
    assert_eq!(button.dirty_bounds(frame), frame);
    assert_ne!(render_button(&button), before);

    button.sync_from(Button::new("Save"));
    assert!(!button.update_animation(0.1));
    assert_eq!(button.dirty_bounds(frame), Rect::zero());
}

#[test]
fn icon_button_is_square_and_uses_icon_as_accessible_name() {
    let button = Button::icon("settings");
    let size = button.measure(Constraints::unconstrained());
    assert_eq!(size.w, size.h);

    let accessibility = button.snapshot_fields().accessibility();
    assert_eq!(accessibility.name.as_deref(), Some("settings"));
}

#[test]
fn button_group_marks_connected_positions_and_builds_one_row() {
    let buttons = ButtonGroup::new()
        .buttons(vec![Button::new("A"), Button::new("B"), Button::new("C")])
        .into_positioned_buttons();
    let positions = buttons
        .iter()
        .map(|button| match button.snapshot_fields() {
            SnapshotFields::Button { group_position, .. } => group_position,
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        positions,
        vec![
            Some(ButtonGroupPosition::Left),
            Some(ButtonGroupPosition::Middle),
            Some(ButtonGroupPosition::Right),
        ]
    );
    assert!(buttons[0].resolve_style().border_width.left > 0.0);
    assert_eq!(buttons[1].resolve_style().border_width.left, 0.0);
    assert_eq!(buttons[2].resolve_style().border_width.left, 0.0);

    let node = ButtonGroup::new()
        .buttons(vec![Button::new("A"), Button::new("B")])
        .build();
    assert_eq!(node.children.len(), 2);
}

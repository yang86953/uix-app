use crate::tests::common::*;
use crate::ui::widgets::general::button::*;

#[test]
fn measure_clamps_button_size() {
    let measured = Button::new("abcdef").measure(Constraints::loose(Size::new(40.0, 24.0)));

    assert_eq!(measured, Size::new(40.0, 24.0));
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

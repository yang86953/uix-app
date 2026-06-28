//! uix-ui animation 模块集成测试。

use uix_ui::api::{Animation, Easing};
use uix_ui::api::Animatable;
use uix_graphics::color::Color;
use uix_platform::geometry::Rect;

// ════════════════════════════════════════════════════════════════════════════
// animation::core 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn animation_linear_progress() {
    let mut anim = Animation::new(0.0f32, 100.0, 1.0).with_easing(Easing::Linear);
    assert!((anim.current_value() - 0.0).abs() < 1e-6);
    anim.update(0.5);
    assert!((anim.current_value() - 50.0).abs() < 0.1);
    anim.update(0.5);
    assert!((anim.current_value() - 100.0).abs() < 0.1);
    assert!(anim.is_finished());
}

#[test]
fn animation_color_transition() {
    let red = Color::from_rgb(255, 0, 0);
    let blue = Color::from_rgb(0, 0, 255);
    let mut anim = Animation::new(red, blue, 1.0).with_easing(Easing::Linear);
    let mid = anim.update(0.5);
    assert_eq!(mid.r, 128);
    assert_eq!(mid.b, 128);
}

#[test]
fn animation_custom_easing() {
    let anim =
        Animation::new(0.0f64, 1.0, 1.0).with_easing(Easing::CubicBezier(0.42, 0.0, 0.58, 1.0));
    assert!((anim.current_value() - 0.0).abs() < 1e-6);
}

#[test]
fn animation_reverse() {
    let mut anim = Animation::new(0.0f32, 100.0, 1.0);
    anim.update(0.3);
    anim.reverse();
    assert!((anim.from - 100.0).abs() < 1e-6);
    assert!((anim.to - 0.0).abs() < 1e-6);
    assert!((anim.elapsed - 0.0).abs() < 1e-6);
}

// ════════════════════════════════════════════════════════════════════════════
// animation::easing 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn easing_linear_maps_one_to_one() {
    let e = Easing::Linear;
    assert!((e.sample(0.0) - 0.0).abs() < 1e-6);
    assert!((e.sample(0.5) - 0.5).abs() < 1e-6);
    assert!((e.sample(1.0) - 1.0).abs() < 1e-6);
}

#[test]
fn easing_quad_in_out_symmetric() {
    let e = Easing::QuadInOut;
    let mid = e.sample(0.5);
    assert!((mid - 0.5).abs() < 0.01);
    assert!((e.sample(0.25) - (1.0 - e.sample(0.75))).abs() < 1e-6);
}

#[test]
fn easing_antd_default_in_range() {
    let e = Easing::antd_default();
    assert!((e.sample(0.0) - 0.0).abs() < 1e-6);
    assert!((e.sample(1.0) - 1.0).abs() < 1e-6);
    assert!(e.sample(0.3) < e.sample(0.5));
    assert!(e.sample(0.5) < e.sample(0.7));
}

#[test]
fn animatable_lerp_f32() {
    let v = f32::lerp(0.0, 100.0, 0.5);
    assert!((v - 50.0).abs() < 1e-6);
}

#[test]
fn animatable_lerp_color() {
    let a = Color::from_rgb(0, 0, 0);
    let b = Color::from_rgb(255, 255, 255);
    let mid = Color::lerp(a, b, 0.5);
    assert_eq!(mid.r, 128);
    assert_eq!(mid.g, 128);
    assert_eq!(mid.b, 128);
}

#[test]
fn animatable_lerp_rect() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(10.0, 20.0, 200.0, 300.0);
    let mid = Rect::lerp(a, b, 0.5);
    assert!((mid.x - 5.0).abs() < 1e-6);
    assert!((mid.y - 10.0).abs() < 1e-6);
    assert!((mid.w - 150.0).abs() < 1e-6);
    assert!((mid.h - 200.0).abs() < 1e-6);
}

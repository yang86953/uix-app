//! uix 集成测试（geometry 模块）。

use uix::core::geometry::{EdgeInsets, Point, Rect, Size};

// ════════════════════════════════════════════════════════════════════════════
// geometry 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn rect_contains_point_inside() {
    let r = Rect::new(10.0, 10.0, 100.0, 50.0);
    assert!(r.contains(Point::new(10.0, 10.0)));
    assert!(r.contains(Point::new(110.0, 60.0)));
    assert!(r.contains(Point::new(50.0, 30.0)));
}

#[test]
fn rect_contains_point_outside() {
    let r = Rect::new(10.0, 10.0, 100.0, 50.0);
    assert!(!r.contains(Point::new(9.0, 10.0)));
    assert!(!r.contains(Point::new(10.0, 9.0)));
    assert!(!r.contains(Point::new(111.0, 30.0)));
    assert!(!r.contains(Point::new(50.0, 61.0)));
}

#[test]
fn rect_intersect_overlapping() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(50.0, 50.0, 100.0, 100.0);
    let c = a.intersect(&b).unwrap();
    assert_eq!(c, Rect::new(50.0, 50.0, 50.0, 50.0));
}

#[test]
fn rect_intersect_non_overlapping() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(200.0, 200.0, 100.0, 100.0);
    assert!(a.intersect(&b).is_none());
}

#[test]
fn rect_intersect_contained() {
    let outer = Rect::new(0.0, 0.0, 100.0, 100.0);
    let inner = Rect::new(10.0, 10.0, 80.0, 80.0);
    assert_eq!(outer.intersect(&inner), Some(inner));
}

#[test]
fn rect_union_separate() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(200.0, 50.0, 100.0, 100.0);
    let u = a.union(&b);
    assert_eq!(u, Rect::new(0.0, 0.0, 300.0, 150.0));
}

#[test]
fn rect_union_overlapping() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(50.0, 50.0, 100.0, 100.0);
    let u = a.union(&b);
    assert_eq!(u, Rect::new(0.0, 0.0, 150.0, 150.0));
}

#[test]
fn point_zero() {
    assert_eq!(Point::zero(), Point::new(0.0, 0.0));
}

#[test]
fn size_infinite() {
    let s = Size::infinite();
    assert_eq!(s.w, f32::MAX);
    assert_eq!(s.h, f32::MAX);
}

// ════════════════════════════════════════════════════════════════════════════
// Point
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn point_new() {
    let p = Point::new(3.0, 4.0);
    assert_eq!(p.x, 3.0);
    assert_eq!(p.y, 4.0);
}

#[test]
fn point_midpoint() {
    let a = Point::new(1.0, 2.0);
    let b = Point::new(3.0, 6.0);
    let m = Point::midpoint(a, b);
    assert_eq!(m, Point::new(2.0, 4.0));
}

#[test]
fn point_midpoint_negative() {
    let a = Point::new(-10.0, -20.0);
    let b = Point::new(10.0, 20.0);
    let m = Point::midpoint(a, b);
    assert_eq!(m, Point::new(0.0, 0.0));
}

#[test]
fn point_midpoint_same_point() {
    let a = Point::new(5.0, 5.0);
    let m = Point::midpoint(a, a);
    assert_eq!(m, Point::new(5.0, 5.0));
}

#[test]
fn point_default() {
    let p = Point::default();
    assert_eq!(p, Point::zero());
    assert_eq!(p.x, 0.0);
    assert_eq!(p.y, 0.0);
}

// ════════════════════════════════════════════════════════════════════════════
// Size
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn size_new_normal_values() {
    let s = Size::new(100.0, 200.0);
    assert_eq!(s.w, 100.0);
    assert_eq!(s.h, 200.0);
}

#[test]
fn size_new_nan_w_clamped_to_zero() {
    let s = Size::new(f32::NAN, 50.0);
    assert_eq!(s.w, 0.0);
    assert_eq!(s.h, 50.0);
}

#[test]
fn size_new_nan_h_clamped_to_zero() {
    let s = Size::new(30.0, f32::NAN);
    assert_eq!(s.w, 30.0);
    assert_eq!(s.h, 0.0);
}

#[test]
fn size_new_nan_both_clamped_to_zero() {
    let s = Size::new(f32::NAN, f32::NAN);
    assert_eq!(s.w, 0.0);
    assert_eq!(s.h, 0.0);
}

#[test]
fn size_new_negative_values_preserved() {
    let s = Size::new(-10.0, -20.0);
    assert_eq!(s.w, -10.0);
    assert_eq!(s.h, -20.0);
}

#[test]
fn size_new_zero() {
    let s = Size::new(0.0, 0.0);
    assert_eq!(s.w, 0.0);
    assert_eq!(s.h, 0.0);
}

#[test]
fn size_zero() {
    let s = Size::zero();
    assert_eq!(s.w, 0.0);
    assert_eq!(s.h, 0.0);
}

#[test]
fn size_default() {
    let s = Size::default();
    assert_eq!(s, Size::zero());
}

#[test]
fn size_clone_copy() {
    let a = Size::new(10.0, 20.0);
    let b = a;
    assert_eq!(a, b);
}

// ════════════════════════════════════════════════════════════════════════════
// Rect
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn rect_new_normal_values() {
    let r = Rect::new(10.0, 20.0, 100.0, 50.0);
    assert_eq!(r.x, 10.0);
    assert_eq!(r.y, 20.0);
    assert_eq!(r.w, 100.0);
    assert_eq!(r.h, 50.0);
}

#[test]
fn rect_new_nan_x_clamped_to_zero() {
    let r = Rect::new(f32::NAN, 1.0, 2.0, 3.0);
    assert_eq!(r.x, 0.0);
    assert_eq!(r.y, 1.0);
    assert_eq!(r.w, 2.0);
    assert_eq!(r.h, 3.0);
}

#[test]
fn rect_new_nan_y_clamped_to_zero() {
    let r = Rect::new(1.0, f32::NAN, 2.0, 3.0);
    assert_eq!(r.x, 1.0);
    assert_eq!(r.y, 0.0);
    assert_eq!(r.w, 2.0);
    assert_eq!(r.h, 3.0);
}

#[test]
fn rect_new_nan_w_clamped_to_zero() {
    let r = Rect::new(1.0, 2.0, f32::NAN, 4.0);
    assert_eq!(r.w, 0.0);
    assert_eq!(r.h, 4.0);
}

#[test]
fn rect_new_nan_h_clamped_to_zero() {
    let r = Rect::new(1.0, 2.0, 3.0, f32::NAN);
    assert_eq!(r.w, 3.0);
    assert_eq!(r.h, 0.0);
}

#[test]
fn rect_new_nan_all_clamped_to_zero() {
    let r = Rect::new(f32::NAN, f32::NAN, f32::NAN, f32::NAN);
    assert_eq!(r, Rect::zero());
}

#[test]
fn rect_new_negative_values_preserved() {
    let r = Rect::new(-10.0, -20.0, -30.0, -40.0);
    assert_eq!(r.x, -10.0);
    assert_eq!(r.y, -20.0);
    assert_eq!(r.w, -30.0);
    assert_eq!(r.h, -40.0);
}

#[test]
fn rect_zero() {
    let r = Rect::zero();
    assert_eq!(r, Rect::new(0.0, 0.0, 0.0, 0.0));
}

#[test]
fn rect_default() {
    let r = Rect::default();
    assert_eq!(r, Rect::zero());
}

#[test]
fn rect_clone_copy() {
    let a = Rect::new(1.0, 2.0, 3.0, 4.0);
    let b = a;
    assert_eq!(a, b);
}

// ════════════════════════════════════════════════════════════════════════════
// Rect::inset
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn rect_inset_uniform() {
    let r = Rect::new(10.0, 10.0, 100.0, 50.0);
    let i = r.inset(EdgeInsets::uniform(5.0));
    assert_eq!(i, Rect::new(15.0, 15.0, 90.0, 40.0));
}

#[test]
fn rect_inset_asymmetric() {
    let r = Rect::new(10.0, 10.0, 100.0, 50.0);
    let i = r.inset(EdgeInsets::new(5.0, 10.0, 15.0, 20.0));
    assert_eq!(i, Rect::new(15.0, 20.0, 80.0, 20.0));
}

#[test]
fn rect_inset_larger_than_rect() {
    let r = Rect::new(0.0, 0.0, 10.0, 10.0);
    let i = r.inset(EdgeInsets::uniform(20.0));
    assert_eq!(i, Rect::new(20.0, 20.0, 0.0, 0.0));
}

#[test]
fn rect_inset_zero() {
    let r = Rect::new(10.0, 10.0, 100.0, 50.0);
    let i = r.inset(EdgeInsets::zero());
    assert_eq!(i, r);
}

#[test]
fn rect_inset_negative_expands() {
    let r = Rect::new(10.0, 10.0, 100.0, 50.0);
    let i = r.inset(EdgeInsets::uniform(-5.0));
    assert_eq!(i, Rect::new(5.0, 5.0, 110.0, 60.0));
}

#[test]
fn rect_inset_only_left() {
    let r = Rect::new(10.0, 10.0, 100.0, 50.0);
    let i = r.inset(EdgeInsets::new(5.0, 0.0, 0.0, 0.0));
    assert_eq!(i, Rect::new(15.0, 10.0, 95.0, 50.0));
}

#[test]
fn rect_inset_only_top() {
    let r = Rect::new(10.0, 10.0, 100.0, 50.0);
    let i = r.inset(EdgeInsets::new(0.0, 5.0, 0.0, 0.0));
    assert_eq!(i, Rect::new(10.0, 15.0, 100.0, 45.0));
}

#[test]
fn rect_inset_exact_size_shrinks_to_zero() {
    let r = Rect::new(0.0, 0.0, 10.0, 10.0);
    let i = r.inset(EdgeInsets::new(5.0, 5.0, 5.0, 5.0));
    assert_eq!(i, Rect::new(5.0, 5.0, 0.0, 0.0));
}

#[test]
fn rect_inset_preserves_position_when_zero_inset() {
    let r = Rect::new(100.0, 200.0, 300.0, 400.0);
    let i = r.inset(EdgeInsets::zero());
    assert_eq!(i, r);
}

// ════════════════════════════════════════════════════════════════════════════
// EdgeInsets
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn edge_insets_new() {
    let e = EdgeInsets::new(1.0, 2.0, 3.0, 4.0);
    assert_eq!(e.left, 1.0);
    assert_eq!(e.top, 2.0);
    assert_eq!(e.right, 3.0);
    assert_eq!(e.bottom, 4.0);
}

#[test]
fn edge_insets_uniform() {
    let e = EdgeInsets::uniform(5.0);
    assert_eq!(e.left, 5.0);
    assert_eq!(e.top, 5.0);
    assert_eq!(e.right, 5.0);
    assert_eq!(e.bottom, 5.0);
}

#[test]
fn edge_insets_uniform_zero() {
    let e = EdgeInsets::uniform(0.0);
    assert_eq!(e, EdgeInsets::zero());
}

#[test]
fn edge_insets_zero() {
    let e = EdgeInsets::zero();
    assert_eq!(e.left, 0.0);
    assert_eq!(e.top, 0.0);
    assert_eq!(e.right, 0.0);
    assert_eq!(e.bottom, 0.0);
}

#[test]
fn edge_insets_horizontal() {
    let e = EdgeInsets::new(1.0, 2.0, 3.0, 4.0);
    assert_eq!(e.horizontal(), 4.0);
}

#[test]
fn edge_insets_horizontal_symmetric() {
    let e = EdgeInsets::new(5.0, 0.0, 5.0, 0.0);
    assert_eq!(e.horizontal(), 10.0);
}

#[test]
fn edge_insets_vertical() {
    let e = EdgeInsets::new(1.0, 2.0, 3.0, 4.0);
    assert_eq!(e.vertical(), 6.0);
}

#[test]
fn edge_insets_vertical_symmetric() {
    let e = EdgeInsets::new(0.0, 5.0, 0.0, 5.0);
    assert_eq!(e.vertical(), 10.0);
}

#[test]
fn edge_insets_default() {
    let e = EdgeInsets::default();
    assert_eq!(e, EdgeInsets::zero());
}

#[test]
fn edge_insets_clone_copy() {
    let a = EdgeInsets::new(1.0, 2.0, 3.0, 4.0);
    let b = a;
    assert_eq!(a, b);
}

// ════════════════════════════════════════════════════════════════════════════
// 边界条件
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn rect_zero_size_contains_origin() {
    let r = Rect::new(0.0, 0.0, 0.0, 0.0);
    assert!(r.contains(Point::new(0.0, 0.0)));
    assert!(!r.contains(Point::new(0.1, 0.0)));
}

#[test]
fn rect_zero_size_intersect_returns_none() {
    let a = Rect::new(0.0, 0.0, 0.0, 0.0);
    let b = Rect::new(10.0, 10.0, 100.0, 100.0);
    assert!(a.intersect(&b).is_none());
}

#[test]
fn rect_negative_size_intersect() {
    let a = Rect::new(0.0, 0.0, -10.0, -10.0);
    let b = Rect::new(-5.0, -5.0, 10.0, 10.0);
    assert!(a.intersect(&b).is_none());
}

#[test]
fn point_equality() {
    let a = Point::new(1.0, 2.0);
    let b = Point::new(1.0, 2.0);
    assert_eq!(a, b);
}

#[test]
fn point_inequality() {
    let a = Point::new(1.0, 2.0);
    let b = Point::new(1.0, 3.0);
    assert_ne!(a, b);
}

#[test]
fn size_equality() {
    let a = Size::new(10.0, 20.0);
    let b = Size::new(10.0, 20.0);
    assert_eq!(a, b);
}

#[test]
fn rect_union_with_negative_position() {
    let a = Rect::new(-100.0, -100.0, 50.0, 50.0);
    let b = Rect::new(0.0, 0.0, 100.0, 100.0);
    let u = a.union(&b);
    assert_eq!(u, Rect::new(-100.0, -100.0, 200.0, 200.0));
}

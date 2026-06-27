//! uix-platform crate 集成测试。

use uix::platform::geometry::{Point, Rect, Size};

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

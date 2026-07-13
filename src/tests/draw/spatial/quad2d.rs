use crate::tests::common::*;
use std::ops::{Add, Mul, Sub};
use crate::draw::spatial::quad2d::*;

#[test]
fn bounds_axis_aligned() {
    let q = Quad2D::new(
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(10.0, 5.0),
        Vec2::new(0.0, 5.0),
    );
    let b = q.bounds();
    assert!((b.x - 0.0).abs() < 1e-10);
    assert!((b.y - 0.0).abs() < 1e-10);
    assert!((b.w - 10.0).abs() < 1e-10);
    assert!((b.h - 5.0).abs() < 1e-10);
}

#[test]
fn bounds_rotated() {
    // 旋转 45 度的正方形投影
    let q = Quad2D::new(
        Vec2::new(5.0, 0.0),
        Vec2::new(10.0, 5.0),
        Vec2::new(5.0, 10.0),
        Vec2::new(0.0, 5.0),
    );
    let b = q.bounds();
    assert!((b.x - 0.0).abs() < 1e-10);
    assert!((b.y - 0.0).abs() < 1e-10);
    assert!((b.w - 10.0).abs() < 1e-10);
    assert!((b.h - 10.0).abs() < 1e-10);
}

#[test]
fn contains_inside() {
    let q = Quad2D::new(
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(10.0, 10.0),
        Vec2::new(0.0, 10.0),
    );
    assert!(q.contains(Vec2::new(5.0, 5.0)));
}

#[test]
fn contains_outside() {
    let q = Quad2D::new(
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(10.0, 10.0),
        Vec2::new(0.0, 10.0),
    );
    assert!(!q.contains(Vec2::new(15.0, 5.0)));
}

#[test]
fn contains_trapezoid() {
    // 梯形（透视投影的常见形状）
    let q = Quad2D::new(
        Vec2::new(2.0, 0.0),
        Vec2::new(8.0, 0.0),
        Vec2::new(10.0, 10.0),
        Vec2::new(0.0, 10.0),
    );
    assert!(q.contains(Vec2::new(5.0, 5.0)));
    assert!(!q.contains(Vec2::new(0.0, 0.0))); // 在梯形左上方之外
}

#[test]
fn to_path_has_four_points() {
    let q = Quad2D::new(
        Vec2::new(0.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(0.0, 1.0),
    );
    let path = q.to_path();
    assert_eq!(path.len(), 4);
}

#[test]
fn from_points() {
    let pts = vec![
        Vec2::new(5.0, 10.0),
        Vec2::new(20.0, 3.0),
        Vec2::new(15.0, 25.0),
        Vec2::new(1.0, 8.0),
    ];
    let q = Quad2D::from_points(&pts);
    let b = q.bounds();
    assert!((b.x - 1.0).abs() < 1e-10);
    assert!((b.y - 3.0).abs() < 1e-10);
    assert!((b.w - 19.0).abs() < 1e-10);
    assert!((b.h - 22.0).abs() < 1e-10);
}

// ── Vec2 运算符与向量方法 ──

#[test]
fn vec2_add_sub_mul() {
    let a = Vec2::new(1.0, 2.0);
    let b = Vec2::new(10.0, 20.0);
    assert_eq!(a + b, Vec2::new(11.0, 22.0));
    assert_eq!(b - a, Vec2::new(9.0, 18.0));
    assert_eq!(a * 2.0, Vec2::new(2.0, 4.0));
    assert_eq!(2.0 * a, Vec2::new(2.0, 4.0));
}

#[test]
fn vec2_dot_length_normalized() {
    let a = Vec2::new(3.0, 4.0);
    assert!((a.length() - 5.0).abs() < 1e-6);
    assert!((a.dot(&Vec2::new(1.0, 0.0)) - 3.0).abs() < 1e-10);
    let n = a.normalized();
    assert!((n.length() - 1.0).abs() < 1e-6);
}

#[test]
fn vec2_normalized_zero_is_safe() {
    let n = Vec2::zero().normalized();
    assert_eq!(n, Vec2::zero());
}

#[test]
fn vec2_from_tuple() {
    let v: Vec2 = (1.5, 2.5).into();
    assert!((v.x - 1.5).abs() < 1e-10);
    assert!((v.y - 2.5).abs() < 1e-10);
}

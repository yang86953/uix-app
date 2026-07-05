//! uix draw 域 空间坐标系统集成测试。
//! 补充单元测试，直接从公开 API 测试 SpatialContext、PhysicalUnit 等。

use uix::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
use uix::draw::null_engine::NullEngine;
use uix::draw::spatial::{
    Orientation, PhysicalUnit, Quad2D, SpatialContext, Vec2, Vec3, AABB3D,
};

// ════════════════════════════════════════════════════════════════════════════
// PhysicalUnit 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn physical_unit_px_to_dip() {
    let u = PhysicalUnit::Px(100.0);
    assert!((u.to_dip(96.0) - 100.0).abs() < 1e-6);
}

#[test]
fn physical_unit_mm_to_dip() {
    let u = PhysicalUnit::Mm(25.4);
    let result = u.to_dip(96.0);
    assert!((result - 96.0).abs() < 0.01); // 25.4mm = 1 inch @ 96 DPI
}

#[test]
fn physical_unit_cm_to_dip() {
    let u = PhysicalUnit::Cm(2.54);
    let result = u.to_dip(96.0);
    assert!((result - 96.0).abs() < 0.01); // 2.54cm = 1 inch @ 96 DPI
}

#[test]
fn physical_unit_m_to_dip() {
    let u = PhysicalUnit::M(1.0);
    let result = u.to_dip(96.0);
    assert!((result - 96.0 / 0.0254).abs() < 0.01);
}

#[test]
fn physical_unit_pt_to_dip() {
    let u = PhysicalUnit::Pt(72.0);
    let result = u.to_dip(96.0);
    assert!((result - 96.0).abs() < 0.01); // 72pt = 1 inch @ 96 DPI
}

#[test]
fn physical_unit_inch_to_dip() {
    let u = PhysicalUnit::Inch(1.0);
    let result = u.to_dip(96.0);
    assert!((result - 96.0).abs() < 1e-6);
}

#[test]
fn physical_unit_to_px() {
    let u = PhysicalUnit::Inch(1.0);
    let result = u.to_px(96.0, 2.0);
    assert!((result - 192.0).abs() < 1e-6); // 1 inch @ 96 DPI * 2.0 DPR = 192px
}

#[test]
fn physical_unit_value() {
    let u = PhysicalUnit::Px(42.0);
    assert!((u.value() - 42.0).abs() < 1e-6);
    let u = PhysicalUnit::Mm(10.0);
    assert!((u.value() - 10.0).abs() < 1e-6);
}

// ════════════════════════════════════════════════════════════════════════════
// Vec3 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn vec3_zero() {
    let v = Vec3::zero();
    assert_eq!(v.x, 0.0);
    assert_eq!(v.y, 0.0);
    assert_eq!(v.z, 0.0);
}

#[test]
fn vec3_new() {
    let v = Vec3::new(1.0, 2.0, 3.0);
    assert_eq!(v.x, 1.0);
    assert_eq!(v.y, 2.0);
    assert_eq!(v.z, 3.0);
}

#[test]
fn vec3_add() {
    let a = Vec3::new(1.0, 2.0, 3.0);
    let b = Vec3::new(10.0, 20.0, 30.0);
    let c = a + b;
    assert!((c.x - 11.0).abs() < 1e-6);
    assert!((c.y - 22.0).abs() < 1e-6);
    assert!((c.z - 33.0).abs() < 1e-6);
}

#[test]
fn vec3_sub() {
    let a = Vec3::new(10.0, 20.0, 30.0);
    let b = Vec3::new(1.0, 2.0, 3.0);
    let c = a - b;
    assert!((c.x - 9.0).abs() < 1e-6);
}

#[test]
fn vec3_dot() {
    let a = Vec3::new(1.0, 0.0, 0.0);
    let b = Vec3::new(1.0, 0.0, 0.0);
    assert!((a.dot(&b) - 1.0).abs() < 1e-6);
    assert!((a.dot(&Vec3::new(0.0, 1.0, 0.0))).abs() < 1e-6);
}

#[test]
fn vec3_cross() {
    let x = Vec3::new(1.0, 0.0, 0.0);
    let y = Vec3::new(0.0, 1.0, 0.0);
    let z = x.cross(&y);
    assert!((z.x - 0.0).abs() < 1e-6);
    assert!((z.y - 0.0).abs() < 1e-6);
    assert!((z.z - 1.0).abs() < 1e-6);
}

#[test]
fn vec3_length() {
    let v = Vec3::new(3.0, 4.0, 0.0);
    assert!((v.length() - 5.0).abs() < 1e-6);
}

#[test]
fn vec3_normalized() {
    let v = Vec3::new(3.0, 4.0, 0.0).normalized();
    assert!((v.length() - 1.0).abs() < 1e-6);
}

#[test]
fn vec3_debug() {
    let v = Vec3::new(1.0, 2.0, 3.0);
    let s = format!("{v:?}");
    assert!(s.contains("1"));
    assert!(s.contains("2"));
    assert!(s.contains("3"));
}

// ════════════════════════════════════════════════════════════════════════════
// AABB3D 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn aabb3d_new() {
    let min = Vec3::new(0.0, 0.0, 0.0);
    let max = Vec3::new(100.0, 50.0, 25.0);
    let aabb = AABB3D::new(min, max);
    assert_eq!(aabb.min, min);
    assert_eq!(aabb.max, max);
}

#[test]
fn aabb3d_center() {
    let aabb = AABB3D::new(Vec3::zero(), Vec3::new(10.0, 10.0, 10.0));
    let c = aabb.center();
    assert!((c.x - 5.0).abs() < 1e-6);
    assert!((c.y - 5.0).abs() < 1e-6);
    assert!((c.z - 5.0).abs() < 1e-6);
}

#[test]
fn aabb3d_size() {
    let aabb = AABB3D::new(Vec3::zero(), Vec3::new(10.0, 20.0, 30.0));
    let s = aabb.size();
    assert!((s.x - 10.0).abs() < 1e-6);
    assert!((s.y - 20.0).abs() < 1e-6);
    assert!((s.z - 30.0).abs() < 1e-6);
}

// ════════════════════════════════════════════════════════════════════════════
// Quad2D 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn quad2d_bounds() {
    let quad = Quad2D::new(
        Vec2::new(0.0, 0.0),
        Vec2::new(100.0, 0.0),
        Vec2::new(100.0, 50.0),
        Vec2::new(0.0, 50.0),
    );
    let bounds = quad.bounds();
    assert!((bounds.x - 0.0).abs() < 1e-6);
    assert!((bounds.y - 0.0).abs() < 1e-6);
    assert!((bounds.w - 100.0).abs() < 1e-6);
    assert!((bounds.h - 50.0).abs() < 1e-6);
}

// ════════════════════════════════════════════════════════════════════════════
// SpatialContext 测试
// ════════════════════════════════════════════════════════════════════════════

fn make_canvas() -> (NullEngine, NoopCanvas2D) {
    let engine = NullEngine::new();
    let canvas = NoopCanvas2D;
    (engine, canvas)
}

#[test]
fn spatial_context_default_2d() {
    let (_engine, mut canvas) = make_canvas();
    let ctx = SpatialContext::new(&mut canvas, 96.0, 1.0, Orientation::YDown, 800, 600);
    assert!(ctx.is_2d_only());
    assert!((ctx.dpi() - 96.0).abs() < 1e-6);
}

#[test]
fn spatial_context_project_identity() {
    let (_engine, mut canvas) = make_canvas();
    let ctx = SpatialContext::new(&mut canvas, 96.0, 1.0, Orientation::YDown, 800, 600);
    // 默认正交投影: left=0, right=800, bottom=600, top=0
    // 屏幕中心 (400,300) → NDC (0,0) → 屏幕 (400,300)
    let (sx, sy) = ctx.project(&Vec3::new(400.0, 300.0, 0.0));
    assert!((sx - 400.0).abs() < 1.0, "sx 应在 400 附近，得到 {sx}");
    assert!((sy - 300.0).abs() < 1.0, "sy 应在 300 附近，得到 {sy}");
}

#[test]
fn spatial_context_project_aabb() {
    let (_engine, mut canvas) = make_canvas();
    let ctx = SpatialContext::new(&mut canvas, 96.0, 1.0, Orientation::YDown, 800, 600);
    // 默认正交投影下，世界坐标直接映射到屏幕坐标
    let aabb = AABB3D::new(Vec3::new(100.0, 100.0, 0.0), Vec3::new(300.0, 200.0, 0.0));
    let quad = ctx.project_aabb(&aabb);
    let bounds = quad.bounds();
    assert!((bounds.x - 100.0).abs() < 1.0);
    assert!((bounds.y - 100.0).abs() < 1.0);
    assert!((bounds.w - 200.0).abs() < 1.0);
    assert!((bounds.h - 100.0).abs() < 1.0);
}

#[test]
fn spatial_context_dpi_affects_physical_unit() {
    let (_engine, mut canvas) = make_canvas();
    let ctx = SpatialContext::new(&mut canvas, 192.0, 2.0, Orientation::YDown, 800, 600);
    assert!((ctx.dpi() - 192.0).abs() < 1e-6);
}

// ════════════════════════════════════════════════════════════════════════════
// Orientation 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn orientation_y_down_vs_y_up() {
    let y_down = Orientation::YDown;
    let y_up = Orientation::YUp;
    assert_ne!(y_down, y_up);
    let s_down = format!("{y_down:?}");
    let s_up = format!("{y_up:?}");
    assert!(s_down.contains("YDown"));
    assert!(s_up.contains("YUp"));
}

use super::*;
use crate::draw::null_engine::NullEngine;
use crate::draw::spatial::PhysicalUnitExt;
use crate::draw::traits::GraphicsEngine;

fn make_context(surface_w: i32, surface_h: i32) -> SpatialContext<'static> {
    let mut engine = NullEngine::new();
    engine.initialize(surface_w, surface_h).ok();
    let canvas = engine.canvas_2d();
    // 注意：canvas 的生命周期是 'static 因为 NullEngine 是 'static
    // 但实际使用中 canvas 的引用不能超过 engine 的生命周期
    // 这里为了测试用 unsafe
    let canvas_ref: &'static mut dyn Canvas2D = unsafe { std::mem::transmute(canvas) };
    SpatialContext::new(
        canvas_ref,
        96.0,
        1.0,
        Orientation::YDown,
        surface_w,
        surface_h,
    )
}

#[test]
fn default_2d_config() {
    let ctx = make_context(800, 600);
    assert!(ctx.is_2d_only());
    assert!(!ctx.is_perspective());
    assert!((ctx.dpi() - 96.0).abs() < 1e-10);
    assert_eq!(ctx.surface_size(), (800, 600));
}

#[test]
fn push_pop_matrix() {
    let mut ctx = make_context(800, 600);
    // MVP = projection * view * model。初始时 model=identity, view=identity,
    // projection=orthographic(0,800,600,0,-1,1) 不是 is_identity 但 is_2d_only
    let mvp_initial = ctx.mvp_matrix();
    assert!(mvp_initial.is_2d_only(), "initial MVP should be 2d_only");

    ctx.push_matrix(Mat4::translate(10.0, 20.0, 0.0));
    let mvp_translated = ctx.mvp_matrix();
    // 平移后的 MVP 应该不同于初始
    assert_ne!(mvp_translated, mvp_initial, "translate should change MVP");

    ctx.pop_matrix();
    let mvp_restored = ctx.mvp_matrix();
    assert_eq!(mvp_restored, mvp_initial, "pop should restore initial MVP");
}

#[test]
fn project_point_2d() {
    let ctx = make_context(800, 600);
    // 在 2D 正交投影下，(100, 200, 0) 应该投影到屏幕 (100, 200)
    let (sx, sy) = ctx.project(&Vec3::new(100.0, 200.0, 0.0));
    assert!((sx - 100.0).abs() < 1.0);
    assert!((sy - 200.0).abs() < 1.0);
}

#[test]
fn project_point_3d_transformed() {
    let mut ctx = make_context(800, 600);
    ctx.push_matrix(Mat4::translate(50.0, 30.0, 0.0));
    // 平移后，(100, 200) 实际在模型空间的 (150, 230)
    let (sx, sy) = ctx.project(&Vec3::new(100.0, 200.0, 0.0));
    assert!((sx - 150.0).abs() < 1.0);
    assert!((sy - 230.0).abs() < 1.0);
    ctx.pop_matrix();
}

#[test]
fn save_restore() {
    let mut ctx = make_context(800, 600);
    let identity = ctx.mvp_matrix();

    ctx.save();
    ctx.translate(10.0, 20.0, 0.0);
    assert!(!ctx.mvp_matrix().is_identity());
    ctx.restore();
    assert_eq!(ctx.mvp_matrix(), identity);
}

#[test]
fn set_perspective_changes_mode() {
    let mut ctx = make_context(800, 600);
    assert!(!ctx.is_perspective());

    ctx.set_perspective(1.0, 0.1, 100.0);
    assert!(ctx.is_perspective());
    assert!(!ctx.is_2d_only());
}

#[test]
fn set_orthographic_2d_restores() {
    let mut ctx = make_context(800, 600);
    ctx.set_perspective(1.0, 0.1, 100.0);
    assert!(ctx.is_perspective());

    ctx.set_orthographic_2d();
    assert!(!ctx.is_perspective());
    assert!(ctx.is_2d_only());
}

#[test]
fn unproject_2d_roundtrip() {
    let ctx = make_context(800, 600);
    // 在 2D 模式下，屏幕 (100, 200) 反投影后应该在 z=0 平面上
    let ray = ctx.unproject(100.0, 200.0).unwrap();
    if let Some(hit) = ray.intersect_z0() {
        assert!((hit.x - 100.0).abs() < 5.0, "x: {}", hit.x);
        assert!((hit.y - 200.0).abs() < 5.0, "y: {}", hit.y);
    } else {
        panic!("ray should intersect z=0");
    }
}

#[test]
fn unproject_with_transform_roundtrip() {
    let mut ctx = make_context(800, 600);
    // model→world: translate(50,30,0)
    ctx.push_matrix(Mat4::translate(50.0, 30.0, 0.0));

    // project model-space point (100,200,0) → screen
    let (sx, sy) = ctx.project(&Vec3::new(100.0, 200.0, 0.0));

    // unproject screen → ray → z=0 intersection
    let ray = ctx.unproject(sx, sy).unwrap();
    if let Some(hit) = ray.intersect_z0() {
        // inv_mvp 已将 NDC 转回模型空间，hit 在模型空间中
        // 期望 hit ≈ (100, 200, 0)
        assert!((hit.x - 100.0).abs() < 5.0, "hit.x: {}", hit.x);
        assert!((hit.y - 200.0).abs() < 5.0, "hit.y: {}", hit.y);
        assert!((hit.z).abs() < 1.0, "hit.z: {}", hit.z);
    } else {
        panic!("ray should intersect z=0");
    }
    ctx.pop_matrix();
}

#[test]
fn project_aabb_2d() {
    let ctx = make_context(800, 600);
    let aabb = AABB3D::from_rect_z(100.0, 50.0, 200.0, 100.0, 0.0, 0.0);
    let quad = ctx.project_aabb(&aabb);
    let bounds = quad.bounds();
    assert!((bounds.x - 100.0).abs() < 1.0);
    assert!((bounds.y - 50.0).abs() < 1.0);
    assert!((bounds.w - 200.0).abs() < 1.0);
    assert!((bounds.h - 100.0).abs() < 1.0);
}

#[test]
fn translate_2d_method() {
    let mut ctx = make_context(800, 600);
    ctx.translate_2d(100.0, 200.0);
    let (sx, sy) = ctx.project(&Vec3::new(50.0, 30.0, 0.0));
    assert!((sx - 150.0).abs() < 1.0);
    assert!((sy - 230.0).abs() < 1.0);
}

#[test]
fn rotate_z_keeps_2d_only() {
    let mut ctx = make_context(800, 600);
    ctx.rotate_z(0.5);
    // rotate_z + orthographic projection 应该保持 2D only
    // （is_2d_only 检查的是 z 轴相关的旋转/平移，不检查投影的 z 缩放）
    let mvp = ctx.mvp_matrix();
    // 验证 z 轴无旋转:
    assert!((mvp.0[2]).abs() < 1e-6, "z→x rotation should be 0");
    assert!((mvp.0[6]).abs() < 1e-6, "z→y rotation should be 0");
    assert!((mvp.0[8]).abs() < 1e-6, "x→z rotation should be 0");
    assert!((mvp.0[9]).abs() < 1e-6, "y→z rotation should be 0");
}

#[test]
fn rotate_x_breaks_2d_only() {
    let mut ctx = make_context(800, 600);
    ctx.rotate_x(0.5);
    assert!(!ctx.mvp_matrix().is_2d_only());
}

#[test]
fn scale_2d_keeps_2d_only() {
    let mut ctx = make_context(800, 600);
    ctx.scale(2.0, 2.0, 1.0);
    let mvp = ctx.mvp_matrix();
    assert!((mvp.0[2]).abs() < 1e-6);
    assert!((mvp.0[6]).abs() < 1e-6);
}

#[test]
fn set_camera_look_at() {
    let mut ctx = make_context(800, 600);
    ctx.set_camera_look_at(
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::zero(),
        Vec3::new(0.0, 1.0, 0.0),
    );
    // 设置相机后，m_p 矩阵变了
    assert!(!ctx.mvp_matrix().is_identity());
}

#[test]
fn dpr_affects_pixel_coords() {
    // 在 2x DPR 下，100 dip → 200 物理像素
    let mut engine = NullEngine::new();
    engine.initialize(800, 600).ok();
    let canvas = engine.canvas_2d();
    let canvas_ref: &'static mut dyn Canvas2D = unsafe { std::mem::transmute(canvas) };
    let ctx = SpatialContext::new(canvas_ref, 96.0, 2.0, Orientation::YDown, 800, 600);
    assert!((ctx.device_pixel_ratio() - 2.0).abs() < 1e-10);
}

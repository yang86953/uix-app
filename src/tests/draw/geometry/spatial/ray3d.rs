use crate::draw::geometry::spatial::ray3d::*;
use crate::tests::common::*;

#[test]
fn ray_intersects_aabb() {
    let ray = Ray3D::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
    let aabb = AABB3D::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
    assert!(ray.intersects_aabb(&aabb));
}

#[test]
fn ray_misses_aabb() {
    let ray = Ray3D::new(Vec3::new(10.0, 10.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
    let aabb = AABB3D::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
    assert!(!ray.intersects_aabb(&aabb));
}

#[test]
fn ray_intersects_z0_plane() {
    let ray = Ray3D::new(Vec3::new(10.0, 20.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
    let hit = ray.intersect_z0().unwrap();
    assert!((hit.x - 10.0).abs() < 1e-6);
    assert!((hit.y - 20.0).abs() < 1e-6);
    assert!((hit.z).abs() < 1e-6);
}

#[test]
fn ray_parallel_to_z0() {
    let ray = Ray3D::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(1.0, 0.0, 0.0));
    assert!(ray.intersect_z0().is_none());
}

#[test]
fn ray_behind_z0() {
    let ray = Ray3D::new(Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 0.0, -1.0));
    // 射线从 z=-5 向 -z 方向，不会碰到 z=0
    assert!(ray.intersect_z0().is_none());
}

#[test]
fn ray_at_parameter() {
    let ray = Ray3D::new(Vec3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 0.0, 0.0));
    let p = ray.at(5.0);
    assert!((p.x - 6.0).abs() < 1e-6);
    assert!((p.y - 2.0).abs() < 1e-6);
    assert!((p.z - 3.0).abs() < 1e-6);
}

#[test]
fn ray_aabb_edge_case_origin_inside() {
    let ray = Ray3D::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
    let aabb = AABB3D::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
    assert!(ray.intersects_aabb(&aabb));
}

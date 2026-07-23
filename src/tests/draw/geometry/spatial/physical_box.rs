use crate::draw::geometry::spatial::physical_box::*;
use crate::draw::geometry::spatial::PhysicalUnitExt;
use crate::tests::common::*;

#[test]
fn rect_into_aabb() {
    let rect = crate::core::Rect::new(10.0, 20.0, 100.0, 50.0);
    let aabb = rect.into_aabb(96.0, 1.0);
    assert!((aabb.min.x - 10.0).abs() < 1e-10);
    assert!((aabb.min.y - 20.0).abs() < 1e-10);
    assert!((aabb.min.z).abs() < 1e-10);
    assert!((aabb.max.x - 110.0).abs() < 1e-10);
    assert!((aabb.max.y - 70.0).abs() < 1e-10);
}

#[test]
fn aabb3d_into_aabb_identity() {
    let aabb = AABB3D::new(
        crate::draw::geometry::spatial::Vec3::new(1.0, 2.0, 3.0),
        crate::draw::geometry::spatial::Vec3::new(4.0, 5.0, 6.0),
    );
    let result = aabb.into_aabb(96.0, 1.0);
    assert!((result.min.x - 1.0).abs() < 1e-10);
    assert!((result.max.x - 4.0).abs() < 1e-10);
}

#[test]
fn physical_box_into_aabb() {
    let pb = PhysicalBox::new_2d(10.mm(), 5.mm(), 5.cm(), 3.cm());
    // 10mm @ 96dpi = 37.8px, 5mm = 18.9px
    // 5cm = 50mm = 189px, 3cm = 30mm = 113.4px
    let aabb = pb.into_aabb(96.0, 1.0);
    assert!((aabb.min.x - 37.795_276).abs() < 1.0);
    assert!((aabb.min.y - 18.897_638).abs() < 0.5);
    assert!((aabb.max.x - (37.795_276 + 188.976_38)).abs() < 2.0);
}

use crate::draw::spatial::vec3::*;

#[test]
fn vec3_add() {
    let a = Vec3::new(1.0, 2.0, 3.0);
    let b = Vec3::new(4.0, 5.0, 6.0);
    assert_eq!(a + b, Vec3::new(5.0, 7.0, 9.0));
}

#[test]
fn vec3_sub() {
    let a = Vec3::new(1.0, 2.0, 3.0);
    let b = Vec3::new(4.0, 5.0, 6.0);
    assert_eq!(a - b, Vec3::new(-3.0, -3.0, -3.0));
}

#[test]
fn vec3_mul_scalar() {
    let v = Vec3::new(1.0, 2.0, 3.0);
    assert_eq!(v * 2.0, Vec3::new(2.0, 4.0, 6.0));
    assert_eq!(2.0 * v, Vec3::new(2.0, 4.0, 6.0));
}

#[test]
fn vec3_dot() {
    let a = Vec3::new(1.0, 0.0, 0.0);
    let b = Vec3::new(0.0, 1.0, 0.0);
    assert!((a.dot(&b)).abs() < 1e-10);

    let c = Vec3::new(2.0, 3.0, 4.0);
    let d = Vec3::new(5.0, 6.0, 7.0);
    assert!((c.dot(&d) - 56.0).abs() < 1e-6);
}

#[test]
fn vec3_cross() {
    let x = Vec3::new(1.0, 0.0, 0.0);
    let y = Vec3::new(0.0, 1.0, 0.0);
    assert_eq!(x.cross(&y), Vec3::new(0.0, 0.0, 1.0));
    assert_eq!(y.cross(&x), Vec3::new(0.0, 0.0, -1.0));
}

#[test]
fn vec3_length() {
    let v = Vec3::new(3.0, 4.0, 0.0);
    assert!((v.length() - 5.0).abs() < 1e-6);
}

#[test]
fn vec3_normalized() {
    let v = Vec3::new(0.0, 5.0, 0.0);
    let n = v.normalized();
    assert!((n.x).abs() < 1e-10);
    assert!((n.y - 1.0).abs() < 1e-10);
    assert!((n.z).abs() < 1e-10);
}

#[test]
fn vec3_normalized_zero() {
    let v = Vec3::zero();
    let n = v.normalized();
    assert_eq!(n, Vec3::zero());
}

#[test]
fn vec4_to_vec3_homogeneous_identity_w() {
    let v = Vec4::new(2.0, 3.0, 4.0, 1.0);
    let r = v.to_vec3_homogeneous();
    assert!((r.x - 2.0).abs() < 1e-10);
    assert!((r.y - 3.0).abs() < 1e-10);
    assert!((r.z - 4.0).abs() < 1e-10);
}

#[test]
fn vec4_to_vec3_homogeneous_perspective() {
    let v = Vec4::new(4.0, 6.0, 8.0, 2.0);
    let r = v.to_vec3_homogeneous();
    assert!((r.x - 2.0).abs() < 1e-10);
    assert!((r.y - 3.0).abs() < 1e-10);
    assert!((r.z - 4.0).abs() < 1e-10);
}

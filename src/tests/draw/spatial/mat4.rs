use super::*;

// ── 构造与 identity ──

#[test]
fn identity_matrix() {
    let i = Mat4::identity();
    assert!(i.is_identity());
    assert_eq!(i, Mat4::default());
}

#[test]
fn zero_matrix_not_identity() {
    assert!(!Mat4::zero().is_identity());
}

// ── 矩阵乘法 ──

#[test]
fn mul_identity_is_noop() {
    let a = Mat4::translate(1.0, 2.0, 3.0);
    let i = Mat4::identity();
    assert_eq!((a * i), a);
    assert_eq!((i * a), a);
}

#[test]
fn mul_translate_then_translate() {
    let t1 = Mat4::translate(1.0, 0.0, 0.0);
    let t2 = Mat4::translate(0.0, 2.0, 0.0);
    // t1 * t2 = 先 t2 后 t1? 不对，矩阵乘法是左乘
    // 这里用 transform_point 来验证
    let p = Vec3::new(0.0, 0.0, 0.0);
    let r = (t1 * t2).transform_point(&p);
    assert!((r.x - 1.0).abs() < 1e-6);
    assert!((r.y - 2.0).abs() < 1e-6);
}

// ── 平移 ──

#[test]
fn translate_point() {
    let t = Mat4::translate(10.0, 20.0, 30.0);
    let p = Vec3::new(1.0, 2.0, 3.0);
    let r = t.transform_point(&p);
    assert!((r.x - 11.0).abs() < 1e-6);
    assert!((r.y - 22.0).abs() < 1e-6);
    assert!((r.z - 33.0).abs() < 1e-6);
}

#[test]
fn translate_direction_unaffected() {
    let t = Mat4::translate(10.0, 20.0, 30.0);
    let d = Vec3::new(1.0, 0.0, 0.0);
    let r = t.transform_direction(&d);
    assert!((r.x - 1.0).abs() < 1e-6);
    assert!((r.y).abs() < 1e-6);
    assert!((r.z).abs() < 1e-6);
}

// ── 旋转 ──

#[test]
fn rotate_x_90_degrees() {
    let r = Mat4::rotate_x(std::f32::consts::FRAC_PI_2);
    let p = Vec3::new(0.0, 1.0, 0.0);
    let result = r.transform_point(&p);
    assert!((result.x).abs() < 1e-6, "x should be 0, got {}", result.x);
    assert!((result.y).abs() < 1e-6, "y should be 0, got {}", result.y);
    assert!(
        (result.z - 1.0).abs() < 1e-6,
        "z should be 1, got {}",
        result.z
    );
}

#[test]
fn rotate_y_90_degrees() {
    let r = Mat4::rotate_y(std::f32::consts::FRAC_PI_2);
    let p = Vec3::new(1.0, 0.0, 0.0);
    let result = r.transform_point(&p);
    assert!((result.x).abs() < 1e-6, "x should be 0, got {}", result.x);
    assert!((result.y).abs() < 1e-6, "y should be 0, got {}", result.y);
    assert!(
        (result.z + 1.0).abs() < 1e-6,
        "z should be -1, got {}",
        result.z
    );
}

#[test]
fn rotate_z_90_degrees() {
    let r = Mat4::rotate_z(std::f32::consts::FRAC_PI_2);
    let p = Vec3::new(1.0, 0.0, 0.0);
    let result = r.transform_point(&p);
    assert!((result.x).abs() < 1e-6, "x should be 0, got {}", result.x);
    assert!(
        (result.y - 1.0).abs() < 1e-6,
        "y should be 1, got {}",
        result.y
    );
}

// ── 缩放 ──

#[test]
fn scale_point() {
    let s = Mat4::scale(2.0, 3.0, 4.0);
    let p = Vec3::new(1.0, 2.0, 3.0);
    let r = s.transform_point(&p);
    assert!((r.x - 2.0).abs() < 1e-6);
    assert!((r.y - 6.0).abs() < 1e-6);
    assert!((r.z - 12.0).abs() < 1e-6);
}

// ── 逆矩阵 ──

#[test]
fn inverse_translate() {
    let t = Mat4::translate(10.0, 20.0, 30.0);
    let inv = t.inverse().unwrap();
    let r = t * inv;
    assert!(r.is_identity(), "T * T^-1 should be identity");
}

#[test]
fn inverse_rotate_scale() {
    let r = Mat4::rotate_z(0.5);
    let s = Mat4::scale(2.0, 3.0, 1.0);
    let m = r * s;
    let inv = m.inverse().unwrap();
    let result = m * inv;
    assert!(result.is_identity(), "M * M^-1 should be identity");
}

#[test]
fn inverse_perspective() {
    let p = Mat4::perspective(1.0, 1.6, 0.1, 100.0);
    let inv = p.inverse().unwrap();
    let r = p * inv;
    assert!(r.is_identity(), "P * P^-1 should be identity");
}

#[test]
fn singular_matrix_no_inverse() {
    let z = Mat4::zero();
    assert!(z.inverse().is_none());
}

// ── 投影 ──

#[test]
fn orthographic_is_orthographic() {
    let o = Mat4::orthographic_2d(800.0, 600.0);
    assert!(o.is_orthographic());
    assert!(!o.is_perspective());
}

#[test]
fn perspective_is_not_orthographic() {
    let p = Mat4::perspective(1.0, 1.6, 0.1, 100.0);
    assert!(p.is_perspective());
    assert!(!p.is_orthographic());
}

#[test]
fn orthographic_2d_translate() {
    let o = Mat4::orthographic_2d(800.0, 600.0);
    let p = Vec3::new(100.0, 200.0, 0.0);
    let r = o.transform_point(&p);
    // NDC: x=100 → -1 + 100/800*2 = -0.75, y=200 → 1 - 200/600*2 = 0.333
    assert!((r.x - (-0.75)).abs() < 1e-6, "ndc x: {}", r.x);
    assert!((r.y - 0.3333333).abs() < 1e-5, "ndc y: {}", r.y);
}

#[test]
fn perspective_w_division() {
    let p = Mat4::perspective(1.0, 1.6, 0.1, 100.0);
    let pt = Vec3::new(0.0, 0.0, -5.0); // z = -5 (在 near=0.1 和 far=100 之间)
    let r = p.transform_point(&pt);
    // 透视投影后 w ≠ 1，会做齐次除法
    assert!((r.z).abs() < 1.0, "ndc z should be in [-1,1], got {}", r.z);
}

// ── is_2d_only ──

#[test]
fn rotate_x_is_not_2d_only() {
    let r = Mat4::rotate_x(0.5);
    assert!(!r.is_2d_only());
}

#[test]
fn translate_z_is_not_2d_only() {
    let t = Mat4::translate(0.0, 0.0, 5.0);
    // z 平移会影响 m[14]，所以 !is_2d_only
    assert!(!t.is_2d_only(), "translate_z should not be 2d only");
}

// ── to_affine_2d ──

#[test]
fn extract_2d_affine() {
    let m = Mat4::translate(100.0, 50.0, 0.0) * Mat4::rotate_z(0.5);
    let aff = m.to_affine_2d();
    assert!(aff.is_some(), "should extract 2d affine");
    let (_a, _b, tx, _c, _d, ty) = aff.unwrap();
    assert!((tx - 100.0).abs() < 1e-6);
    assert!((ty - 50.0).abs() < 1e-6);
}

#[test]
fn cannot_extract_2d_affine_from_3d() {
    let m = Mat4::rotate_x(0.5);
    assert!(m.to_affine_2d().is_none());
}

// ── look_at ──

#[test]
fn look_at_default() {
    // 相机在 (0,0,5) 看向原点，up = (0,1,0)
    let view = Mat4::look_at(
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::zero(),
        Vec3::new(0.0, 1.0, 0.0),
    );
    let origin = Vec3::zero();
    let r = view.transform_point(&origin);
    // 相机看向 -Z，原点在相机前方，view 空间中原点的 z 应为 -5
    assert!((r.z - (-5.0)).abs() < 1e-6, "z should be -5, got {}", r.z);
}

// ── rotate_axis ──

#[test]
fn rotate_axis_x() {
    let r = Mat4::rotate_axis(Vec3::new(1.0, 0.0, 0.0), std::f32::consts::FRAC_PI_2);
    let p = Vec3::new(0.0, 1.0, 0.0);
    let result = r.transform_point(&p);
    assert!((result.x).abs() < 1e-6);
    assert!((result.y).abs() < 1e-6);
    assert!((result.z - 1.0).abs() < 1e-6);
}

// ── 组合变换 ──

#[test]
fn compound_transform() {
    // 先平移再旋转
    let t = Mat4::translate(10.0, 0.0, 0.0);
    let r = Mat4::rotate_z(std::f32::consts::FRAC_PI_2);
    let m = r * t; // 先应用 t，再应用 r

    let p = Vec3::new(0.0, 0.0, 0.0);
    let result = m.transform_point(&p);
    // 先平移到 (10,0,0)，再绕 Z 旋转 90° → (0,10,0)
    assert!((result.x).abs() < 1e-6, "x: {}", result.x);
    assert!((result.y - 10.0).abs() < 1e-6, "y: {}", result.y);
}

// ── 链式验证：translate → inverse → identity ──

#[test]
fn chain_translate_inverse() {
    let points = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 2.0, 3.0),
        Vec3::new(-5.0, 10.0, -3.0),
    ];
    let t = Mat4::translate(7.0, 8.0, 9.0);
    let inv = t.inverse().unwrap();
    for &p in &points {
        let transformed = t.transform_point(&p);
        let restored = inv.transform_point(&transformed);
        assert!(
            (restored - p).length() < 1e-5,
            "p: {:?} restored: {:?}",
            p,
            restored
        );
    }
}

// ── Transform ↔ Mat4 双向互转 ──

#[test]
fn transform_to_mat4_roundtrip_translate() {
    let t = crate::draw::Transform::translate(10.0, 20.0);
    let m = Mat4::from(t);
    let back = m.to_transform().unwrap();
    assert_eq!(back.m, t.m);
}

#[test]
fn mat4_to_transform_returns_none_for_3d() {
    // 绕 X 轴旋转会引入 z→x/y 耦合，不再是 2D-only
    let m = Mat4::rotate_x(0.5);
    assert!(m.to_transform().is_none());
}

#[test]
fn mat4_to_transform_returns_none_for_perspective() {
    let m = Mat4::perspective(1.0, 1.6, 0.1, 100.0);
    assert!(m.to_transform().is_none());
}

#[test]
fn mat4_translate_z_to_transform_none() {
    // z 平移不影响 x/y 渲染，但 is_2d_only 要求 m[14]==0，故返回 None
    let m = Mat4::translate(0.0, 0.0, 5.0);
    assert!(m.to_transform().is_none());
}

#[test]
fn affine_2d_layout_matches_transform_m() {
    // 验证 to_affine_2d 的 (a,b,tx,c,d,ty) 与 Transform.m 索引一致
    let t = crate::draw::Transform {
        m: [2.0, 0.5, 10.0, -0.5, 3.0, 20.0],
    };
    let m = Mat4::from(t);
    let (a, b, tx, c, d, ty) = m.to_affine_2d().unwrap();
    assert!((a - 2.0).abs() < 1e-6);
    assert!((b - 0.5).abs() < 1e-6);
    assert!((tx - 10.0).abs() < 1e-6);
    assert!((c - (-0.5)).abs() < 1e-6);
    assert!((d - 3.0).abs() < 1e-6);
    assert!((ty - 20.0).abs() < 1e-6);
}

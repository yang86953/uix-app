// 引入当前模块的几何校验辅助。
use super::valid_shadow_corners;

// 旋转后的平行四边形必须进入 RHI shadow lowering。
#[test]
fn accepts_affine_parallelogram() {
    // 构造一个带旋转/剪切的四角载荷。
    let corners = [[10.0, 20.0], [90.0, 60.0], [70.0, 140.0], [-10.0, 100.0]];
    // 确认单位 quad 可以稳定覆盖该四边形。
    assert!(valid_shadow_corners(&corners));
}

// 自交或凹四边形必须被门禁拒绝。
#[test]
fn rejects_non_convex_shadow_quad() {
    // 构造一个凹四边形，避免 shader 产生不确定覆盖。
    let corners = [[10.0, 20.0], [90.0, 20.0], [40.0, 40.0], [10.0, 80.0]];
    // 确认异常几何不会进入 RHI draw。
    assert!(!valid_shadow_corners(&corners));
}

// 引入当前区域与 sampled 合成 helper。
use super::{
    // 测试 blur 区域转换。
    lower_picture_blur_region,
    // 测试 sampled quad 对当前 opacity/Additive 的保真。
    lower_picture_sampled_quad,
};
// 引入统一逻辑矩形类型。
use crate::core::Rect;
// 引入 opaque texture 句柄验证 Picture 身份不被替换。
use crate::platform::presentation::rhi::TextureHandle;

// 逻辑离屏纹理的 fractional region 应只做整数覆盖取整。
#[test]
fn picture_blur_region_stays_in_logical_target_space() {
    // 构造一个需要向下取起点、向上取尺寸的区域。
    let region = lower_picture_blur_region(Rect::new(2.25, 3.75, 5.5, 7.25));
    // 起点应保持逻辑像素坐标，而不是被窗口 DPR 放大。
    assert_eq!((region.x, region.y), (2, 3));
    // 尺寸应覆盖原始 fractional 区域的末端。
    assert_eq!((region.width, region.height), (6, 8));
}

// 非有限或非正区域必须变成 blur renderer 可安全 no-op 的空尺寸。
#[test]
fn picture_blur_region_rejects_invalid_extent() {
    // 使用 NaN 起点和负尺寸覆盖异常输入边界。
    let region = lower_picture_blur_region(Rect::new(f32::NAN, 1.0, -2.0, f32::INFINITY));
    // 起点回到有限安全值。
    assert_eq!((region.x, region.y), (0, 1));
    // 空尺寸确保后续 blur 不会创建 scratch texture。
    assert_eq!((region.width, region.height), (0, 0));
}

// Picture blur 后的 sampled 合成必须保留当前目标的 opacity 与 Additive。
#[test]
fn picture_sampled_quad_preserves_additive_and_target_scale() {
    // 使用稳定 Picture texture 身份。
    let texture = TextureHandle::from_raw(41);
    // 组装主 surface 上的 Additive sampled quad。
    let quad = lower_picture_sampled_quad(
        // 指定 Picture texture。
        texture,
        // 使用逻辑目标矩形。
        Rect::new(2.0, 3.0, 4.0, 5.0),
        // 模拟水平二倍 drawable 比例。
        2.0,
        // 模拟垂直三倍 drawable 比例。
        3.0,
        // 使用有限半透明度。
        0.25,
        // 选择当前目标的 Additive blend。
        true,
        // 使用已归一化的裁剪 UV。
        [0.1, 0.2, 0.7, 0.9],
    );
    // 几何只应用一次目标比例。
    assert_eq!((quad.x, quad.y, quad.w, quad.h), (4.0, 9.0, 8.0, 15.0));
    // tint 四通道必须共同保存 opacity。
    assert_eq!(quad.rgba, [0.25; 4]);
    // lowering 必须选择 Additive sampled pipeline 事实。
    assert!(quad.additive);
    // texture 身份必须仍属于原 Picture。
    assert_eq!(quad.texture, texture);
    // UV 必须原样保留，避免 blur 后二次裁剪漂移。
    assert_eq!((quad.u0, quad.v0, quad.u1, quad.v1), (0.1, 0.2, 0.7, 0.9));
}

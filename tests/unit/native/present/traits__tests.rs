// 引入待验证的内部转换函数。
use super::rhi_resize_extent_for_logical;

// 验证有效 DPR 能按比例放大逻辑窗口尺寸。
#[test]
fn rhi_resize_extent_scales_logical_dimensions() {
    // 计算 900×640 在 1.5 DPR 下的物理 extent。
    let extent = match rhi_resize_extent_for_logical(900, 640, 1.5) {
        // 有效输入必须成功转换。
        Ok(extent) => extent,
        // 转换失败说明尺寸计算边界有缺口。
        Err(error) => panic!("valid DPR must produce a physical RHI extent: {error:?}"),
    };
    // 验证宽度按同一 DPR 转换。
    assert_eq!(extent.width, 1350);
    // 验证高度按同一 DPR 转换。
    assert_eq!(extent.height, 960);
}

// 验证无效 DPR 不会被静默修正为可用 surface。
#[test]
fn rhi_resize_extent_rejects_invalid_device_pixel_ratio() {
    // 传入零 DPR，要求返回明确的 state error。
    let result = rhi_resize_extent_for_logical(1200, 800, 0.0);
    // 验证调用方可以按 typed error 分类恢复。
    assert!(matches!(
        result,
        Err(error) if error.code() == crate::core::Errc::InvalidState
    ));
}

// 验证物理 extent 不会超过 adapter 当前的 i32 drawable 表示范围。
#[test]
fn rhi_resize_extent_rejects_adapter_extent_overflow() {
    // 让最大逻辑尺寸在 2.0 DPR 下产生超出 i32 的物理宽高。
    let result = rhi_resize_extent_for_logical(i32::MAX, i32::MAX, 2.0);
    // 验证范围错误不会被截断后交给 native surface。
    assert!(matches!(
        result,
        Err(error) if error.code() == crate::core::Errc::InvalidArgument
    ));
}

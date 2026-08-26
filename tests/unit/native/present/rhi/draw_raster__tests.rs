// 引入被测完整状态及共享几何值对象。
use super::*;

// 创建稳定的完整目标 viewport。
const VIEWPORT: RhiViewport = RhiViewport {
    // 固定测试宽度。
    width: 20.0,
    // 固定测试高度。
    height: 10.0,
};

// 创建位于目标内的显式裁剪。
const SCISSOR: RhiScissor = RhiScissor {
    // 从第二列开始。
    x: 1,
    // 从第三行开始。
    y: 2,
    // 覆盖四列。
    width: 4,
    // 覆盖五行。
    height: 5,
};

// 完整状态必须保持构造时冻结的两项事实。
#[test]
fn draw_raster_state_owns_viewport_and_scissor() {
    // 一次构造明确启用裁剪的状态。
    let state = DrawRasterState::new(VIEWPORT, Some(SCISSOR));
    // viewport 投影必须保持原值。
    assert_eq!(state.viewport(), VIEWPORT);
    // scissor 投影必须保持显式 Some。
    assert_eq!(state.scissor(), Some(SCISSOR));
    // 两项值都属于共享原生值域。
    assert!(state.is_valid());
    // 两项值都完整落在测试目标内。
    assert!(state.fits_within(RhiExtent::new(20, 10)));
}

// 无效或越界状态必须在任一 Adapter 前共享拒绝。
#[test]
fn draw_raster_state_rejects_invalid_or_outside_geometry() {
    // 小数 viewport 不能由两个 Adapter 选择不同量化规则。
    let fractional = DrawRasterState::new(
        // 构造非整像素宽度。
        RhiViewport {
            // 使用半像素宽度触发共同值域门禁。
            width: 19.5,
            // 高度保持合法。
            height: 10.0,
        },
        // 明确关闭裁剪。
        None,
    );
    // 小数 viewport 必须在目标边界检查前失败。
    assert!(!fractional.is_valid());
    // 合法值域但越过目标的 scissor 也必须失败。
    let outside = DrawRasterState::new(
        // 保持 viewport 合法。
        VIEWPORT,
        // 构造右边界越过目标的裁剪。
        Some(RhiScissor {
            // 从目标最右列开始。
            x: 19,
            // 从首行开始。
            y: 0,
            // 两列会越过目标。
            width: 2,
            // 保持高度合法。
            height: 1,
        }),
    );
    // 矩形自身仍属于共同原生值域。
    assert!(outside.is_valid());
    // 目标关系门禁必须拒绝越界组合。
    assert!(!outside.fits_within(RhiExtent::new(20, 10)));
}

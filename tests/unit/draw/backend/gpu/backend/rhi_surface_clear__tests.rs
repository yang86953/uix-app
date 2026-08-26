// 引入被测纯转换函数。
use super::physical_clear_scissor;
// 引入物理目标范围。
use crate::platform::presentation::rhi::RhiExtent;
// 引入父模块已收敛的逻辑清理记录。
use super::GpuSolidRect;

// 验证 mixed-DPI 清理范围按绝对边界取整。
#[test]
fn pending_clear_rect_scales_into_retained_texture() {
    // 构造无圆角透明 damage 清理记录。
    let rect = GpuSolidRect {
        // 设置逻辑左边界。
        x: 10.0,
        // 设置逻辑上边界。
        y: 20.0,
        // 设置逻辑宽度。
        w: 30.0,
        // 设置逻辑高度。
        h: 40.0,
        // 使用透明 premultiplied black。
        rgba: [0.0; 4],
        // damage 清理不携带圆角。
        radius: [0.0; 4],
    };
    // 按非对称 drawable 比例降低到 retained texture。
    let scissor = physical_clear_scissor(rect, 1.5, 2.0, RhiExtent::new(100, 200))
        // 有效 damage 必须产生物理清理矩形。
        .expect("有效 damage 应降低为 retained ClearRect");
    // 验证物理左边界。
    assert_eq!(scissor.x, 15);
    // 验证物理上边界。
    assert_eq!(scissor.y, 40);
    // 验证物理宽度。
    assert_eq!(scissor.width, 45);
    // 验证物理高度。
    assert_eq!(scissor.height, 80);
}

// 验证无法表达的高层圆角不会被误清理。
#[test]
fn pending_clear_rect_rejects_non_rectangular_geometry() {
    // 构造带圆角的非法 damage 清理记录。
    let rect = GpuSolidRect {
        // 设置逻辑左边界。
        x: 0.0,
        // 设置逻辑上边界。
        y: 0.0,
        // 设置逻辑宽度。
        w: 20.0,
        // 设置逻辑高度。
        h: 20.0,
        // 使用透明 premultiplied black。
        rgba: [0.0; 4],
        // 非零圆角不能进入 ClearRect 原语。
        radius: [1.0; 4],
    };
    // 无法无损表达时必须让上层返回 typed lowering failure。
    assert!(physical_clear_scissor(rect, 1.0, 1.0, RhiExtent::new(20, 20)).is_none());
}

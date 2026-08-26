// 引入本模块的显式 extent 几何与裁剪换算。
use super::{rhi_physical_geometry, rhi_physical_scissor};
// 引入 API 无关的物理范围与裁剪值。
use crate::platform::presentation::rhi::{RhiExtent, RhiScissor};

// 证明 Drawing 只凭显式目标 extent 就能统一计算 viewport、DPI 与裁剪。
#[test]
fn explicit_extent_drives_geometry_and_scissor_without_surface() {
    // 构造与任何 swapchain 代际无关的纹理范围。
    let extent = RhiExtent::new(300, 200);
    // 用非对称逻辑尺寸覆盖 X/Y 不同缩放比例。
    let (viewport, scale_x, scale_y) = rhi_physical_geometry(extent, 100, 50);
    // viewport 必须完整覆盖显式纹理宽度。
    assert_eq!(viewport.width, 300.0);
    // viewport 必须完整覆盖显式纹理高度。
    assert_eq!(viewport.height, 200.0);
    // 水平比例只能由 extent 与逻辑宽度推导。
    assert_eq!(scale_x, 3.0);
    // 垂直比例只能由 extent 与逻辑高度推导。
    assert_eq!(scale_y, 4.0);
    // 将逻辑矩形换算为同一物理范围内的整数裁剪。
    let scissor = rhi_physical_scissor((10, 5, 20, 10), scale_x, scale_y, extent)
        // 测试输入完全可见，缺少裁剪说明契约被破坏。
        .expect("explicit extent should produce a visible scissor");
    // 两轴缩放后的裁剪必须保持同一 Drawing 语义。
    assert_eq!(
        // 比较 API 无关的整数物理矩形。
        scissor,
        // 期望坐标与尺寸分别使用 X/Y 比例。
        RhiScissor {
            // 水平原点按三倍缩放。
            x: 30,
            // 垂直原点按四倍缩放。
            y: 20,
            // 水平宽度按三倍缩放。
            width: 60,
            // 垂直高度按四倍缩放。
            height: 40,
        }
    );
}

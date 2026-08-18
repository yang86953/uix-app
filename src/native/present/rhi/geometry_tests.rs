//! RHI viewport 与 scissor 的共享目标边界契约测试。

// 复用父模块的中立几何值，不实例化任何原生图形 API。
use super::{RhiExtent, RhiScissor, RhiViewport};

// 验证两个 Adapter 必须共享的完整目标边界语义。
#[test]
fn viewport_and_scissor_share_exact_target_extent_contract() {
    // 建立一个稳定的物理 render target 范围。
    let extent = RhiExtent::new(100, 80);
    // 恰好覆盖完整目标的 viewport 必须合法。
    assert!(
        RhiViewport {
            // 宽度与目标宽度完全一致。
            width: 100.0,
            // 高度与目标高度完全一致。
            height: 80.0,
        }
        // 通过共享目标范围判定。
        .fits_within(extent)
    );
    // 仅越过一个小数像素的 viewport 也必须在所有 Adapter 上拒绝。
    assert!(
        !RhiViewport {
            // 超出目标宽度四分之一个像素。
            width: 100.25,
            // 高度仍保持合法。
            height: 80.0,
        }
        // 通过同一共享判定取得拒绝结果。
        .fits_within(extent)
    );
    // 即使没有越过目标，小数物理 viewport 也必须被共享层拒绝。
    assert!(
        !RhiViewport {
            // 小数宽度会导致 OpenGL 取整而 D3D11 保留原值。
            width: 99.5,
            // 高度保持合法整像素。
            height: 80.0,
        }
        // 共享值对象必须先消除这种跨 API 量化分叉。
        .fits_within(extent)
    );
    // 超出有符号原生 viewport 值域的浮点整数必须被拒绝。
    assert!(
        !RhiViewport {
            // 该值是 i32 最大值之后第一个可表示的 f32 整数。
            width: 2_147_483_648.0,
            // 高度保持最小正整像素。
            height: 1.0,
        }
        // 最大无符号目标不能绕过共享原生值域门禁。
        .fits_within(RhiExtent::new(u32::MAX, u32::MAX))
    );
    // 恰好贴住右下边界的 scissor 必须合法。
    assert!(
        RhiScissor {
            // 从目标右侧前十像素开始。
            x: 90,
            // 从目标底部前十像素开始。
            y: 70,
            // 右边界恰好等于目标宽度。
            width: 10,
            // 下边界恰好等于目标高度。
            height: 10,
        }
        // 通过共享目标范围判定。
        .fits_within(extent)
    );
    // 越过右边界一个像素的 scissor 必须在所有 Adapter 上拒绝。
    assert!(
        !RhiScissor {
            // 右边界将到达一百零一。
            x: 91,
            // 垂直位置仍保持合法。
            y: 70,
            // 使用十像素宽度形成越界。
            width: 10,
            // 高度仍贴住底部。
            height: 10,
        }
        // 通过同一共享判定取得拒绝结果。
        .fits_within(extent)
    );
    // 负起点必须在边界加法前被拒绝。
    assert!(
        !RhiScissor {
            // 使用非法负水平起点。
            x: -1,
            // 垂直起点保持为零。
            y: 0,
            // 宽度使用最小正值。
            width: 1,
            // 高度使用最小正值。
            height: 1,
        }
        // 共享判定不得把负数转换为巨大无符号值后误接收。
        .fits_within(extent)
    );
    // 即使无符号目标足够大，有符号原生边界溢出也必须在共享层拒绝。
    assert!(
        !RhiScissor {
            // 从有符号坐标最大值开始。
            x: i32::MAX,
            // 垂直起点保持为零。
            y: 0,
            // 增加一个像素会使右边界无法被原生 RECT 表达。
            width: 1,
            // 高度保持最小正值。
            height: 1,
        }
        // 使用最大无符号目标证明拒绝原因来自原生边界值域而不是目标尺寸。
        .fits_within(RhiExtent::new(u32::MAX, u32::MAX))
    );
}

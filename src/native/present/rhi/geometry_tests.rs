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
        // 直接读取 viewport 自身的共同原生值域门禁。
        .is_valid()
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
        RhiScissor {
            // 从有符号坐标最大值开始。
            x: i32::MAX,
            // 垂直起点保持为零。
            y: 0,
            // 增加一个像素会使右边界无法被原生 RECT 表达。
            width: 1,
            // 高度保持最小正值。
            height: 1,
        }
        // 直接读取 scissor 自身的 checked 矩形投影。
        .native_rect()
        // 有符号远端边界溢出必须返回空。
        .is_none()
    );
}

// 验证各 Adapter 只消费同一套原生尺寸、矩形与目标方向投影。
#[test]
fn native_geometry_projections_are_checked_and_lossless() {
    // 创建可由全部现有原生 ABI 表达的目标尺寸。
    let extent = RhiExtent::new(100, 80);
    // 共享投影必须保留精确宽高。
    assert_eq!(extent.native_size_i32(), Some((100, 80)));
    // 零宽目标不得进入资源或 pass 生命周期。
    assert_eq!(RhiExtent::new(0, 80).native_size_i32(), None);
    // 超过有符号上限的目标不得由 Adapter 各自截断。
    assert_eq!(
        RhiExtent::new(i32::MAX as u32 + 1, 1).native_size_i32(),
        None
    );
    // 创建完整目标 viewport。
    let viewport = RhiViewport {
        // 使用整像素宽度。
        width: 100.0,
        // 使用整像素高度。
        height: 80.0,
    };
    // OpenGL 有符号尺寸与 D3D11 浮点尺寸必须表示同一整数。
    assert_eq!(viewport.native_size_i32(), Some((100, 80)));
    // 创建贴住右下角的左上原点 scissor。
    let scissor = RhiScissor {
        // 从右边前十像素开始。
        x: 90,
        // 从底边前十像素开始。
        y: 70,
        // 宽度延伸到目标右边界。
        width: 10,
        // 高度延伸到目标下边界。
        height: 10,
    };
    // D3D11 RECT 与 clear 必须消费相同四边。
    assert_eq!(scissor.native_rect(), Some((90, 70, 100, 80)));
    // OpenGL surface 底部原点换算后该矩形从零开始。
    assert_eq!(scissor.bottom_origin_y(extent), Some(0));
    // 创建不贴边矩形以锁定一般坐标翻转公式。
    let interior = RhiScissor {
        // 水平位置不影响 Y 翻转。
        x: 5,
        // 从顶部十像素开始。
        y: 10,
        // 使用稳定正宽度。
        width: 20,
        // 矩形高度为二十像素。
        height: 20,
    };
    // 底部原点 Y 必须等于目标高度减去顶部坐标系底边。
    assert_eq!(interior.bottom_origin_y(extent), Some(50));
}

// 引入待测 Canvas 与 pending operation。
use super::{NativeGpuCanvas2D, PendingNativeOp};
// 引入矩形和仿射变换测试几何。
use crate::core::Rect;
// 引入 blend 与 transform 状态。
use crate::draw::geometry::types::{BlendMode, Transform};
// 引入公开 Canvas2D 方法。
use crate::draw::Canvas2D;
// 引入测试颜色。
use crate::draw::Color;
// 引入 graphics backend 私有的 renderer 能力投影。
use crate::draw::backend::gpu::NativeRasterCaps;

// 不同半径的圆形填充与描边必须保留同一个分数设备圆心。
#[test]
fn circle_queue_preserves_shared_fractional_center() {
    let native_caps = NativeRasterCaps {
        retained_color_target: true,
        ..NativeRasterCaps::default()
    };
    let mut native = NativeGpuCanvas2D::new(40, 30, native_caps);
    // 内点半径为半像素、外圈半径为整数，曾分别按边界取整并产生半像素偏心。
    native.fill_circle(10.3, 8.6, 3.5, Color::blue());
    native.stroke_circle(10.3, 8.6, 6.0, Color::blue(), 1.5);

    let [
        PendingNativeOp::SolidRect(fill),
        PendingNativeOp::StrokeRect(stroke),
    ] = native.pending_native.as_slice()
    else {
        panic!("圆形应进入 retained shape 队列");
    };
    let fill_center = (
        fill.rect.x + fill.rect.w * 0.5,
        fill.rect.y + fill.rect.h * 0.5,
    );
    let stroke_center = (
        stroke.rect.x + stroke.rect.w * 0.5,
        stroke.rect.y + stroke.rect.h * 0.5,
    );
    assert!((fill_center.0 - 10.3).abs() < f32::EPSILON);
    assert!((fill_center.1 - 8.6).abs() < f32::EPSILON);
    assert_eq!(fill_center, stroke_center);
}

// 能力存在时直达，能力或轴对齐证明缺失时保持 Additive soft segment。
#[test]
fn additive_stroke_respects_rhi_capability_and_axis_alignment() {
    // 启用 retained RHI 与 Additive 事实能力。
    let native_caps = NativeRasterCaps {
        // retained surface 允许固定 probe 覆盖的图元进入 native queue。
        retained_color_target: true,
        // 声明通用 RHI 可以执行 Additive shape pipeline。
        rhi_additive_blend: true,
        // 其余能力保持关闭，避免测试依赖无关图元。
        ..NativeRasterCaps::default()
    };
    // 使用 hybrid canvas，确保错误分流会真实创建 soft staging。
    let mut native = NativeGpuCanvas2D::new(32, 24, native_caps);
    // 切换到目标相关 Additive blend。
    native.set_blend_mode(BlendMode::Additive);
    // 圆形描边应复用带圆角的 StrokeRect shape。
    native.stroke_circle(10.0, 8.0, 3.0, Color::green(), 1.0);
    // pending 载荷必须显式保留 Additive 与圆形半径事实。
    assert!(matches!(
        native.pending_native.as_slice(),
        [PendingNativeOp::StrokeRect(stroke)]
            if stroke.additive && stroke.rect.radius == [3.0; 4]
    ));
    // 直达 RHI shape 时不得创建 CPU surface。
    assert!(native.soft_fallback.is_none());
    // native 内容不能同时伪装成 soft segment。
    assert!(!native.soft_has_content);

    // 仅声明 retained surface，刻意缺少 Additive RHI 事实。
    let fallback_caps = NativeRasterCaps {
        // 证明分流失败不是因为缺少 retained surface。
        retained_color_target: true,
        // 其余能力包括 rhi_additive_blend 保持默认 false。
        ..NativeRasterCaps::default()
    };
    // 使用允许 soft fallback 的第二个 hybrid canvas。
    let mut fallback = NativeGpuCanvas2D::new(20, 12, fallback_caps);
    // 请求 Additive 描边矩形。
    fallback.set_blend_mode(BlendMode::Additive);
    // 绘制有限轴对齐描边。
    fallback.stroke_rect(Rect::new(2.0, 2.0, 6.0, 4.0), Color::red(), 1.0, None);
    // 没有事实能力时不能生成 native operation。
    assert!(fallback.pending_native.is_empty());
    // 等价 CPU staging 必须存在。
    assert!(fallback.soft_fallback.is_some());
    // 快照应只包含一个 Additive soft segment。
    let segments = fallback.packed_soft_segments();
    // 单次绘制不能产生额外段。
    assert_eq!(segments.len(), 1);
    // 段的目标 blend 必须保持 Additive。
    assert!(segments[0].additive);

    // 启用 retained surface，验证 Additive 仿射描边仍不会误入 SrcOver tessellation。
    let transformed_caps = NativeRasterCaps {
        // 固定 probe 已覆盖描边与 mesh pipeline。
        retained_color_target: true,
        // Additive pipeline 能力本身存在。
        rhi_additive_blend: true,
        // 其余能力保持默认。
        ..NativeRasterCaps::default()
    };
    // 使用第三个 hybrid canvas 覆盖非轴对齐边界。
    let mut transformed = NativeGpuCanvas2D::new(24, 18, transformed_caps);
    // 应用带剪切分量的可逆仿射变换。
    transformed.set_transform(Transform {
        // 非零 b 让矩形不再轴对齐。
        m: [1.0, 0.25, 0.0, 0.0, 1.0, 0.0],
    });
    // 请求 Additive 语义。
    transformed.set_blend_mode(BlendMode::Additive);
    // 绘制一个变换后的描边矩形。
    transformed.stroke_rect(Rect::new(3.0, 3.0, 8.0, 5.0), Color::blue(), 1.0, None);
    // 非轴对齐 Additive 不得偷换成普通 solid mesh。
    assert!(transformed.pending_native.is_empty());
    // 当前非目标范围继续由等价 soft segment 承接。
    assert!(transformed.soft_fallback.is_some());
}

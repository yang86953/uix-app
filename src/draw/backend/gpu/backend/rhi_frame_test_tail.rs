//! FrameEncoder RHI lowering 的顺序和几何测试。

// 引入测试所需的编码器几何和 raster 操作。
use crate::draw::painting::{
    FrameEncoder, FrameEncoderError, FrameRadius, FrameRasterOp, FrameRect, FrameStrokeRect,
    FrameStrokeWidth,
};
// 引入 RHI 操作枚举以区分普通与加法 shape pipeline。
use crate::draw::backend::rhi_renderer::RhiOp;
// 引入测试颜色和底层物理尺寸。
use crate::draw::Color;
// 引入测试使用的通用纹理和 viewport 值。
use crate::native::present::rhi::{RhiExtent, RhiViewport, TextureHandle};

// 片段拆分必须保留 scroll 前后的普通 draw 顺序。
#[test]
fn lower_frame_encoder_splits_scroll_boundary() {
    // 创建一个小尺寸、便于审计的编码器。
    let Ok(mut encoder) = FrameEncoder::new(8, 6) else {
        // 合法测试尺寸必须可构造。
        panic!("test encoder dimensions are valid");
    };
    // 记录 scroll 前的普通 shape。
    encoder.native(FrameRasterOp::FillRect {
        rect: FrameRect::new(0, 0, 2, 2),
        color: Color::red(),
    });
    // 记录一个把 source 向右平移后复制回 viewport 的 scroll。
    encoder.native(FrameRasterOp::ScrollCopy {
        viewport: FrameRect::new(1, 1, 4, 3),
        dx: 1,
        dy: 0,
    });
    // 记录 scroll 后的第二个普通 shape。
    encoder.native(FrameRasterOp::FillRect {
        rect: FrameRect::new(3, 2, 2, 2),
        color: Color::blue(),
    });
    // 执行只读 lowering，不触碰任何 native context。
    let lowered = match super::lower_frame_encoder(
        &encoder,
        RhiViewport {
            width: 8.0,
            height: 6.0,
        },
        1.0,
        1.0,
    ) {
        // 合法测试操作必须全部可表示。
        Ok(Some(lowered)) => lowered,
        // 编码器内容不可表示说明 lowering 校验过严。
        Ok(None) => panic!("all test operations should be representable"),
        // 只读 lowering 不允许失败。
        Err(error) => panic!("scroll lowering should not fail: {error:?}"),
    };
    // 一个 scroll 应该把连续普通操作拆成两个片段。
    assert_eq!(lowered.segments.len(), 2);
    // 首段不能提前携带 move。
    assert!(lowered.segments[0].move_before.is_none());
    // 首段只包含 scroll 前的 shape。
    assert_eq!(lowered.segments[0].operations.len(), 1);
    // 后段必须携带原始 scroll 的位移。
    assert_eq!(lowered.segments[1].move_before.map(|item| item.dx), Some(1));
    // 后段只包含 scroll 后的 shape。
    assert_eq!(lowered.segments[1].operations.len(), 1);
}

// 验证 FrameEncoder scroll 与 CPU source/destination 裁剪保持一致。
#[test]
fn lower_frame_scroll_move_clips_and_scales() {
    // 创建一个 viewport source 向右偏移一个逻辑像素的搬移。
    let movement = match super::lower_frame_scroll_move(
        super::FrameScrollCopy {
            viewport: FrameRect::new(1, 1, 4, 3),
            dx: 1,
            dy: 0,
        },
        TextureHandle::from_raw(9),
        8,
        6,
        1.0,
        1.0,
        RhiExtent::new(8, 6),
    ) {
        // 非空 scroll 必须产生一个物理搬移。
        Ok(Some(movement)) => movement,
        // 无搬移结果说明裁剪逻辑与编码器语义矛盾。
        Ok(None) => panic!("non-empty scroll should produce a move"),
        // 整数 DPR 的 scroll 不允许失败。
        Err(error) => panic!("integral scroll should lower: {error:?}"),
    };
    // source 起点是 viewport + delta，destination 起点保持 viewport 原点。
    assert_eq!(movement.source_x, 2);
    assert_eq!(movement.destination_x, 1);
    // 共同可见区域宽高应保持 viewport 尺寸。
    assert_eq!((movement.width, movement.height), (4, 3));
    // 非整数 DPR 不能伪造整数纹理搬移。
    let result = super::lower_frame_scroll_move(
        super::FrameScrollCopy {
            viewport: FrameRect::new(0, 0, 2, 2),
            dx: 1,
            dy: 0,
        },
        TextureHandle::from_raw(9),
        4,
        4,
        1.25,
        1.0,
        RhiExtent::new(5, 4),
    );
    // 失败必须保持 NotImplemented，而不是返回错位的物理区域。
    assert!(matches!(result, Err(error) if error.code() == crate::core::Errc::NotImplemented));
}

// 验证不同 blend 的相邻描边不会混批，并映射到对应 RHI shape 操作。
#[test]
fn additive_stroke_stays_separate_and_lowers_to_additive_shape() {
    // 创建一个足以容纳两个互不相交描边的编码器。
    let Ok(mut encoder) = FrameEncoder::new(12, 6) else {
        // 合法测试尺寸必须可构造。
        panic!("test encoder dimensions are valid");
    };
    // 构造通过边界校验的一像素描边宽度。
    let Ok(line_width) = FrameStrokeWidth::new(1.0) else {
        // 正有限宽度必须通过验证。
        panic!("test stroke width is valid");
    };
    // 记录普通 SrcOver 描边作为前一批次。
    encoder.native(FrameRasterOp::StrokeRoundedRects {
        // 使用左侧矩形，避免与后一描边产生几何重叠。
        strokes: vec![FrameStrokeRect::new(
            FrameRect::new(1, 1, 3, 3),
            Color::red(),
            FrameRadius::zero(),
            line_width,
        )],
        // 两批故意使用完全相同的 clip。
        clip: FrameRect::new(0, 0, 12, 6),
        // 首批保持普通 SrcOver。
        additive: false,
    });
    // 紧邻记录 Additive 描边，专门覆盖 blend 批隔离。
    encoder.native(FrameRasterOp::StrokeRoundedRects {
        // 使用右侧矩形，排除几何重叠本身造成的拆批。
        strokes: vec![FrameStrokeRect::new(
            FrameRect::new(8, 1, 3, 3),
            Color::green(),
            FrameRadius::zero(),
            line_width,
        )],
        // 保持与首批相同的完整 clip。
        clip: FrameRect::new(0, 0, 12, 6),
        // 第二批显式选择 Additive。
        additive: true,
    });
    // blend 不同必须在 encoder 阶段保持两个原子命令。
    assert_eq!(encoder.commands().len(), 2);
    // 执行只读 lowering，不触碰任何 native context。
    let lowered = match super::lower_frame_encoder(
        &encoder,
        RhiViewport {
            width: 12.0,
            height: 6.0,
        },
        1.0,
        1.0,
    ) {
        // 合法 shape 操作必须全部可表示。
        Ok(Some(lowered)) => lowered,
        // 编码器内容不可表示说明 lowering 校验过严。
        Ok(None) => panic!("both stroke blends should be representable"),
        // 只读 lowering 不允许失败。
        Err(error) => panic!("stroke lowering should not fail: {error:?}"),
    };
    // 没有 scroll 时所有 draw 应保留在同一个 painter-order 片段。
    assert_eq!(lowered.segments.len(), 1);
    // 两个 blend 批次必须产生两个独立 RHI 操作。
    assert_eq!(lowered.segments[0].operations.len(), 2);
    // 首条描边必须继续使用普通 SrcOver shape pipeline。
    assert!(matches!(lowered.segments[0].operations[0], RhiOp::Shape(_)));
    // 第二条描边必须进入饱和加法 shape pipeline。
    assert!(matches!(
        lowered.segments[0].operations[1],
        RhiOp::AdditiveShape(_)
    ));
    // 另建编码器验证 Additive 描边绝不能伪装成透明 CPU segment。
    let Ok(mut cpu_encoder) = FrameEncoder::new(6, 6) else {
        // 合法测试尺寸必须可构造。
        panic!("test encoder dimensions are valid");
    };
    // 尝试通过 source-independent API 记录目标相关 Additive 描边。
    let result = cpu_encoder.cpu_segment([FrameRasterOp::StrokeRoundedRects {
        // 使用完全位于测试 surface 内的合法描边，排除几何失败。
        strokes: vec![FrameStrokeRect::new(
            FrameRect::new(1, 1, 3, 3),
            Color::green(),
            FrameRadius::zero(),
            line_width,
        )],
        // 使用完整 surface clip。
        clip: FrameRect::new(0, 0, 6, 6),
        // 唯一拒绝原因应是目标相关 blend。
        additive: true,
    }]);
    // 拒绝结果必须携带稳定的操作名，供 typed 诊断使用。
    assert!(matches!(
        result,
        Err(FrameEncoderError::DestinationDependentCpuSegment {
            operation: "StrokeRoundedRectsAdditive"
        })
    ));
    // 失败必须发生在编码器变更之前。
    assert!(cpu_encoder.commands().is_empty());
}

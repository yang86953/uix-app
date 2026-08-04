//! FrameEncoder RHI lowering 的顺序和几何测试。

// 引入测试所需的编码器几何和 raster 操作。
use crate::draw::painting::{FrameEncoder, FrameRasterOp, FrameRect};
// 引入测试颜色和底层物理尺寸。
use crate::draw::Color;
// 引入测试使用的通用纹理和 viewport 值。
use crate::native::present::rhi::{RhiExtent, RhiViewport, TextureHandle};

// 片段拆分必须保留 scroll 前后的普通 draw 顺序。
#[test]
fn lower_frame_encoder_splits_scroll_boundary() {
    // 创建一个小尺寸、便于审计的编码器。
    let mut encoder = FrameEncoder::new(8, 6).expect("test encoder dimensions are valid");
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
    let lowered = super::lower_frame_encoder(
        &encoder,
        RhiViewport {
            width: 8.0,
            height: 6.0,
        },
        1.0,
        1.0,
    )
    .expect("scroll lowering should not fail")
    .expect("all test operations should be representable");
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
    let movement = super::lower_frame_scroll_move(
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
    )
    .expect("integral scroll should lower")
    .expect("non-empty scroll should produce a move");
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

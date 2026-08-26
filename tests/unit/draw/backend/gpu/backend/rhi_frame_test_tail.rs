//! FrameEncoder RHI lowering 的顺序和几何测试。

// 引入测试所需的编码器几何和 raster 操作。
use crate::draw::painting::{
    FrameCommand, FrameEncoder, FrameEncoderError, FrameImage, FrameOpacity, FrameRadius,
    FrameRasterOp, FrameRect, FrameSampledRect, FrameStrokeRect, FrameStrokeWidth,
};
// 引入 RHI 操作枚举以区分普通与加法 shape pipeline。
use crate::draw::backend::rhi_renderer::RhiOp;
// 引入测试颜色和底层物理尺寸。
use crate::draw::Color;
// 引入测试使用的通用纹理和 viewport 值。
use crate::platform::presentation::rhi::{RhiColor, RhiExtent, RhiViewport, TextureHandle};

// 完整图片 lowering 必须延续共享所有权，只有真实裁剪才生成紧密载荷。
#[test]
fn full_picture_crop_reuses_shared_pixels_and_partial_crop_is_tight() {
    let pixels = std::sync::Arc::new(vec![1, 2, 3, 4, 5, 6]);
    let image = FrameImage::from_shared(3, 2, std::sync::Arc::clone(&pixels))
        .expect("完整测试图片应满足像素数量契约");
    let full = super::copy_image_crop(&image, FrameRect::new(0, 0, 3, 2))
        .expect("完整图片 lowering 不应失败")
        .expect("完整图片必须产生载荷");
    assert!(std::sync::Arc::ptr_eq(&pixels, &full));

    let crop = super::copy_image_crop(&image, FrameRect::new(1, 0, 2, 2))
        .expect("合法裁剪 lowering 不应失败")
        .expect("合法裁剪必须产生载荷");
    assert!(!std::sync::Arc::ptr_eq(&pixels, &crop));
    assert_eq!(crop.as_slice(), [2, 3, 5, 6]);
}

// 页面销毁产生的空 PictureBlit 必须与 CPU 参考执行保持相同的 no-op 语义。
#[test]
// 同时锁定录制入口和历史命令流的 retained RHI 兼容边界。
fn empty_picture_blit_is_a_lossless_no_op() {
    // 创建一张有效的一像素图片，隔离空目标这一具体页面切换事实。
    let image = match FrameImage::solid(1, 1, Color::red()) {
        // 合法图片应成功构造。
        Ok(image) => image,
        // 测试载荷构造失败属于测试错误。
        Err(error) => panic!("test picture should be valid: {error:?}"),
    };
    // 创建公开录制入口使用的主帧编码器。
    let mut recorded = match FrameEncoder::new(8, 6) {
        // 合法尺寸应成功构造。
        Ok(encoder) => encoder,
        // 测试尺寸失败属于测试错误。
        Err(error) => panic!("test encoder should be valid: {error:?}"),
    };
    // 模拟页面销毁后宽度归零的 Picture 目标。
    recorded.blit_picture(
        // 图片本身保持有效。
        image.clone(),
        // 源区域覆盖完整图片。
        FrameRect::new(0, 0, 1, 1),
        // 目标宽度为零，因此没有像素贡献。
        FrameRect::new(4, 3, 0, 1),
    );
    // 新录制入口不应保留这个安全 no-op。
    assert!(recorded.commands().is_empty());

    // 创建一个模拟旧缓存命令流的独立编码器。
    let mut retained = match FrameEncoder::new(8, 6) {
        // 合法尺寸应成功构造。
        Ok(encoder) => encoder,
        // 测试尺寸失败属于测试错误。
        Err(error) => panic!("retained test encoder should be valid: {error:?}"),
    };
    // 直接追加旧版可能保留下来的空 PictureBlit。
    let append = retained.append_validated_commands(vec![FrameCommand::PictureBlit {
        // 保留有效图片载荷。
        image,
        // 保留有效源区域。
        src: FrameRect::new(0, 0, 1, 1),
        // 复现整数空目标被规范化为零 sampled rect 的事实。
        dst: FrameSampledRect::from_integer(FrameRect::new(4, 3, 0, 1)),
        // 使用可见 opacity，证明 no-op 来自空几何。
        opacity: FrameOpacity::opaque(),
        // 页面切换图片沿用普通 SrcOver。
        additive: false,
    }]);
    // 已验证命令追加不应引入参数错误。
    assert!(append.is_ok());
    // 对旧命令流执行纯 lowering，不触碰原生设备。
    let lowered = super::lower_frame_encoder(
        // 传入包含空 PictureBlit 的完整命令流。
        &retained,
        // 使用与逻辑尺寸相同的物理 viewport。
        RhiViewport {
            // 设置物理宽度。
            width: 8.0,
            // 设置物理高度。
            height: 6.0,
        },
        // 使用单位水平比例。
        1.0,
        // 使用单位垂直比例。
        1.0,
    );
    // 空图片必须完整 lower，而不是返回 Unsupported。
    let Some(lowered) = lowered.expect("empty picture lowering should not fail") else {
        // None 会重新触发主 surface typed failure。
        panic!("empty picture should lower as a no-op");
    };
    // 空图片不应生成任何 draw 操作。
    assert!(lowered.segments[0].operations.is_empty());
}

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
    assert_eq!(movement.transfer().source().origin().x(), 2);
    // 目标横坐标必须由同一传输几何派生。
    assert_eq!(movement.transfer().destination().origin().x(), 1);
    // 共同可见区域宽高应保持 viewport 尺寸。
    assert_eq!(
        (
            movement.transfer().extent().width,
            movement.transfer().extent().height
        ),
        (4, 3)
    );
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

// 中途 clear 必须成为新的 RHI pass，而不是迫使主帧切回 legacy adapter。
#[test]
fn lower_frame_encoder_splits_mid_frame_clear_boundary() {
    // 创建一个小尺寸编码器。
    let Ok(mut encoder) = FrameEncoder::new(8, 6) else {
        // 合法测试尺寸必须可构造。
        panic!("test encoder dimensions are valid");
    };
    // 记录 clear 之前的普通 shape。
    encoder.native(FrameRasterOp::FillRect {
        // 使用左上角小矩形。
        rect: FrameRect::new(0, 0, 2, 2),
        // 使用可区分的红色。
        color: Color::red(),
    });
    // 在帧中途清为半透明蓝色，覆盖不会经过硬件混合的预乘边界。
    encoder.clear(Color::from_rgba(0, 0, 255, 128));
    // 记录 clear 之后的普通 shape。
    encoder.native(FrameRasterOp::FillRect {
        // 使用右下区域小矩形。
        rect: FrameRect::new(4, 3, 2, 2),
        // 使用可区分的红色以外颜色。
        color: Color::green(),
    });
    // 执行只读 lowering，不触碰 native context。
    let lowered = match super::lower_frame_encoder(
        // 传入完整命令流。
        &encoder,
        // 使用与逻辑尺寸一致的 viewport。
        RhiViewport {
            // 设置物理宽度。
            width: 8.0,
            // 设置物理高度。
            height: 6.0,
        },
        // 使用单位水平比例。
        1.0,
        // 使用单位垂直比例。
        1.0,
    ) {
        // 合法命令必须完整 lower。
        Ok(Some(lowered)) => lowered,
        // 中途 clear 不得被当成能力缺口。
        Ok(None) => panic!("mid-frame clear should be representable"),
        // 纯 lowering 不允许失败。
        Err(error) => panic!("mid-frame clear lowering should not fail: {error:?}"),
    };
    // 中途 clear 应把命令流拆成前后两个 pass 片段。
    assert_eq!(lowered.segments.len(), 2);
    // 首段沿用调用方 load，不提前执行蓝色 clear。
    assert!(lowered.segments[0].clear_before.is_none());
    // 首段只保留 clear 前的 shape。
    assert_eq!(lowered.segments[0].operations.len(), 1);
    // 计算与 8-bit alpha 相同的规范浮点值。
    let alpha = 128.0 / 255.0;
    // 第二段必须以预乘蓝色 clear 初始化目标。
    assert_eq!(
        // 读取第二段的显式清理颜色。
        lowered.segments[1].clear_before,
        // 蓝色 RGB 必须在共享 RHI 边界乘一次 alpha。
        // 红绿为零，蓝色乘 alpha 后与 alpha 通道相同。
        Some(RhiColor::from_premultiplied_rgba([0.0, 0.0, alpha, alpha]))
    );
    // 第二段只保留 clear 后的 shape。
    assert_eq!(lowered.segments[1].operations.len(), 1);
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

// Additive 填充载荷的逻辑 clip 必须按设备缩放成为物理 RHI scissor。
#[test]
fn additive_fill_clip_lowers_to_scaled_scissor() {
    // 创建一个具有明确逻辑尺寸的测试编码器。
    let Ok(mut encoder) = FrameEncoder::new(12, 8) else {
        // 合法测试尺寸必须可构造。
        panic!("test encoder dimensions are valid");
    };
    // 记录一个大于局部裁剪范围的 Additive 实心矩形。
    encoder.native(FrameRasterOp::FillRectAdditive {
        // 逻辑几何在 2 倍缩放后应成为 (4, 2, 16, 12)。
        rect: FrameRect::new(2, 1, 8, 6),
        // 使用可辨识的绿色源色。
        color: Color::green(),
        // 逻辑裁剪在 2 倍缩放后应成为 (6, 4, 8, 6)。
        clip: FrameRect::new(3, 2, 4, 3),
    });
    // 以 2 倍设备缩放执行只读 lowering。
    let lowered = match super::lower_frame_encoder(
        &encoder,
        RhiViewport {
            // viewport 使用对应的物理宽度。
            width: 24.0,
            // viewport 使用对应的物理高度。
            height: 16.0,
        },
        2.0,
        2.0,
    ) {
        // 合法 Additive shape 必须完整降低。
        Ok(Some(lowered)) => lowered,
        // 该固定整数几何不允许被判为不可表示。
        Ok(None) => panic!("clipped additive fill should be representable"),
        // 只读 lowering 不允许失败。
        Err(error) => panic!("clipped additive fill lowering should not fail: {error:?}"),
    };
    // 无 scroll 时应只产生一个 painter-order 片段。
    let [segment] = lowered.segments.as_slice() else {
        // 额外片段说明普通 shape 被错误拆分。
        panic!("expected one lowered segment");
    };
    // 唯一操作必须进入 Additive shape pipeline。
    let [RhiOp::AdditiveShape(shape)] = segment.operations.as_slice() else {
        // 普通 shape 或额外操作都会丢失目标相关 blend 事实。
        panic!("expected one additive shape operation");
    };
    // 物理几何必须按两个轴的设备比例缩放。
    assert_eq!((shape.x, shape.y, shape.w, shape.h), (4.0, 2.0, 16.0, 12.0));
    // 局部逻辑裁剪必须保留为一个非空物理 scissor。
    let Some(scissor) = shape.scissor else {
        // 丢失 scissor 会让 Additive 写入裁剪外目标。
        panic!("additive shape should retain a physical scissor");
    };
    // scissor 坐标与范围必须精确反映 2 倍缩放。
    assert_eq!(
        (scissor.x, scissor.y, scissor.width, scissor.height),
        (6, 4, 8, 6)
    );
}

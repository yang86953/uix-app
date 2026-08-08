// FrameRecordingCanvas Additive 全局 opacity 回归测试。

// 有限 opacity 必须折叠进 Native shape 颜色，并保留局部裁剪与参考像素语义。
#[test]
fn additive_shapes_fold_finite_opacity_into_native_colors() {
    // 创建能够容纳填充、圆角填充与描边的录制画布。
    let mut canvas = FrameRecordingCanvas::new(16, 8);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸的记录初始化不得失败。
        panic!("opacity additive recording should begin: {error:?}");
    }
    // 先用不透明红色建立可观察的累计目标。
    canvas.fill_rect(Rect::new(0.0, 0.0, 16.0, 8.0), Color::red(), None);
    // 后续 Additive shape 统一使用一半全局 opacity。
    canvas.set_opacity(0.5);
    // 设置局部裁剪，验证颜色折叠不会改变 surface-space clip。
    canvas.push_clip(Rect::new(1.0, 1.0, 4.0, 4.0));
    // 切换到目标相关 Additive 混合。
    canvas.set_blend_mode(BlendMode::Additive);
    // 记录一个右侧超出 clip 的普通填充矩形。
    canvas.fill_rect(Rect::new(1.0, 1.0, 5.0, 4.0), Color::green(), None);
    // 恢复完整裁剪以隔离圆角和描边颜色事实。
    canvas.pop_clip();
    // 记录一个通过圆角 shape 表达的半透明 Additive 圆。
    canvas.fill_circle(8.0, 3.0, 2.0, Color::green());
    // 记录一个像素边界明确的半透明 Additive 描边矩形。
    canvas.stroke_rect(Rect::new(11.0, 1.0, 4.0, 4.0), Color::green(), 1.0, None);
    // 完成记录并取得不可变命令流。
    let encoder = match canvas.finish_recording() {
        // 保存成功的编码器供命令与像素审计。
        Ok(encoder) => encoder,
        // 合法有限 opacity 不应产生 deferred failure。
        Err(error) => panic!("opacity additive recording should finish: {error:?}"),
    };
    // 精确匹配 clear、底色、普通填充、圆角填充和描边五段命令。
    let [FrameCommand::Clear { .. }, FrameCommand::Native {
        operation: FrameRasterOp::FillRect { .. },
    }, FrameCommand::Native {
        operation:
            FrameRasterOp::FillRectAdditive {
                color: rect_color,
                clip: rect_clip,
                ..
            },
    }, FrameCommand::Native {
        operation:
            FrameRasterOp::FillRoundedRectAdditive {
                color: circle_color,
                ..
            },
    }, FrameCommand::Native {
        operation:
            FrameRasterOp::StrokeRoundedRects {
                strokes,
                additive: true,
                ..
            },
    }] = encoder.commands()
    else {
        // CPU segment、漏掉 shape 或错误顺序都会破坏这一命令事实。
        panic!("expected opacity-folded additive shape commands");
    };
    // 使用 CPU rasterizer 的同源函数构造位精确 premultiplied 颜色。
    let expected_color =
        crate::draw::raster::rasterizer::color_with_premultiplied_opacity(Color::green(), 0.5);
    // 普通填充命令必须携带折叠后的颜色。
    assert_eq!(*rect_color, expected_color);
    // 圆角填充命令必须携带同一折叠颜色。
    assert_eq!(*circle_color, expected_color);
    // 描边调用只应产生一条共享 shape 载荷。
    assert_eq!(strokes.len(), 1);
    // 描边颜色也必须采用同一 premultiplied 折叠顺序。
    assert_eq!(strokes[0].color(), expected_color);
    // 局部 clip 必须保持原始 surface-space 整数矩形。
    assert_eq!(*rect_clip, FrameRect::new(1, 1, 4, 4));
    // 执行 CPU 参考路径以核验目标相关加法的真实像素。
    let reference = encoder.render_reference();
    // 红底与一半绿色相加后应保留 127 的绿色 premultiplied 通道。
    assert_eq!(reference.pixel(2, 2), Some(0xffff_7f00));
    // 填充几何内但 clip 外的像素必须保持原始红色。
    assert_eq!(reference.pixel(5, 2), Some(Color::red().premultiplied()));
    // 圆心附近的完全覆盖像素应采用同一半透明加法结果。
    assert_eq!(reference.pixel(8, 3), Some(0xffff_7f00));
    // 一像素直角描边的左上像素也应采用同一半透明加法结果。
    assert_eq!(reference.pixel(11, 1), Some(0xffff_7f00));
}

// 量化为全透明的有限 opacity 应 no-op，非有限 opacity 仍应稳定拒绝。
#[test]
fn additive_shape_noops_zero_opacity_and_rejects_non_finite_opacity() {
    // 创建用于验证全透明 no-op 的独立画布。
    let mut transparent = FrameRecordingCanvas::new(8, 8);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = transparent.begin_recording(true) {
        // 合法尺寸的记录初始化不得失败。
        panic!("transparent additive recording should begin: {error:?}");
    }
    // 把全局 opacity 设为有限零值。
    transparent.set_opacity(0.0);
    // 切换到目标相关 Additive 混合。
    transparent.set_blend_mode(BlendMode::Additive);
    // 普通填充量化为全透明后不得再验证无贡献的非法圆角。
    transparent.fill_rect(
        Rect::new(1.0, 1.0, 3.0, 3.0),
        Color::green(),
        Some(Radius::uniform(f32::NAN)),
    );
    // 描边也必须在非法圆角或宽度验证前复用同一透明 no-op 边界。
    transparent.stroke_rect(
        Rect::new(4.0, 1.0, 3.0, 3.0),
        Color::green(),
        f32::NAN,
        Some(Radius::uniform(f32::NAN)),
    );
    // 合法 no-op 场景必须能够正常完成记录。
    let transparent_encoder = match transparent.finish_recording() {
        // 保存成功编码器以审计命令数量。
        Ok(encoder) => encoder,
        // 有限零 opacity 不应产生 deferred failure。
        Err(error) => panic!("transparent additive recording should finish: {error:?}"),
    };
    // 透明 shape 不 flush、不产生命令，因此只保留初始 clear。
    assert!(matches!(
        transparent_encoder.commands(),
        [FrameCommand::Clear { .. }]
    ));

    // 创建用于验证非有限 opacity 拒绝边界的独立画布。
    let mut non_finite = FrameRecordingCanvas::new(8, 8);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = non_finite.begin_recording(true) {
        // 合法尺寸的记录初始化不得失败。
        panic!("non-finite additive recording should begin: {error:?}");
    }
    // 注入无法稳定编码到 Native 颜色的 NaN opacity。
    non_finite.set_opacity(f32::NAN);
    // 切换到目标相关 Additive 混合。
    non_finite.set_blend_mode(BlendMode::Additive);
    // 尝试记录 otherwise 合法的整数矩形。
    non_finite.fill_rect(Rect::new(1.0, 1.0, 3.0, 3.0), Color::green(), None);
    // 完成边界必须返回稳定的 NotImplemented typed failure。
    let error = match non_finite.finish_recording() {
        // 保存错误以核对类型边界。
        Err(error) => error,
        // 成功会把非有限 opacity 错误提升为 Native shape。
        Ok(_) => panic!("non-finite additive opacity must be rejected"),
    };
    // 拒绝原因必须保持在不能等价 lowering 的类型边界。
    assert_eq!(error.code(), crate::core::Errc::NotImplemented);
    // 失败前只能保留初始 clear，不能偷偷追加 Native 或 CPU segment。
    assert_eq!(
        non_finite
            .encoder
            .as_ref()
            .map(|encoder| encoder.commands().len()),
        Some(1)
    );
}

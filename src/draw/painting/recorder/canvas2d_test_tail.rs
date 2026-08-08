// FrameRecordingCanvas Additive 纯平移 transform 回归测试。

// 有限整数纯平移 transform 必须在 offset 之后映射几何，且不重复移动 clip。
#[test]
fn additive_shapes_map_integral_translation_transform_after_offset() {
    // 创建能够容纳组合正负平移后几何的录制画布。
    let mut canvas = FrameRecordingCanvas::new(16, 10);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸的记录初始化不得失败。
        panic!("translated additive recording should begin: {error:?}");
    }
    // 先用不透明红色建立可观察的累计目标。
    canvas.fill_rect(Rect::new(0.0, 0.0, 16.0, 10.0), Color::red(), None);
    // 软件映射先应用 (1,-1) 像素 offset。
    canvas.set_offset(1.0, -1.0);
    // 再应用 (2,2) 纯平移 transform，组合位移为 (3,1)。
    canvas.set_transform(Transform::translate(2.0, 2.0));
    // 局部 clip 应按同一顺序只映射一次到 surface 坐标 (4,2,3,4)。
    canvas.push_clip(Rect::new(1.0, 1.0, 3.0, 4.0));
    // 后续填充与描边切换到目标相关 Additive 混合。
    canvas.set_blend_mode(BlendMode::Additive);
    // 本地圆形边界 (1,1,4,4) 应映射为 surface 矩形 (4,2,4,4)。
    canvas.fill_circle(3.0, 3.0, 2.0, Color::green());
    // 恢复完整 surface clip，隔离后一条负平移命令。
    canvas.pop_clip();
    // 清除像素 offset，让后一条命令只验证负 transform 平移。
    canvas.set_offset(0.0, 0.0);
    // 应用 (-2,-1) 纯平移 transform。
    canvas.set_transform(Transform::translate(-2.0, -1.0));
    // 本地矩形 (4,6,4,3) 应映射为 surface 矩形 (2,5,4,3)。
    canvas.stroke_rect(Rect::new(4.0, 6.0, 4.0, 3.0), Color::green(), 1.0, None);
    // 完成记录并取得不可变命令流。
    let encoder = match canvas.finish_recording() {
        // 保存成功的编码器供载荷和像素审计。
        Ok(encoder) => encoder,
        // 合法纯平移 transform 不应产生 deferred failure。
        Err(error) => panic!("translated additive recording should finish: {error:?}"),
    };
    // 精确匹配命令序列，同时证明两条 Additive 操作都没有进入 CPU segment。
    let [FrameCommand::Clear { .. }, FrameCommand::Native {
        operation: FrameRasterOp::FillRect { .. },
    }, FrameCommand::Native {
        operation:
            FrameRasterOp::FillRoundedRectAdditive {
                rect: fill_rect,
                clip: fill_clip,
                ..
            },
    }, FrameCommand::Native {
        operation:
            FrameRasterOp::StrokeRoundedRects {
                strokes,
                clip: stroke_clip,
                additive: true,
            },
    }] = encoder.commands()
    else {
        // CPU segment、错误几何或错误批次都会破坏这一精确事实。
        panic!("expected translated additive fill and stroke commands");
    };
    // offset 与正 transform 必须按软件顺序合成到圆的正方形几何。
    assert_eq!(*fill_rect, FrameRect::new(4, 2, 4, 4));
    // 当前 clip 已是 surface 坐标，必须保持 (4,2,3,4) 而不能再次平移。
    assert_eq!(*fill_clip, FrameRect::new(4, 2, 3, 4));
    // 负 transform 下当前调用只应产生一条描边。
    assert_eq!(strokes.len(), 1);
    // 负 transform 必须把本地描边矩形平移到 surface 左上侧。
    assert_eq!(strokes[0].rect(), FrameRect::new(2, 5, 4, 3));
    // pop_clip 后的描边必须恢复完整 surface 裁剪。
    assert_eq!(*stroke_clip, FrameRect::new(0, 0, 16, 10));
    // 执行 CPU 参考路径以核验组合平移和裁剪后的真实目标像素。
    let reference = encoder.render_reference();
    // 组合正平移后裁剪内的圆形像素应由红绿相加得到黄色。
    assert_eq!(
        reference.pixel(6, 3),
        Some(Color::from_rgb(255, 255, 0).premultiplied())
    );
    // x=7 位于圆内但在 surface-space clip 外，必须保持原始红色。
    assert_eq!(reference.pixel(7, 3), Some(Color::red().premultiplied()));
    // 负 transform 后的描边左上像素也应对红色目标执行加法。
    assert_eq!(
        reference.pixel(2, 5),
        Some(Color::from_rgb(255, 255, 0).premultiplied())
    );
}

// 单位正交 transform 必须无损映射统一圆角填充与直角描边。
#[test]
fn additive_shapes_map_unit_orthogonal_transforms() {
    // 创建能够同时容纳旋转圆与镜像描边的录制画布。
    let mut canvas = FrameRecordingCanvas::new(16, 12);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸的记录初始化不得失败。
        panic!("orthogonal additive recording should begin: {error:?}");
    }
    // 先用不透明红色建立可观察的累计目标。
    canvas.fill_rect(Rect::new(0.0, 0.0, 16.0, 12.0), Color::red(), None);
    // 软件映射先应用一个正整数水平 offset。
    canvas.set_offset(1.0, 0.0);
    // 应用带整数平移的顺时针四分之一转单位正交矩阵。
    canvas.set_transform(Transform {
        m: [0.0, -1.0, 10.0, 1.0, 0.0, 1.0],
    });
    // 局部 clip 应按相同矩阵只映射一次到 surface 坐标。
    canvas.push_clip(Rect::new(1.0, 1.0, 4.0, 5.0));
    // 后续图元切换到目标相关 Additive 混合。
    canvas.set_blend_mode(BlendMode::Additive);
    // 本地圆形边界经 offset 和旋转后应保持 4×4 正方形与统一半径。
    canvas.fill_circle(3.0, 3.0, 2.0, Color::green());
    // 恢复完整 surface clip，隔离后一条镜像命令。
    canvas.pop_clip();
    // 镜像命令不再叠加前一条像素 offset。
    canvas.set_offset(0.0, 0.0);
    // 应用带整数平移的水平镜像单位正交矩阵。
    canvas.set_transform(Transform {
        m: [-1.0, 0.0, 15.0, 0.0, 1.0, 0.0],
    });
    // 本地直角描边应镜像到 surface 右侧且保持线宽不变。
    canvas.stroke_rect(Rect::new(2.0, 7.0, 4.0, 3.0), Color::green(), 1.0, None);
    // 完成记录并取得不可变命令流。
    let encoder = match canvas.finish_recording() {
        // 保存成功编码器供命令与像素审计。
        Ok(encoder) => encoder,
        // 合法单位正交 transform 不应产生 deferred failure。
        Err(error) => panic!("orthogonal additive recording should finish: {error:?}"),
    };
    // 精确匹配命令序列，证明旋转与镜像均未进入 CPU segment。
    let [FrameCommand::Clear { .. }, FrameCommand::Native {
        operation: FrameRasterOp::FillRect { .. },
    }, FrameCommand::Native {
        operation:
            FrameRasterOp::FillRoundedRectAdditive {
                rect: fill_rect,
                radius,
                clip: fill_clip,
                ..
            },
    }, FrameCommand::Native {
        operation:
            FrameRasterOp::StrokeRoundedRects {
                strokes,
                clip: stroke_clip,
                additive: true,
            },
    }] = encoder.commands()
    else {
        // 任何 CPU segment 或错误批次都说明正交准入发生退化。
        panic!("expected orthogonal additive fill and stroke commands");
    };
    // 旋转后的圆边界必须映射为 surface 矩形 (5,3,4,4)。
    assert_eq!(*fill_rect, FrameRect::new(5, 3, 4, 4));
    // 圆的统一半径在单位正交变换后必须保持为 2 像素。
    assert_eq!(radius.to_radius(), Radius::uniform(2.0));
    // 局部 clip 必须只映射一次并保存为 surface 矩形 (4,3,5,4)。
    assert_eq!(*fill_clip, FrameRect::new(4, 3, 5, 4));
    // 镜像命令只应产生一条描边。
    assert_eq!(strokes.len(), 1);
    // 水平镜像必须把本地描边映射为 surface 矩形 (9,7,4,3)。
    assert_eq!(strokes[0].rect(), FrameRect::new(9, 7, 4, 3));
    // pop_clip 后的镜像描边必须恢复完整 surface 裁剪。
    assert_eq!(*stroke_clip, FrameRect::new(0, 0, 16, 12));
    // 执行 CPU 参考路径以核验目标相关像素语义。
    let reference = encoder.render_reference();
    // 旋转圆心应对红色目标叠加绿色并得到黄色。
    assert_eq!(
        reference.pixel(7, 5),
        Some(Color::from_rgb(255, 255, 0).premultiplied())
    );
    // 镜像描边左上像素也应执行同一加法混合。
    assert_eq!(
        reference.pixel(9, 7),
        Some(Color::from_rgb(255, 255, 0).premultiplied())
    );
}

// 非统一圆角不能在旋转后沿用未重排的角载荷。
#[test]
fn additive_shape_rejects_non_uniform_radius_under_rotation() {
    // 创建足以容纳待拒绝圆角矩形的录制画布。
    let mut canvas = FrameRecordingCanvas::new(8, 8);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸的记录初始化不得失败。
        panic!("non-uniform radius recording should begin: {error:?}");
    }
    // 应用能够把测试几何保留在 surface 内的四分之一转矩阵。
    canvas.set_transform(Transform {
        m: [0.0, -1.0, 7.0, 1.0, 0.0, 0.0],
    });
    // 选择目标相关 Additive 混合。
    canvas.set_blend_mode(BlendMode::Additive);
    // 提供旋转后必须重排但当前命令模型尚未处理的四个不同角半径。
    let radius = Radius {
        tl: 0.0,
        tr: 1.0,
        br: 2.0,
        bl: 3.0,
    };
    // 尝试记录 otherwise 合法的整数圆角矩形。
    canvas.fill_rect(Rect::new(1.0, 1.0, 2.0, 3.0), Color::green(), Some(radius));
    // 完成边界必须返回稳定的 NotImplemented typed failure。
    let error = match canvas.finish_recording() {
        // 错误结果就是本测试需要审计的门禁事实。
        Err(error) => error,
        // 成功会错误复用旋转前的角半径顺序。
        Ok(_) => panic!("rotated non-uniform additive radius must be rejected"),
    };
    // 拒绝原因必须保持在不能等价 lowering 的类型边界。
    assert_eq!(error.code(), crate::core::Errc::NotImplemented);
    // 失败前只能保留初始 clear，不能追加 Native 或 CPU segment。
    assert_eq!(
        canvas
            .encoder
            .as_ref()
            .map(|encoder| encoder.commands().len()),
        Some(1)
    );
}

// 分数平移与非平移 transform 都不能伪装成整数轴对齐 shape。
#[test]
fn additive_shape_rejects_fractional_or_scaled_transform() {
    // 依次覆盖分数纯平移与轴对齐缩放两个拒绝分支。
    for transform in [Transform::translate(0.5, 0.0), Transform::scale(2.0, 1.0)] {
        // 为每个拒绝场景创建独立录制画布。
        let mut canvas = FrameRecordingCanvas::new(8, 8);
        // 开始一帧带透明 clear 的正式记录。
        if let Err(error) = canvas.begin_recording(true) {
            // 合法尺寸的记录初始化不得失败。
            panic!("rejected transform recording should begin: {error:?}");
        }
        // 设置当前待验证的 transform。
        canvas.set_transform(transform);
        // 选择目标相关 Additive 混合。
        canvas.set_blend_mode(BlendMode::Additive);
        // 尝试记录 otherwise 合法的整数矩形。
        canvas.fill_rect(Rect::new(1.0, 1.0, 2.0, 2.0), Color::green(), None);
        // 完成边界必须返回稳定的 NotImplemented typed failure。
        let error = match canvas.finish_recording() {
            // 错误结果就是本测试需要审计的门禁事实。
            Err(error) => error,
            // 成功会把不支持的 transform 错误提升到整数 shape。
            Ok(_) => panic!("unsupported additive transform must be rejected"),
        };
        // 拒绝原因必须保持在不能等价 lowering 的类型边界。
        assert_eq!(error.code(), crate::core::Errc::NotImplemented);
        // 失败前只能保留初始 clear，不能偷偷追加 Native 或 CPU segment。
        assert_eq!(
            canvas
                .encoder
                .as_ref()
                .map(|encoder| encoder.commands().len()),
            Some(1)
        );
    }
}

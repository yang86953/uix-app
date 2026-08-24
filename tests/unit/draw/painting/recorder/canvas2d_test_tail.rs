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

// 八种单位正交线性变换都必须把非统一圆角映射到正确的 surface 角位。
#[test]
fn additive_shape_reorders_radius_for_all_unit_orthogonal_transforms() {
    // 使用四个不同数值，使每个源角在重排后都可唯一识别。
    let radius = Radius {
        // 左上角使用一。
        tl: 1.0,
        // 右上角使用二。
        tr: 2.0,
        // 右下角使用三。
        br: 3.0,
        // 左下角使用四。
        bl: 4.0,
    };
    // 列出 D4 中全部有符号轴置换及其期望 tl/tr/br/bl 顺序。
    let cases = [
        // 恒等变换不改变角位。
        ([1.0, 0.0, 0.0, 1.0], [1.0, 2.0, 3.0, 4.0]),
        // 水平镜像交换左右角。
        ([-1.0, 0.0, 0.0, 1.0], [2.0, 1.0, 4.0, 3.0]),
        // 垂直镜像交换上下角。
        ([1.0, 0.0, 0.0, -1.0], [4.0, 3.0, 2.0, 1.0]),
        // 半周旋转交换对角。
        ([-1.0, 0.0, 0.0, -1.0], [3.0, 4.0, 1.0, 2.0]),
        // 主对角镜像交换右上与左下。
        ([0.0, 1.0, 1.0, 0.0], [1.0, 4.0, 3.0, 2.0]),
        // 顺时针四分之一转依次推进四个角。
        ([0.0, -1.0, 1.0, 0.0], [4.0, 1.0, 2.0, 3.0]),
        // 逆时针四分之一转按反方向推进四个角。
        ([0.0, 1.0, -1.0, 0.0], [2.0, 3.0, 4.0, 1.0]),
        // 副对角镜像交换左上与右下。
        ([0.0, -1.0, -1.0, 0.0], [3.0, 2.0, 1.0, 4.0]),
    ];
    // 逐一验证全部八种合法线性部分。
    for ([a, b, c, d], expected) in cases {
        // 调用生产准入路径共用的角位重排函数。
        let mapped = FrameRecordingCanvas::unit_orthogonal_radius(radius, a, b, c, d);
        // 比较公开 Radius 顺序，避免仅验证某个代表变换。
        assert_eq!([mapped.tl, mapped.tr, mapped.br, mapped.bl], expected);
    }
}

// 填充与描边必须把非统一圆角重排后直接编码，并保持目标相关参考像素。
#[test]
fn additive_shapes_record_reordered_non_uniform_radius() {
    // 创建足以容纳旋转填充与镜像描边的录制画布。
    let mut canvas = FrameRecordingCanvas::new(12, 10);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸的记录初始化不得失败。
        panic!("non-uniform radius recording should begin: {error:?}");
    }
    // 先用红色建立可观察的累计目标。
    canvas.fill_rect(Rect::new(0.0, 0.0, 12.0, 10.0), Color::red(), None);
    // 后续两个圆角图元都选择目标相关 Additive 混合。
    canvas.set_blend_mode(BlendMode::Additive);
    // 设置带整数平移的顺时针四分之一转。
    canvas.set_transform(Transform {
        // 该矩阵把本地矩形映射到 surface 内的整数 AABB。
        m: [0.0, -1.0, 8.0, 1.0, 0.0, 1.0],
    });
    // 记录四角不同的旋转圆角填充。
    canvas.fill_rect(
        // 宽高在轴交换后应互换。
        Rect::new(1.0, 1.0, 3.0, 4.0),
        // 绿色用于与红色目标形成黄色参考像素。
        Color::green(),
        // 顺时针旋转后应得到 bl/tl/tr/br 顺序。
        Some(Radius {
            // 原左上半径为零。
            tl: 0.0,
            // 原右上半径为半像素。
            tr: 0.5,
            // 原右下半径为一像素。
            br: 1.0,
            // 原左下半径为一点五像素。
            bl: 1.5,
        }),
    );
    // 改用带整数平移的水平镜像。
    canvas.set_transform(Transform {
        // 镜像后的描边仍完整落在 surface 内。
        m: [-1.0, 0.0, 11.0, 0.0, 1.0, 0.0],
    });
    // 记录共享同一重排逻辑的非统一圆角描边。
    canvas.stroke_rect(
        // 选择与填充不相交的本地矩形。
        Rect::new(1.0, 6.0, 4.0, 3.0),
        // 继续使用绿色观察 Additive 结果。
        Color::green(),
        // 使用可直接编码的一像素线宽。
        1.0,
        // 水平镜像后应交换左右圆角。
        Some(Radius {
            // 原左上半径为零。
            tl: 0.0,
            // 原右上半径为半像素。
            tr: 0.5,
            // 原右下半径为一像素。
            br: 1.0,
            // 原左下半径为一点五像素。
            bl: 1.5,
        }),
    );
    // 完成记录并取得不可变命令流。
    let encoder = match canvas.finish_recording() {
        // 保存成功编码器供命令与参考像素审计。
        Ok(encoder) => encoder,
        // 合法单位正交圆角不应产生 deferred failure。
        Err(error) => panic!("non-uniform radius recording should finish: {error:?}"),
    };
    // 精确匹配 clear、背景、圆角填充和圆角描边，排除 CPU segment。
    let [FrameCommand::Clear { .. }, FrameCommand::Native {
        operation: FrameRasterOp::FillRect { .. },
    }, FrameCommand::Native {
        operation:
            FrameRasterOp::FillRoundedRectAdditive {
                rect: fill_rect,
                radius: fill_radius,
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
        // 任一 CPU 回退或错误命令类型都应使测试失败。
        panic!("expected reordered additive rounded shape commands");
    };
    // 四分之一转必须交换矩形宽高并保存 surface 几何。
    assert_eq!(*fill_rect, FrameRect::new(3, 2, 4, 3));
    // 填充半径必须按 bl/tl/tr/br 重排。
    assert_eq!(
        fill_radius.to_radius(),
        Radius {
            // 原左下角映射到左上角。
            tl: 1.5,
            // 原左上角映射到右上角。
            tr: 0.0,
            // 原右上角映射到右下角。
            br: 0.5,
            // 原右下角映射到左下角。
            bl: 1.0,
        }
    );
    // 当前调用只应产生一条镜像描边。
    assert_eq!(strokes.len(), 1);
    // 水平镜像必须保存归一化后的 surface 描边矩形。
    assert_eq!(strokes[0].rect(), FrameRect::new(6, 6, 4, 3));
    // 描边半径必须交换左右角位。
    assert_eq!(
        strokes[0].radius().to_radius(),
        Radius {
            // 原右上角映射到左上角。
            tl: 0.5,
            // 原左上角映射到右上角。
            tr: 0.0,
            // 原左下角映射到右下角。
            br: 1.5,
            // 原右下角映射到左下角。
            bl: 1.0,
        }
    );
    // 执行 CPU 参考路径以验证目标相关混合语义未因角位重排改变。
    let reference = encoder.render_reference();
    // 旋转圆角填充中央像素必须由红绿相加得到黄色。
    assert_eq!(
        reference.pixel(5, 3),
        Some(Color::from_rgb(255, 255, 0).premultiplied())
    );
    // 镜像圆角描边顶部中央像素必须按半覆盖对目标执行 Additive 混合。
    assert_eq!(
        reference.pixel(8, 6),
        Some(Color::from_rgb(255, 128, 0).premultiplied())
    );
}

// 分数平移与缩放无法进入固定 Native shape 时必须改走 Additive sampled segment。
#[test]
fn additive_stroke_fractional_or_scaled_transform_uses_sampled_segment() {
    // 依次覆盖分数纯平移与轴对齐缩放两个描边 fallback 分支。
    for transform in [Transform::translate(0.5, 0.0), Transform::scale(2.0, 1.0)] {
        // 为每个仿射场景创建独立录制画布。
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
        // 记录 otherwise 合法的一像素矩形描边。
        canvas.stroke_rect(
            // 使用完整位于 surface 内的本地矩形。
            Rect::new(1.0, 1.0, 2.0, 2.0),
            // 颜色不影响 transform 门禁。
            Color::green(),
            // 使用合法正有限线宽。
            1.0,
            // 直角描边排除圆角载荷干扰。
            None,
        );
        // 完成记录并取得 sampled 命令流。
        let encoder = match canvas.finish_recording() {
            // 合法仿射描边应由软件路径保真处理。
            Ok(encoder) => encoder,
            // typed failure 表示新 fallback 没有覆盖当前变换。
            Err(error) => panic!("additive stroke transform should finish: {error:?}"),
        };
        // 不可固定编码的描边只能形成 Additive PictureBlit，禁止 CpuSegment。
        assert!(matches!(
            encoder.commands(),
            [
                FrameCommand::Clear { .. },
                FrameCommand::PictureBlit { additive: true, .. }
            ]
        ));
        // 软件描边必须实际产生可见像素，不能因忽略 transform 被扫描边界裁空。
        assert!(encoder
            .render_reference()
            .pixels()
            .iter()
            .any(|pixel| pixel & 0xff00_0000 != 0));
    }
}

// 稳定场景应复用上一已执行帧的同一命令分配，同时保持像素结果一致。
#[test]
fn recording_reuses_recycled_command_allocation() {
    // 创建独立录制画布并完成首帧。
    let mut canvas = FrameRecordingCanvas::new(12, 8);
    canvas
        .begin_recording(true)
        .expect("first recording should begin");
    canvas.fill_rect(Rect::new(2.0, 1.0, 5.0, 4.0), Color::green(), None);
    let first = canvas
        .finish_recording()
        .expect("first recording should finish");
    let first_command_count = first.commands().len();
    assert!(first_command_count > 0);
    let first_capacity = first.command_capacity();
    let first_storage = first.commands().as_ptr();
    let first_pixels = first.render_reference().pixels().to_vec();
    // 同步消费完成后把 encoder 归还给唯一 recorder owner。
    canvas.recycle_encoder(first);

    // 第二帧应直接取得同一 Vec 分配，而不是按规模提示重新申请。
    canvas
        .begin_recording(false)
        .expect("second recording should begin");
    let recycled = canvas.encoder.as_ref().expect("second encoder should exist");
    assert_eq!(recycled.command_capacity(), first_capacity);
    assert_eq!(recycled.commands().as_ptr(), first_storage);
    canvas.fill_rect(Rect::new(2.0, 1.0, 5.0, 4.0), Color::green(), None);
    let second = canvas
        .finish_recording()
        .expect("second recording should finish");

    // 容量复用不得改变相同绘制在透明目标上的参考像素。
    assert_eq!(
        first_pixels,
        second.render_reference().pixels()
    );
}

// 稳定文字录制应复用上一帧字形批次，不再逐字形申请临时 Vec。
#[test]
fn recording_reuses_recycled_glyph_batch_allocation() {
    let mut canvas = FrameRecordingCanvas::new(32, 8);
    let coverage: std::sync::Arc<[u8]> = std::sync::Arc::from([u8::MAX]);
    canvas
        .begin_recording(false)
        .expect("first glyph recording should begin");
    for x in 0..6 {
        canvas.blit_glyph_shared(x, 0, std::sync::Arc::clone(&coverage), 1, 1, Color::white());
    }
    let first = canvas
        .finish_recording()
        .expect("first glyph recording should finish");
    let [
        FrameCommand::Native {
            operation: FrameRasterOp::BlitGlyphs { glyphs, .. },
        },
    ] = first.commands()
    else {
        panic!("expected one glyph batch");
    };
    let first_storage = glyphs.as_ptr();
    let first_pixels = first.render_reference().pixels().to_vec();
    canvas.recycle_encoder(first);

    canvas
        .begin_recording(false)
        .expect("second glyph recording should begin");
    for x in 0..6 {
        canvas.blit_glyph_shared(x, 0, std::sync::Arc::clone(&coverage), 1, 1, Color::white());
    }
    let second = canvas
        .finish_recording()
        .expect("second glyph recording should finish");
    let [
        FrameCommand::Native {
            operation: FrameRasterOp::BlitGlyphs { glyphs, .. },
        },
    ] = second.commands()
    else {
        panic!("expected one recycled glyph batch");
    };
    assert_eq!(glyphs.as_ptr(), first_storage);
    assert_eq!(second.render_reference().pixels(), first_pixels);
}

// 文字场景骤减后不应长期驻留上一峰值字形槽位。
#[test]
fn recording_drops_oversized_recycled_glyph_allocation_after_shrink() {
    let mut canvas = FrameRecordingCanvas::new(512, 8);
    let coverage: std::sync::Arc<[u8]> = std::sync::Arc::from([u8::MAX]);
    canvas
        .begin_recording(false)
        .expect("large glyph recording should begin");
    for x in 0..256 {
        canvas.blit_glyph_shared(x, 0, std::sync::Arc::clone(&coverage), 1, 1, Color::white());
    }
    let large = canvas
        .finish_recording()
        .expect("large glyph recording should finish");
    canvas.recycle_encoder(large);
    let large_retained = canvas
        .spare_encoder
        .as_ref()
        .expect("large encoder should be retained")
        .retained_memory_usage();

    canvas
        .begin_recording(false)
        .expect("empty recording should begin");
    let empty = canvas
        .finish_recording()
        .expect("empty recording should finish");
    canvas.recycle_encoder(empty);
    let shrunk_retained = canvas
        .spare_encoder
        .as_ref()
        .expect("empty encoder should be retained")
        .retained_memory_usage();
    assert!(shrunk_retained < large_retained);
}

// 峰值命令容量不得在场景骤减后无界驻留。
#[test]
fn recording_drops_oversized_recycled_command_allocation() {
    let mut canvas = FrameRecordingCanvas::new(12, 8);
    let oversized = crate::draw::painting::FrameEncoder::with_command_capacity(12, 8, 1024)
        .expect("oversized test encoder should be valid");

    canvas.recycle_encoder(oversized);

    assert!(canvas.spare_encoder.is_none());
    assert_eq!(canvas.command_capacity_hint, 0);
}

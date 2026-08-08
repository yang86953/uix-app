// Additive 仿射描边必须与填充共享一个紧边界 sampled source 批次。
#[test]
fn additive_affine_strokes_batch_with_fill_into_one_sampled_segment() {
    // 引入路径端点与连接样式。
    use crate::draw::geometry::path::{LineCap, LineJoin, PathBuilder};

    // 创建可容纳所有变换后图元且能证明 tile 紧缩的画布。
    let mut canvas = FrameRecordingCanvas::new(44, 24);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("additive affine stroke recording should begin: {error:?}");
    }
    // 先用红色建立可观察的累计目标。
    canvas.fill_rect(Rect::new(0.0, 0.0, 44.0, 24.0), Color::red(), None);
    // 后续所有源贡献切换到目标相关 Additive。
    canvas.set_blend_mode(BlendMode::Additive);
    // 设置包含剪切和平移的可逆仿射，固定 Native shape 无法表达该状态。
    canvas.set_transform(Transform {
        // x' = x + 0.25y + 6，y' = 0.2x + y + 4。
        m: [1.0, 0.25, 6.0, 0.2, 1.0, 4.0],
    });
    // 记录带非统一圆角的绿色矩形描边。
    canvas.stroke_rect(
        // 本地矩形与其他图元保持分离。
        Rect::new(1.0, 1.0, 6.0, 5.0),
        // 绿色用于在 packed image 中识别该图元。
        Color::green(),
        // 两像素线宽确保存在完全覆盖像素。
        2.0,
        // 非统一圆角验证各角 SDF 仍在本地空间求值。
        Some(Radius {
            // 左上角半径。
            tl: 1.0,
            // 右上角半径。
            tr: 2.0,
            // 右下角半径。
            br: 0.5,
            // 左下角半径。
            bl: 1.5,
        }),
    );
    // 记录蓝色圆形描边，仿射后应成为剪切椭圆环。
    canvas.stroke_circle(13.0, 4.0, 3.0, Color::blue(), 2.0);
    // 构造带转角的开放路径。
    let mut builder = PathBuilder::new();
    // 设置路径起点。
    builder.move_to(19.0, 2.0);
    // 追加水平线段。
    builder.line_to(24.0, 2.0);
    // 追加垂直线段以产生 join。
    builder.line_to(24.0, 7.0);
    // 构建不可变路径。
    let path = builder.build();
    // 配置可观察的圆端点与斜角连接。
    let options = StrokeOptions {
        // 使用两像素本地线宽。
        width: 2.0,
        // 圆端点必须在生成本地轮廓时处理。
        cap: LineCap::Round,
        // 斜角连接避免与默认 miter 混淆。
        join: LineJoin::Bevel,
        // 有限 miter 上限仍随载荷保留。
        miter_limit: 3.0,
    };
    // 记录白色路径描边。
    canvas.stroke_path(&path, Color::white(), &options);
    // 构造用于识别直线的黄色。
    let yellow = Color::from_rgb(255, 255, 0);
    // 记录黄色斜线，覆盖独立的 draw_line 软件入口。
    canvas.draw_line(28.0, 2.0, 32.0, 7.0, yellow, 2.0);
    // 构造用于识别填充的青色。
    let cyan = Color::from_rgb(0, 255, 255);
    // 追加青色仿射椭圆填充，证明填充和描边共享同一 Additive source 批次。
    canvas.fill_ellipse(Rect::new(2.0, 12.0, 5.0, 4.0), cyan);
    // 完成记录并取得不可变命令流。
    let encoder = match canvas.finish_recording() {
        // 保存成功编码器供命令、tile 与像素审计。
        Ok(encoder) => encoder,
        // 可逆仿射描边不应产生 deferred failure。
        Err(error) => panic!("additive affine stroke recording should finish: {error:?}"),
    };
    // 精确匹配 clear、红底与单个 Additive sampled source tile。
    let [FrameCommand::Clear { .. }, FrameCommand::Native {
        operation: FrameRasterOp::FillRect { .. },
    }, FrameCommand::PictureBlit {
        image,
        src,
        dst,
        opacity,
        additive: true,
    }] = encoder.commands()
    else {
        // Native 误提升、CpuSegment 或拆成多个 tile 都会破坏本测试。
        panic!("expected one additive sampled segment for affine strokes and fill");
    };
    // scratch 已应用每笔 opacity，最终上传必须保持 opaque。
    assert!(opacity.is_opaque());
    // packed source 必须从图片原点开始且覆盖完整图片。
    assert_eq!(*src, FrameRect::new(0, 0, image.width(), image.height()));
    // sampled 目标坐标应证明仿射平移已经生效。
    assert!(dst.x() > 0.0 && dst.y() > 0.0);
    // 可见 tile 不得退化为完整 surface 上传。
    assert!(image.width() < 44 && image.height() < 24);
    // 图片中必须保留绿色矩形描边的完全覆盖像素。
    assert!(image
        .pixels()
        .iter()
        .any(|pixel| *pixel == Color::green().premultiplied()));
    // 图片中必须保留蓝色圆形描边的完全覆盖像素。
    assert!(image
        .pixels()
        .iter()
        .any(|pixel| *pixel == Color::blue().premultiplied()));
    // 图片中必须保留白色路径描边的完全覆盖像素。
    assert!(image
        .pixels()
        .iter()
        .any(|pixel| *pixel == Color::white().premultiplied()));
    // 图片中必须保留黄色直线描边的完全覆盖像素。
    assert!(image
        .pixels()
        .iter()
        .any(|pixel| *pixel == yellow.premultiplied()));
    // 图片中必须保留青色填充的完全覆盖像素。
    assert!(image
        .pixels()
        .iter()
        .any(|pixel| *pixel == cyan.premultiplied()));
    // 执行同一命令流的 CPU 参考路径。
    let reference = encoder.render_reference();
    // 变换前局部左上区域不能出现被忽略 transform 的绿色描边。
    assert_eq!(reference.pixel(1, 1), Some(Color::red().premultiplied()));
    // sampled tile 内至少一个像素必须改变红色累计目标。
    assert!(reference
        .pixels()
        .iter()
        .any(|pixel| *pixel != Color::red().premultiplied()));
}

// SrcOver 与 Additive 描边切换必须保持严格 painter order。
#[test]
fn additive_strokes_preserve_src_over_barriers() {
    // 引入路径构造器。
    use crate::draw::geometry::path::PathBuilder;

    // 创建左右分区清晰的顺序验证画布。
    let mut canvas = FrameRecordingCanvas::new(18, 8);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("additive stroke barrier recording should begin: {error:?}");
    }
    // 建立完整红色累计目标。
    canvas.fill_rect(Rect::new(0.0, 0.0, 18.0, 8.0), Color::red(), None);
    // 构造左侧水平开放路径。
    let mut builder = PathBuilder::new();
    // 设置路径起点。
    builder.move_to(2.0, 3.0);
    // 设置路径终点。
    builder.line_to(7.0, 3.0);
    // 构建不可变路径。
    let path = builder.build();
    // 使用两像素默认描边，产生稳定的内部覆盖像素。
    let options = StrokeOptions {
        // 指定线宽。
        width: 2.0,
        // 其余 cap、join 与 miter 沿用 canonical 默认值。
        ..StrokeOptions::default()
    };
    // 首个蓝色描边使用 SrcOver 并进入普通 CpuSegment。
    canvas.stroke_path(&path, Color::blue(), &options);
    // 切换到 Additive 时必须先封口普通 scratch。
    canvas.set_blend_mode(BlendMode::Additive);
    // 同一几何上的绿色描边进入独立 Additive sampled segment。
    canvas.stroke_path(&path, Color::green(), &options);
    // 切回 SrcOver 时必须先封口 Additive scratch。
    canvas.set_blend_mode(BlendMode::SrcOver);
    // 最后一条白色斜线进入新的普通 CpuSegment。
    canvas.draw_line(11.0, 2.0, 16.0, 6.0, Color::white(), 2.0);
    // 完成记录并取得不可变命令流。
    let encoder = match canvas.finish_recording() {
        // 保存成功编码器供顺序审计。
        Ok(encoder) => encoder,
        // 合法 blend barrier 不应失败。
        Err(error) => panic!("additive stroke barrier recording should finish: {error:?}"),
    };
    // 精确匹配 Native 红底、普通描边、Additive 描边与后置普通直线的顺序。
    assert!(matches!(
        encoder.commands(),
        [
            FrameCommand::Clear { .. },
            FrameCommand::Native {
                operation: FrameRasterOp::FillRect { .. },
            },
            FrameCommand::CpuSegment { .. },
            FrameCommand::PictureBlit { additive: true, .. },
            FrameCommand::CpuSegment { .. }
        ]
    ));
    // 执行同一命令流的 CPU 参考路径。
    let reference = encoder.render_reference();
    // 左侧路径必须先由蓝色覆盖，再加绿色得到青色。
    assert_eq!(
        reference.pixel(4, 3),
        Some(Color::from_rgb(0, 255, 255).premultiplied())
    );
    // 右侧后置白线必须保持最后的 SrcOver 结果。
    assert_eq!(reference.pixel(14, 4), Some(Color::white().premultiplied()));
}

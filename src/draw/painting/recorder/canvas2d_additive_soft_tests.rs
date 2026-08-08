// Additive 软件填充必须以目标相关 sampled segment 保留任意仿射与饱和加法。
#[test]
fn additive_affine_fills_batch_into_tight_sampled_segment() {
    // 引入路径构造器以覆盖任意仿射路径填充。
    use crate::draw::geometry::path::PathBuilder;

    // 创建能够容纳缩放椭圆、剪切路径和后续普通填充的画布。
    let mut canvas = FrameRecordingCanvas::new(16, 10);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("additive sampled fill recording should begin: {error:?}");
    }
    // 先以红色建立可观察的累计目标。
    canvas.fill_rect(Rect::new(0.0, 0.0, 16.0, 10.0), Color::red(), None);
    // 后续软件填充切换到目标相关 Additive。
    canvas.set_blend_mode(BlendMode::Additive);
    // 每笔源颜色只贡献一半 opacity，验证该状态已经烘焙进 scratch。
    canvas.set_opacity(0.5);
    // 设置固定 Native shape 无法表达的非统一缩放。
    canvas.set_transform(Transform::scale(2.0, 1.0));
    // 连续绘制三次同一椭圆，使绿色通道在透明 scratch 中饱和。
    canvas.fill_ellipse(Rect::new(1.0, 1.0, 3.0, 4.0), Color::green());
    // 第二笔必须与第一笔结合在同一 Additive source tile。
    canvas.fill_ellipse(Rect::new(1.0, 1.0, 3.0, 4.0), Color::green());
    // 第三笔把两次取整后的 254 明确推入 255 饱和边界。
    canvas.fill_ellipse(Rect::new(1.0, 1.0, 3.0, 4.0), Color::green());
    // 构造一个本地直角三角形。
    let mut builder = PathBuilder::new();
    // 设置三角形左上顶点。
    builder.move_to(8.0, 2.0);
    // 设置三角形右上顶点。
    builder.line_to(11.0, 2.0);
    // 设置三角形左下顶点。
    builder.line_to(8.0, 5.0);
    // 闭合路径。
    builder.close();
    // 构建不可变路径载荷。
    let path = builder.build();
    // 改用带水平剪切与整数平移的任意仿射。
    canvas.set_transform(Transform {
        // x' = x + 0.5y - 2，y' = y。
        m: [1.0, 0.5, -2.0, 0.0, 1.0, 0.0],
    });
    // 在同一 Additive 批次中追加蓝色仿射路径。
    canvas.fill_path(&path, Color::blue(), FillRule::NonZero);
    // 切回普通 blend，迫使 Additive scratch 在正确 painter 边界封口。
    canvas.set_blend_mode(BlendMode::SrcOver);
    // 恢复不透明颜色，避免后一普通填充继承半透明状态。
    canvas.set_opacity(1.0);
    // 恢复 identity transform。
    canvas.set_transform(Transform::identity());
    // 在 Additive segment 之后记录一个普通白色矩形。
    canvas.fill_rect(Rect::new(13.0, 1.0, 2.0, 2.0), Color::white(), None);
    // 完成记录并取得不可变命令流。
    let encoder = match canvas.finish_recording() {
        // 保存成功编码器供命令与像素审计。
        Ok(encoder) => encoder,
        // 可逆仿射填充不应产生 deferred failure。
        Err(error) => panic!("additive sampled fill recording should finish: {error:?}"),
    };
    // 精确匹配 clear、红底、一个 Additive sampled tile 与后置白色填充。
    let [FrameCommand::Clear { .. }, FrameCommand::Native {
        operation: FrameRasterOp::FillRect { .. },
    }, FrameCommand::PictureBlit {
        image,
        src,
        dst,
        opacity,
        additive: true,
    }, FrameCommand::Native {
        operation: FrameRasterOp::FillRect { .. },
    }] = encoder.commands()
    else {
        // CpuSegment、多个 Additive tile 或命令错序都应失败。
        panic!("expected one tight additive sampled fill segment");
    };
    // scratch 已经应用每笔 opacity，最终上传必须保持 opaque。
    assert!(opacity.is_opaque());
    // 源裁剪必须从紧密图片原点开始。
    assert_eq!(*src, FrameRect::new(0, 0, image.width(), image.height()));
    // sampled 目标尺寸必须与紧密源图片一致。
    assert_eq!(dst.width(), image.width() as f32);
    // sampled 目标高度必须与紧密源图片一致。
    assert_eq!(dst.height(), image.height() as f32);
    // 可见 tile 不得退化为完整 surface 上传。
    assert!(image.width() < 16 && image.height() < 10);
    // 执行同一命令流的 CPU 参考路径。
    let reference = encoder.render_reference();
    // 三笔半透明绿色椭圆必须先饱和，再与红色目标得到黄色。
    assert_eq!(
        reference.pixel(5, 3),
        Some(Color::from_rgb(255, 255, 0).premultiplied())
    );
    // 剪切路径内部应把半透明蓝色加到红色目标。
    assert_eq!(
        reference.pixel(8, 3),
        Some(Color::from_rgb(255, 0, 127).premultiplied())
    );
    // painter barrier 后的普通白色填充必须最后覆盖目标。
    assert_eq!(reference.pixel(13, 1), Some(Color::white().premultiplied()));
}

// blend 切换和 restore 都必须把透明 scratch 封成独立有序命令。
#[test]
fn additive_sampled_fill_preserves_blend_and_restore_barriers() {
    // 创建两个互不相交的顺序验证区域。
    let mut canvas = FrameRecordingCanvas::new(14, 7);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("blend barrier recording should begin: {error:?}");
    }
    // 先建立完整红色目标。
    canvas.fill_rect(Rect::new(0.0, 0.0, 14.0, 7.0), Color::red(), None);
    // 普通蓝色椭圆进入第一个 SrcOver CPU segment。
    canvas.fill_ellipse(Rect::new(1.0, 1.0, 4.0, 4.0), Color::blue());
    // 切入 Additive 必须先 flush 普通 scratch。
    canvas.set_blend_mode(BlendMode::Additive);
    // 绿色椭圆进入第一个 Additive sampled segment。
    canvas.fill_ellipse(Rect::new(1.0, 1.0, 4.0, 4.0), Color::green());
    // 保存当前 Additive 状态，供 restore barrier 验证。
    canvas.save();
    // 切回普通 blend 必须先 flush Additive scratch。
    canvas.set_blend_mode(BlendMode::SrcOver);
    // 第二个普通蓝色椭圆进入新的 CPU segment。
    canvas.fill_ellipse(Rect::new(8.0, 1.0, 4.0, 4.0), Color::blue());
    // restore 将 blend 恢复为 Additive，必须先 flush 第二个普通 scratch。
    canvas.restore();
    // 第二个绿色椭圆进入新的 Additive sampled segment。
    canvas.fill_ellipse(Rect::new(8.0, 1.0, 4.0, 4.0), Color::green());
    // 完成记录并取得不可变命令流。
    let encoder = match canvas.finish_recording() {
        // 保存成功编码器供顺序审计。
        Ok(encoder) => encoder,
        // 合法 barrier 不应产生 deferred failure。
        Err(error) => panic!("blend barrier recording should finish: {error:?}"),
    };
    // 命令顺序必须严格保持普通、Additive、普通、Additive 四个分段。
    let [FrameCommand::Clear { .. }, FrameCommand::Native {
        operation: FrameRasterOp::FillRect { .. },
    }, FrameCommand::CpuSegment { .. }, FrameCommand::PictureBlit { additive: true, .. }, FrameCommand::CpuSegment { .. }, FrameCommand::PictureBlit { additive: true, .. }] =
        encoder.commands()
    else {
        // 任意跨 blend 合批或 painter 次序变化都应失败。
        panic!("expected ordered SrcOver and Additive scratch segments");
    };
    // 执行同一命令流的 CPU 参考路径。
    let reference = encoder.render_reference();
    // 左侧区域必须先被蓝色覆盖，再叠加绿色得到青色。
    assert_eq!(
        reference.pixel(3, 3),
        Some(Color::from_rgb(0, 255, 255).premultiplied())
    );
    // restore 后的右侧区域必须保持完全相同的顺序结果。
    assert_eq!(
        reference.pixel(10, 3),
        Some(Color::from_rgb(0, 255, 255).premultiplied())
    );
}

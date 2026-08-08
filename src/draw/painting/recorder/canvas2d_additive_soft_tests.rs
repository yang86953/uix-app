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

// 线性与径向渐变必须在局部空间求值，并与其他源贡献共享紧边界 Additive tile。
#[test]
fn additive_affine_gradients_batch_into_tight_sampled_segment() {
    // 创建能容纳缩放线性渐变、径向渐变和普通仿射填充的画布。
    let mut canvas = FrameRecordingCanvas::new(18, 12);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("additive affine gradient recording should begin: {error:?}");
    }
    // 后续纯源贡献使用目标相关 Additive 混合。
    canvas.set_blend_mode(BlendMode::Additive);
    // offset 必须先移动局部渐变，再由 transform 缩放。
    canvas.set_offset(1.0, 0.0);
    // 水平放大用于验证设备像素中心的逆映射。
    canvas.set_transform(Transform::scale(2.0, 1.0));
    // 局部 clip 只保留线性渐变的第一行像素。
    canvas.push_clip(Rect::new(0.0, 1.0, 2.0, 1.0));
    // 第一次线性渐变提供红到绿的局部颜色变化。
    canvas.fill_linear_gradient(
        // offset 后的局部 x 范围为 1..3，设备范围为 2..6。
        Rect::new(0.0, 1.0, 2.0, 2.0),
        // 左端使用不透明红色。
        Color::red(),
        // 右端使用不透明绿色。
        Color::green(),
        // 只沿局部 x 轴推进。
        GradientDirection::Horizontal,
    );
    // 第二次相同渐变必须在透明 scratch 中逐通道饱和累积。
    canvas.fill_linear_gradient(
        // 使用相同局部几何。
        Rect::new(0.0, 1.0, 2.0, 2.0),
        // 重复相同起点颜色。
        Color::red(),
        // 重复相同终点颜色。
        Color::green(),
        // 保持相同方向。
        GradientDirection::Horizontal,
    );
    // 结束局部裁剪，但保持同一个 Additive source 批次。
    canvas.pop_clip();
    // 清除 offset，验证后续径向渐变仍读取最新状态。
    canvas.set_offset(0.0, 0.0);
    // 在画布右下区域追加经过水平缩放的径向渐变。
    canvas.fill_radial_gradient(
        // 局部圆心经过 transform 后位于设备 x=12。
        6.0,
        // 垂直方向保持 y=7。
        7.0,
        // 从圆心开始插值。
        0.0,
        // 局部外半径为两个像素。
        2.0,
        // 内圈使用红色。
        Color::red(),
        // 外圈使用蓝色。
        Color::blue(),
    );
    // 追加既有仿射填充，验证渐变与 shape 共用 source tile。
    canvas.fill_ellipse(Rect::new(1.0, 9.0, 1.0, 1.0), Color::blue());
    // 完成记录并取得不可变命令流。
    let encoder = match canvas.finish_recording() {
        // 保存编码器供命令和参考像素审计。
        Ok(encoder) => encoder,
        // 可逆仿射渐变不应产生 deferred failure。
        Err(error) => panic!("additive affine gradients should finish: {error:?}"),
    };
    // 所有连续源贡献必须收敛为唯一 Additive sampled tile。
    let [FrameCommand::Clear { .. }, FrameCommand::PictureBlit {
        image,
        src,
        dst,
        opacity,
        additive: true,
    }] = encoder.commands()
    else {
        // CpuSegment、多个 sampled tile 或错误 blend 都表示合批失败。
        panic!("expected one additive sampled gradient segment");
    };
    // scratch 已经应用每笔 opacity，最终合成必须保持 opaque。
    assert!(opacity.is_opaque());
    // 紧图片源必须从自身原点开始。
    assert_eq!(*src, FrameRect::new(0, 0, image.width(), image.height()));
    // sampled 目标宽度与紧图片一致。
    assert_eq!(dst.width(), image.width() as f32);
    // sampled 目标高度与紧图片一致。
    assert_eq!(dst.height(), image.height() as f32);
    // 多个局部图元的联合写区仍不得退化为完整 surface 上传。
    assert!(image.width() < 18 && image.height() < 12);
    // 执行同一命令流的 CPU 参考路径。
    let reference = encoder.render_reference();
    // 左侧采样点逆映射到 t=0.125；两笔加法令红色饱和、绿色累积为 62。
    assert_eq!(reference.pixel(2, 1), Some(0xffff_3e00));
    // 右侧采样点逆映射到 t=0.875；绿色饱和、红色累积为 62。
    assert_eq!(reference.pixel(5, 1), Some(0xff3e_ff00));
    // 局部 clip 的第二行不得被渐变写入。
    assert_eq!(
        reference.pixel(2, 2),
        Some(Color::transparent().premultiplied())
    );
    // 径向中心附近必须保持红色通道高于蓝色通道。
    let radial_inner = reference.pixel(12, 7).unwrap_or_default();
    // 提取中心附近的预乘红色通道。
    let inner_red = (radial_inner >> 16) & 0xff;
    // 提取中心附近的预乘蓝色通道。
    let inner_blue = radial_inner & 0xff;
    // 逆映射后的局部距离接近圆心，因此红色必须占优。
    assert!(inner_red > inner_blue);
    // 径向外圈附近必须逐渐转为蓝色。
    let radial_outer = reference.pixel(15, 7).unwrap_or_default();
    // 提取外圈附近的预乘红色通道。
    let outer_red = (radial_outer >> 16) & 0xff;
    // 提取外圈附近的预乘蓝色通道。
    let outer_blue = radial_outer & 0xff;
    // 局部距离接近外半径，因此蓝色必须占优。
    assert!(outer_blue > outer_red);
    // offset 后 transform 的设备左界之外必须保持透明。
    assert_eq!(
        reference.pixel(1, 1),
        Some(Color::transparent().premultiplied())
    );
}

// blend 切换、restore 与非有限 opacity 必须维持渐变的分段和 typed failure 边界。
#[test]
fn additive_gradients_preserve_barriers_and_reject_non_finite_opacity() {
    // 创建三个互不相交的渐变区域。
    let mut canvas = FrameRecordingCanvas::new(15, 5);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("gradient barrier recording should begin: {error:?}");
    }
    // 第一段使用 Additive sampled scratch。
    canvas.set_blend_mode(BlendMode::Additive);
    // 左侧线性渐变进入第一段。
    canvas.fill_linear_gradient(
        // 左侧区域。
        Rect::new(0.0, 0.0, 4.0, 4.0),
        // 使用红色起点。
        Color::red(),
        // 使用绿色终点。
        Color::green(),
        // 水平推进。
        GradientDirection::Horizontal,
    );
    // 保存 Additive 状态供 restore barrier 验证。
    canvas.save();
    // 切换为 SrcOver 必须先封口第一段。
    canvas.set_blend_mode(BlendMode::SrcOver);
    // 中间径向渐变进入普通 CPU segment。
    canvas.fill_radial_gradient(7.0, 2.0, 0.0, 2.0, Color::red(), Color::blue());
    // restore 前必须封口普通 segment，并恢复 Additive。
    canvas.restore();
    // 右侧线性渐变进入新的 Additive sampled segment。
    canvas.fill_linear_gradient(
        // 右侧区域。
        Rect::new(11.0, 0.0, 4.0, 4.0),
        // 使用蓝色起点。
        Color::blue(),
        // 使用绿色终点。
        Color::green(),
        // 垂直推进。
        GradientDirection::Vertical,
    );
    // 完成记录并取得命令流。
    let encoder = match canvas.finish_recording() {
        // 保存成功编码器。
        Ok(encoder) => encoder,
        // 合法 barrier 不应失败。
        Err(error) => panic!("gradient barriers should finish: {error:?}"),
    };
    // 命令必须严格保持 Additive、SrcOver、Additive 的 painter order。
    assert!(matches!(
        encoder.commands(),
        [
            FrameCommand::Clear { .. },
            FrameCommand::PictureBlit { additive: true, .. },
            FrameCommand::CpuSegment { .. },
            FrameCommand::PictureBlit { additive: true, .. }
        ]
    ));

    // 创建独立画布验证非有限 opacity 的拒绝边界。
    let mut non_finite = FrameRecordingCanvas::new(8, 8);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = non_finite.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("non-finite gradient recording should begin: {error:?}");
    }
    // 注入不能稳定烘焙进 sampled tile 的 NaN opacity。
    non_finite.set_opacity(f32::NAN);
    // 选择目标相关 Additive 混合。
    non_finite.set_blend_mode(BlendMode::Additive);
    // 尝试记录 otherwise 合法的径向渐变。
    non_finite.fill_radial_gradient(4.0, 4.0, 0.0, 3.0, Color::red(), Color::blue());
    // 完成边界必须返回稳定 typed failure。
    let error = match non_finite.finish_recording() {
        // 保存错误供类型审计。
        Err(error) => error,
        // 成功表示 NaN 被静默转换成了错误像素。
        Ok(_) => panic!("non-finite additive gradient opacity must be rejected"),
    };
    // 失败必须位于不可等价 lowering 的明确边界。
    assert_eq!(error.code(), crate::core::Errc::NotImplemented);
    // deferred failure 前只能保留初始 clear。
    assert_eq!(
        non_finite
            .encoder
            .as_ref()
            .map(|encoder| encoder.commands().len()),
        Some(1)
    );
}

// 完整 surface clip 下的 Additive raw image 应直接复用 sampled textured pipeline。
#[test]
fn additive_raw_image_records_direct_sampled_picture_blit() {
    // 构造 2x2 的四象限预乘源图片。
    let source = [
        // 左上红色。
        Color::red().premultiplied(),
        // 右上绿色。
        Color::green().premultiplied(),
        // 左下蓝色。
        Color::blue().premultiplied(),
        // 右下白色。
        Color::white().premultiplied(),
    ];
    // 创建能容纳四倍面积目标的画布。
    let mut canvas = FrameRecordingCanvas::new(8, 6);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("direct additive image recording should begin: {error:?}");
    }
    // 先建立完整蓝色累计目标。
    canvas.fill_rect(Rect::new(0.0, 0.0, 8.0, 6.0), Color::blue(), None);
    // 后续图片使用目标相关 Additive 混合。
    canvas.set_blend_mode(BlendMode::Additive);
    // PictureBlit 在最终目标上应用一半 opacity。
    canvas.set_opacity(0.5);
    // 将完整 2x2 源图片 nearest 放大到 4x4 目标。
    canvas.blit_image(
        // 传入完整源像素。
        &source,
        // 源行跨度为两个像素。
        2,
        // 采样完整源 crop。
        Rect::new(0.0, 0.0, 2.0, 2.0),
        // 目标保持轴对齐但执行整数缩放。
        Rect::new(2.0, 1.0, 4.0, 4.0),
    );
    // 完成记录并取得不可变命令流。
    let encoder = match canvas.finish_recording() {
        // 保存编码器供载荷和参考像素审计。
        Ok(encoder) => encoder,
        // 合法 Additive textured quad 不应失败。
        Err(error) => panic!("direct additive image should finish: {error:?}"),
    };
    // 命令必须为 clear、蓝底与一个直接 Additive PictureBlit。
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
        // CpuSegment 或 sampled soft tile 都说明直达路径没有生效。
        panic!("expected direct additive raw image PictureBlit");
    };
    // retained 图片只保存调用方选定的紧 crop。
    assert_eq!(image.pixels(), &source);
    // retained 源从自身原点覆盖完整 2x2 图片。
    assert_eq!(*src, FrameRect::new(0, 0, 2, 2));
    // 整数目标应能无损还原为 4x4 FrameRect。
    assert_eq!(dst.as_integer(), Some(FrameRect::new(2, 1, 4, 4)));
    // 图片 opacity 必须由直接命令保留，而不是提前重复烘焙。
    assert_eq!(opacity.value(), 0.5);
    // 执行同一命令流的 CPU 参考路径。
    let reference = encoder.render_reference();
    // 半透明红源加到蓝底后得到预乘紫色。
    assert_eq!(reference.pixel(2, 1), Some(0xff7f_00ff));
    // 半透明绿源加到蓝底后得到预乘青蓝色。
    assert_eq!(reference.pixel(5, 1), Some(0xff00_7fff));
}

// 部分 clip 或非 identity transform 的 raw image 必须进入仿射 source scratch。
#[test]
fn additive_affine_raw_images_batch_into_tight_sampled_segment() {
    // 构造一行红绿预乘源图片。
    let source = [
        // 第一列为红色。
        Color::red().premultiplied(),
        // 第二列为绿色。
        Color::green().premultiplied(),
    ];
    // 创建足以证明联合 tile 不等于完整 surface 的画布。
    let mut canvas = FrameRecordingCanvas::new(20, 10);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("affine additive image recording should begin: {error:?}");
    }
    // 所有源贡献切换到 Additive。
    canvas.set_blend_mode(BlendMode::Additive);
    // 每笔图片只贡献一半 opacity。
    canvas.set_opacity(0.5);
    // 分数 offset 必须先移动局部目标。
    canvas.set_offset(0.5, 0.0);
    // 非 identity 水平缩放迫使 raw image 使用软件逆映射。
    canvas.set_transform(Transform::scale(2.0, 1.0));
    // 局部 clip 只允许目标左半边写入。
    canvas.push_clip(Rect::new(1.0, 1.0, 1.0, 2.0));
    // 连续三次绘制同一图片，验证透明 source scratch 内的饱和累积。
    for _ in 0..3 {
        // 每笔都采样完整红绿源。
        canvas.blit_image(
            // 复用源像素。
            &source,
            // 源行跨度。
            2,
            // 完整源 crop。
            Rect::new(0.0, 0.0, 2.0, 1.0),
            // offset 后局部目标为 x=1.5..3.5，再映射为设备 x=3..7。
            Rect::new(1.0, 1.0, 2.0, 2.0),
        );
    }
    // 结束图片局部裁剪但保持同一 Additive 批次。
    canvas.pop_clip();
    // 后续 shape 使用最新状态并与图片共享 source tile。
    canvas.set_offset(0.0, 0.0);
    // 追加一个与图片分离的蓝色仿射椭圆。
    canvas.fill_ellipse(Rect::new(6.0, 5.0, 1.0, 1.0), Color::blue());
    // 完成记录并取得命令流。
    let encoder = match canvas.finish_recording() {
        // 保存成功编码器。
        Ok(encoder) => encoder,
        // 可逆仿射图片不应产生 deferred failure。
        Err(error) => panic!("affine additive images should finish: {error:?}"),
    };
    // 图片与椭圆必须共同收敛为唯一 Additive sampled source tile。
    let [FrameCommand::Clear { .. }, FrameCommand::PictureBlit {
        image,
        src,
        dst,
        opacity,
        additive: true,
    }] = encoder.commands()
    else {
        // direct PictureBlit、CpuSegment 或多个 tile 都表示 fallback 分段错误。
        panic!("expected one affine additive image source tile");
    };
    // source scratch 已经烘焙每笔 opacity，最终合成保持 opaque。
    assert!(opacity.is_opaque());
    // 紧图片源从自身原点开始。
    assert_eq!(*src, FrameRect::new(0, 0, image.width(), image.height()));
    // sampled 目标宽度必须与紧图片一致。
    assert_eq!(dst.width(), image.width() as f32);
    // sampled 目标高度必须与紧图片一致。
    assert_eq!(dst.height(), image.height() as f32);
    // 联合写区不能退化为完整 surface 上传。
    assert!(image.width() < 20 && image.height() < 10);
    // 执行同一命令流的参考路径。
    let reference = encoder.render_reference();
    // clip 内红色 texel 连续三次半透明加法必须饱和为不透明红色。
    assert_eq!(reference.pixel(3, 1), Some(Color::red().premultiplied()));
    // clip 右界之外不得写入本应采样到的绿色 texel。
    assert_eq!(
        reference.pixel(5, 1),
        Some(Color::transparent().premultiplied())
    );
    // 相邻仿射椭圆必须仍出现在同一最终 tile 中。
    assert_ne!(
        reference.pixel(12, 5),
        Some(Color::transparent().premultiplied())
    );
}

// 图片 fallback 必须保持 blend/restore barrier，并拒绝非有限 Additive opacity。
#[test]
fn additive_raw_images_preserve_barriers_and_reject_non_finite_opacity() {
    // 使用单个红色预乘 texel简化顺序像素载荷。
    let source = [Color::red().premultiplied()];
    // 创建三个互不相交的图片区域。
    let mut canvas = FrameRecordingCanvas::new(15, 5);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("image barrier recording should begin: {error:?}");
    }
    // 非 identity transform 使三段都走 software image。
    canvas.set_transform(Transform::scale(2.0, 1.0));
    // 第一段使用 Additive。
    canvas.set_blend_mode(BlendMode::Additive);
    // 保存当前 Additive 状态供 restore 验证。
    canvas.save();
    // 左侧图片进入第一段 Additive source tile。
    canvas.blit_image(
        // 单 texel 源。
        &source,
        // 单像素行跨度。
        1,
        // 完整源 crop。
        Rect::new(0.0, 0.0, 1.0, 1.0),
        // 左侧局部目标。
        Rect::new(0.0, 1.0, 1.0, 2.0),
    );
    // 切回 SrcOver 必须先封口第一段。
    canvas.set_blend_mode(BlendMode::SrcOver);
    // 中间图片进入普通 CPU segment。
    canvas.blit_image(
        // 复用单 texel 源。
        &source,
        // 单像素行跨度。
        1,
        // 完整源 crop。
        Rect::new(0.0, 0.0, 1.0, 1.0),
        // 中间局部目标映射到设备 x=6..8。
        Rect::new(3.0, 1.0, 1.0, 2.0),
    );
    // restore 前必须封口普通 segment，并恢复 Additive。
    canvas.restore();
    // 右侧图片进入新的 Additive source tile。
    canvas.blit_image(
        // 复用单 texel 源。
        &source,
        // 单像素行跨度。
        1,
        // 完整源 crop。
        Rect::new(0.0, 0.0, 1.0, 1.0),
        // 右侧局部目标映射到设备 x=12..14。
        Rect::new(6.0, 1.0, 1.0, 2.0),
    );
    // 完成记录并取得命令流。
    let encoder = match canvas.finish_recording() {
        // 保存成功编码器。
        Ok(encoder) => encoder,
        // 合法 barrier 不应失败。
        Err(error) => panic!("image barriers should finish: {error:?}"),
    };
    // painter order 必须严格保持 Additive、SrcOver、Additive 三段。
    assert!(matches!(
        encoder.commands(),
        [
            FrameCommand::Clear { .. },
            FrameCommand::PictureBlit { additive: true, .. },
            FrameCommand::CpuSegment { .. },
            FrameCommand::PictureBlit { additive: true, .. }
        ]
    ));

    // 创建独立画布验证非有限 opacity 拒绝。
    let mut non_finite = FrameRecordingCanvas::new(6, 6);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = non_finite.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("non-finite image recording should begin: {error:?}");
    }
    // 注入不能稳定烘焙的 NaN opacity。
    non_finite.set_opacity(f32::NAN);
    // 选择目标相关 Additive 混合。
    non_finite.set_blend_mode(BlendMode::Additive);
    // 尝试记录 otherwise 合法的 raw image。
    non_finite.blit_image(
        // 单 texel 源。
        &source,
        // 单像素行跨度。
        1,
        // 完整源 crop。
        Rect::new(0.0, 0.0, 1.0, 1.0),
        // 合法目标。
        Rect::new(1.0, 1.0, 2.0, 2.0),
    );
    // 完成边界必须返回稳定 typed failure。
    let error = match non_finite.finish_recording() {
        // 保存错误供类型审计。
        Err(error) => error,
        // 成功表示 NaN 被静默转换成错误像素。
        Ok(_) => panic!("non-finite additive image opacity must be rejected"),
    };
    // 错误必须位于不可等价 lowering 边界。
    assert_eq!(error.code(), crate::core::Errc::NotImplemented);
    // 失败前只能保留初始 clear。
    assert_eq!(
        non_finite
            .encoder
            .as_ref()
            .map(|encoder| encoder.commands().len()),
        Some(1)
    );
}

// 继续在同一测试模块内加载 Additive glyph sampled soft 分段回归测试。
include!("canvas2d_additive_glyph_soft_tests.rs");

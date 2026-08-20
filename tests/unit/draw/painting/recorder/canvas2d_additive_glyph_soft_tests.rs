// Additive glyph 必须按仿射后的 coverage 与其他源贡献共享一个紧边界 sampled 批次。
#[test]
fn additive_affine_glyphs_batch_with_other_sources() {
    // 构造包含完整、半覆盖、透明和完整四个 texel 的字形。
    let coverage = [
        // 左上完全覆盖。
        255, // 右上半覆盖。
        128, // 左下完全透明。
        0,   // 右下完全覆盖。
        255,
    ];
    // 创建足以证明 tile 没有退化为完整 surface 的画布。
    let mut canvas = FrameRecordingCanvas::new(20, 10);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("affine additive glyph recording should begin: {error:?}");
    }
    // 先建立完整红色累计目标。
    canvas.fill_rect(Rect::new(0.0, 0.0, 20.0, 10.0), Color::red(), None);
    // 后续字形和椭圆使用目标相关 Additive。
    canvas.set_blend_mode(BlendMode::Additive);
    // 每笔字形只贡献一半 opacity。
    canvas.set_opacity(0.5);
    // 分数 offset 必须在缩放 transform 前应用。
    canvas.set_offset(0.5, 0.0);
    // 水平二倍缩放迫使 glyph 进入软件逆映射路径。
    canvas.set_transform(Transform::scale(2.0, 1.0));
    // 第一笔绿色 glyph 建立 coverage 源贡献。
    canvas.blit_glyph(1, 1, &coverage, 2, 2, Color::green());
    // 第二笔必须与第一笔在透明 scratch 中饱和累积。
    canvas.blit_glyph(1, 1, &coverage, 2, 2, Color::green());
    // 第三笔把完整 coverage 的绿色通道明确推入饱和边界。
    canvas.blit_glyph(1, 1, &coverage, 2, 2, Color::green());
    // 同一 Additive 状态下追加蓝色椭圆以证明跨图元合批。
    canvas.fill_ellipse(Rect::new(5.0, 1.0, 2.0, 3.0), Color::blue());
    // 完成记录并取得不可变命令流。
    let encoder = match canvas.finish_recording() {
        // 保存编码器供载荷和参考像素审计。
        Ok(encoder) => encoder,
        // 可逆仿射 glyph 不应产生 deferred failure。
        Err(error) => panic!("affine additive glyph should finish: {error:?}"),
    };
    // 命令必须为 clear、红底和一个跨图元 Additive sampled tile。
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
        // CpuSegment 或多个 tile 都说明 glyph 没有进入共享 source scratch。
        panic!("expected one tight additive glyph source segment");
    };
    // scratch 已经应用每笔 opacity，最终上传必须保持 opaque。
    assert!(opacity.is_opaque());
    // 紧图片的源 crop 必须从自身原点覆盖全图。
    assert_eq!(*src, FrameRect::new(0, 0, image.width(), image.height()));
    // sampled 目标尺寸必须与紧图片完全一致。
    assert_eq!(dst.width(), image.width() as f32);
    // 联合 tile 不得退化为完整 surface 上传。
    assert!(image.width() < 20 && image.height() < 10);
    // 执行同一命令流的 CPU 参考路径。
    let reference = encoder.render_reference();
    // 三笔完整 coverage 半透明绿色应先饱和，再与红底得到黄色。
    assert_eq!(
        // 水平缩放后的第一列覆盖设备 x=3。
        reference.pixel(3, 1),
        // 预期逐通道饱和黄色。
        Some(Color::from_rgb(255, 255, 0).premultiplied())
    );
    // 半 coverage 每笔产生 63，三笔应累计为 189。
    assert_eq!(
        // 水平缩放后的第二列覆盖设备 x=5。
        reference.pixel(5, 1),
        // 红底加 189 绿色并保持不透明 alpha。
        Some(0xffff_bd00)
    );
    // coverage 为零的左下 texel 不得改变红色目标。
    assert_eq!(reference.pixel(3, 2), Some(Color::red().premultiplied()));
    // 后续蓝色椭圆必须与 glyph 同处一个 tile 且产生可见贡献。
    assert_ne!(reference.pixel(12, 2), Some(Color::red().premultiplied()));
}

// 仿射 glyph 必须同时保留路径 clip 与 coverage 调制。
#[test]
fn additive_affine_glyph_respects_path_clip_and_coverage() {
    // 引入路径构造器以生成局部字形左半区域的 clip。
    use crate::draw::geometry::path::PathBuilder;

    // 构造两行不同 coverage 的 2x2 字形。
    let coverage = [
        // 左上完整覆盖。
        255, // 右上低覆盖。
        64,  // 左下半覆盖。
        128, // 右下透明。
        0,
    ];
    // 创建容纳剪切后字形的透明画布。
    let mut canvas = FrameRecordingCanvas::new(12, 8);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("path-clipped additive glyph recording should begin: {error:?}");
    }
    // 切换到目标相关 Additive。
    canvas.set_blend_mode(BlendMode::Additive);
    // 全局 opacity 必须先与颜色 alpha 量化。
    canvas.set_opacity(0.5);
    // 设置水平剪切和平移的可逆仿射。
    canvas.set_transform(Transform {
        // x' = x + 0.5y + 2，y' = y + 1。
        m: [1.0, 0.5, 2.0, 0.0, 1.0, 1.0],
    });
    // 构造只覆盖局部 glyph 左列的矩形路径。
    let mut builder = PathBuilder::new();
    // 路径左上角。
    builder.move_to(1.0, 1.0);
    // 路径右上角。
    builder.line_to(2.0, 1.0);
    // 路径右下角。
    builder.line_to(2.0, 3.0);
    // 路径左下角。
    builder.line_to(1.0, 3.0);
    // 闭合局部 clip。
    builder.close();
    // 构建不可变路径。
    let clip = builder.build();
    // 将路径按当前仿射 lowering 为 surface coverage mask。
    canvas.push_clip_path(&clip);
    // 绘制覆盖整个 2x2 局部矩形的绿色字形。
    canvas.blit_glyph(1, 1, &coverage, 2, 2, Color::green());
    // 改回 identity，以验证路径 mask 本身会阻止 Additive Native shape 直达。
    canvas.set_transform(Transform::identity());
    // 该矩形必须经过现有 surface-space 路径 mask 并留在同一 source scratch。
    canvas.fill_rect(Rect::new(4.0, 2.0, 1.0, 1.0), Color::red(), None);
    // 恢复完整裁剪，验证路径状态不会泄露。
    canvas.pop_clip();
    // 完成记录并取得参考像素。
    let encoder = match canvas.finish_recording() {
        // 保存成功结果。
        Ok(encoder) => encoder,
        // 可逆仿射和合法路径不得失败。
        Err(error) => panic!("path-clipped additive glyph should finish: {error:?}"),
    };
    // 路径裁剪后的 glyph 仍应封装为单一 Additive sampled tile。
    assert!(matches!(
        // 读取稳定命令切片。
        encoder.commands(),
        // 禁止退化为 destination-dependent CpuSegment。
        [
            FrameCommand::Clear { .. },
            FrameCommand::PictureBlit { additive: true, .. }
        ]
    ));
    // 执行 CPU 参考路径以检查真实 clip coverage。
    let reference = encoder.render_reference();
    // 左上完整 coverage 的安全内部像素必须可见。
    assert_ne!(reference.pixel(4, 2), Some(0));
    // 左下半 coverage 必须仍保留非零但较小的绿色贡献。
    let lower = reference.pixel(4, 3).unwrap_or(0);
    // 提取下方像素绿色通道。
    let lower_green = (lower >> 8) & 0xff;
    // 半 coverage 的内部像素应介于透明和上方完整 coverage 之间。
    assert!(lower_green > 0 && lower_green < 127);
    // 路径右侧即使位于 glyph AABB 内也必须保持透明。
    assert_eq!(reference.pixel(5, 2), Some(0));
}

// 全 surface AABB 的路径 mask 仍必须阻止无 mask 的 Native shape 与 direct image。
#[test]
fn additive_full_bounds_path_clip_forces_all_sources_into_scratch() {
    // 引入路径构造器以生成 AABB 等于完整 surface 的三角形。
    use crate::draw::geometry::path::PathBuilder;

    // 创建与三角形边界完全一致的目标画布。
    let mut canvas = FrameRecordingCanvas::new(12, 8);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("full-bounds path clip recording should begin: {error:?}");
    }
    // 选择所有后续源贡献都能进入透明 scratch 的 Additive。
    canvas.set_blend_mode(BlendMode::Additive);
    // 构造左上半平面三角形，其 AABB 覆盖完整 surface。
    let mut builder = PathBuilder::new();
    // 左上顶点。
    builder.move_to(0.0, 0.0);
    // 右上顶点使 AABB 宽度达到十二像素。
    builder.line_to(12.0, 0.0);
    // 左下顶点使 AABB 高度达到八像素。
    builder.line_to(0.0, 8.0);
    // 闭合路径。
    builder.close();
    // 建立 surface-space coverage mask。
    canvas.push_clip_path(&builder.build());
    // 内部 glyph 保证最终 sampled tile 含有可见源贡献。
    canvas.blit_glyph(1, 1, &[255], 1, 1, Color::green());
    // 右下矩形位于三角形外；没有 mask 门禁时它会错误直达 Native shape。
    canvas.fill_rect(Rect::new(10.0, 6.0, 2.0, 2.0), Color::red(), None);
    // 构造一个完整不透明蓝色 raw image。
    let source = [Color::blue().premultiplied()];
    // 右下图片同样位于路径外，且其当前矩形 clip 仍等于完整 surface AABB。
    canvas.blit_image(
        // 传入单像素源。
        &source,
        // 源行跨度为一。
        1,
        // 采样完整源 crop。
        Rect::new(0.0, 0.0, 1.0, 1.0),
        // 目标位于三角形外侧。
        Rect::new(11.0, 7.0, 1.0, 1.0),
    );
    // 恢复完整无 mask 状态。
    canvas.pop_clip();
    // 完成记录并取得命令事实。
    let encoder = match canvas.finish_recording() {
        // 保存成功结果。
        Ok(encoder) => encoder,
        // 合法 full-bounds mask 不应失败。
        Err(error) => panic!("full-bounds path clip should finish: {error:?}"),
    };
    // 所有三类源必须共用一个 Additive sampled tile。
    assert!(matches!(
        // 读取稳定命令切片。
        encoder.commands(),
        // Native shape 或第二个 direct PictureBlit 都属于 mask 泄漏。
        [
            FrameCommand::Clear { .. },
            FrameCommand::PictureBlit { additive: true, .. }
        ]
    ));
    // 执行同一命令流的 CPU 参考路径。
    let reference = encoder.render_reference();
    // 三角形安全内部的 glyph 必须保持可见。
    assert_eq!(reference.pixel(1, 1), Some(Color::green().premultiplied()));
    // 三角形外的矩形与 raw image 都必须被路径 mask 完全裁掉。
    assert_eq!(reference.pixel(11, 7), Some(0));
}

// blend 与 restore 必须分隔 glyph 批次，畸形 coverage 和非有限 opacity 保持旧边界。
#[test]
fn additive_glyphs_preserve_barriers_and_reject_invalid_state() {
    // 完整 coverage 单像素足以识别三个顺序分段。
    let coverage = [255];
    // 创建容纳三个互不重叠 glyph 的画布。
    let mut canvas = FrameRecordingCanvas::new(16, 6);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("glyph barrier recording should begin: {error:?}");
    }
    // 非 identity transform 让中间 SrcOver glyph 明确进入 CPU segment。
    canvas.set_transform(Transform::scale(2.0, 2.0));
    // 建立随后可由 restore 找回的 Additive 状态。
    canvas.set_blend_mode(BlendMode::Additive);
    // 保存 Additive blend 与 transform。
    canvas.save();
    // 第一笔绿色 glyph 进入 Additive sampled segment。
    canvas.blit_glyph(1, 1, &coverage, 1, 1, Color::green());
    // 切到 SrcOver 必须先封口第一段。
    canvas.set_blend_mode(BlendMode::SrcOver);
    // 第二笔蓝色 glyph 进入普通 CpuSegment。
    canvas.blit_glyph(3, 1, &coverage, 1, 1, Color::blue());
    // restore 必须先封口普通段并恢复 Additive。
    canvas.restore();
    // 第三笔绿色 glyph 进入新的 Additive sampled segment。
    canvas.blit_glyph(5, 1, &coverage, 1, 1, Color::green());
    // 短 coverage 无法覆盖声明的 2x2 字形，必须安全 no-op。
    canvas.blit_glyph(7, 1, &coverage, 2, 2, Color::red());
    // 完成记录并取得命令顺序。
    let encoder = match canvas.finish_recording() {
        // 保存成功结果。
        Ok(encoder) => encoder,
        // 合法 glyph 与畸形 no-op 不应失败。
        Err(error) => panic!("glyph barriers should finish: {error:?}"),
    };
    // 精确匹配 Additive、SrcOver、Additive 三个 painter 分段。
    assert!(matches!(
        // 读取稳定命令切片。
        encoder.commands(),
        // 中间必须保持 CpuSegment，两个 Additive tile 不得跨 barrier 合并。
        [
            FrameCommand::Clear { .. },
            FrameCommand::PictureBlit { additive: true, .. },
            FrameCommand::CpuSegment { .. },
            FrameCommand::PictureBlit { additive: true, .. }
        ]
    ));
    // 单独创建画布验证非有限 opacity 的 typed failure。
    let mut non_finite = FrameRecordingCanvas::new(6, 6);
    // 开始错误边界记录。
    if let Err(error) = non_finite.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("non-finite glyph recording should begin: {error:?}");
    }
    // NaN 必须保持为不可烘焙状态。
    non_finite.set_opacity(f32::NAN);
    // 选择会进入 sampled scratch 的 Additive。
    non_finite.set_blend_mode(BlendMode::Additive);
    // 尝试记录一个 otherwise 合法的 glyph。
    non_finite.blit_glyph(1, 1, &coverage, 1, 1, Color::green());
    // 帧边界必须返回延迟的 typed failure。
    let error = match non_finite.finish_recording() {
        // 成功会静默吞掉 NaN，属于语义回归。
        Ok(_) => panic!("non-finite additive glyph opacity should fail"),
        // 保存真实错误供错误码审计。
        Err(error) => error,
    };
    // 保持稳定的未实现状态错误码。
    assert_eq!(error.code(), crate::core::Errc::NotImplemented);
    // 失败前不得提交半成品 glyph 命令。
    assert_eq!(
        // 测试模块可直接读取仍保留的内部编码器。
        non_finite
            // begin_recording 已建立编码器。
            .encoder
            // 借用失败后保留的命令载荷。
            .as_ref()
            // 只统计已记录命令数，避免消费画布。
            .map(|encoder| encoder.commands().len()),
        // 初始 clear 是唯一允许存在的命令。
        Some(1)
    );
}

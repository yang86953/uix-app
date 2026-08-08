// 定向与环境阴影必须按局部仿射语义和其他源贡献共享紧边界 sampled 批次。
#[test]
fn additive_affine_shadows_batch_with_other_sources() {
    // 创建足以证明最终 tile 没有退化为完整 surface 的画布。
    let mut canvas = FrameRecordingCanvas::new(30, 14);
    // 开始一帧带透明 clear 的正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("affine additive shadow recording should begin: {error:?}");
    }
    // 先建立完整红色累计目标。
    canvas.fill_rect(Rect::new(0.0, 0.0, 30.0, 14.0), Color::red(), None);
    // 后续阴影和形状使用目标相关 Additive。
    canvas.set_blend_mode(BlendMode::Additive);
    // 每笔源贡献只使用一半 opacity。
    canvas.set_opacity(0.5);
    // 分数画布 offset 必须先于 transform 应用。
    canvas.set_offset(0.5, 0.0);
    // 水平二倍缩放迫使阴影进入设备像素逆映射路径。
    canvas.set_transform(Transform::scale(2.0, 1.0));
    // 第一笔零 blur 定向阴影建立完整绿色 coverage。
    canvas.draw_box_shadow(
        // 阴影来源局部矩形。
        Rect::new(1.0, 2.0, 3.0, 3.0),
        // 零 blur 使用普通 SDF 抗锯齿。
        0.0,
        // 效果水平偏移也必须在 transform 前应用。
        1.0,
        // 本用例不添加垂直效果偏移。
        0.0,
        // 绿色源便于观察饱和加法。
        Color::green(),
        // 直角矩形简化内部像素预期。
        None,
    );
    // 第二笔必须在同一透明 source scratch 中累计。
    canvas.draw_box_shadow(
        // 复用相同局部矩形。
        Rect::new(1.0, 2.0, 3.0, 3.0),
        // 保持零 blur。
        0.0,
        // 保持局部水平效果偏移。
        1.0,
        // 保持零垂直偏移。
        0.0,
        // 继续累计绿色。
        Color::green(),
        // 继续使用直角。
        None,
    );
    // 第三笔把半透明绿色明确推入逐通道饱和边界。
    canvas.draw_box_shadow(
        // 复用相同局部矩形。
        Rect::new(1.0, 2.0, 3.0, 3.0),
        // 保持零 blur。
        0.0,
        // 保持局部水平效果偏移。
        1.0,
        // 保持零垂直偏移。
        0.0,
        // 第三次累计绿色。
        Color::green(),
        // 继续使用直角。
        None,
    );
    // 在同一 Additive 批次追加蓝色环境阴影。
    canvas.draw_box_shadow_ambient(
        // 与定向阴影分离的局部矩形。
        Rect::new(7.0, 2.0, 2.0, 2.0),
        // 正 blur 选择环境 coverage 曲线。
        2.0,
        // 环境阴影不额外水平偏移。
        0.0,
        // 环境阴影不额外垂直偏移。
        0.0,
        // 蓝色用于区分 coverage 曲线。
        Color::blue(),
        // 非统一圆角验证局部 SDF 参数被保留。
        Some(Radius {
            // 左上使用较小半径。
            tl: 0.5,
            // 右上使用较大半径。
            tr: 1.0,
            // 右下恢复较小半径。
            br: 0.5,
            // 左下使用较大半径。
            bl: 1.0,
        }),
    );
    // 追加一个蓝色椭圆证明阴影可与其他源图元合批。
    canvas.fill_ellipse(Rect::new(11.0, 7.0, 1.0, 2.0), Color::blue());
    // 完成记录并取得不可变命令流。
    let encoder = match canvas.finish_recording() {
        // 保存成功结果供命令和像素审计。
        Ok(encoder) => encoder,
        // 合法仿射阴影不应产生 deferred failure。
        Err(error) => panic!("affine additive shadows should finish: {error:?}"),
    };
    // 命令必须为 clear、红底和一个跨来源 Additive sampled tile。
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
        // CpuSegment 或多个 tile 都说明阴影没有进入共享 source scratch。
        panic!("expected one tight additive shadow source segment");
    };
    // scratch 已经应用每笔 opacity，最终上传必须保持 opaque。
    assert!(opacity.is_opaque());
    // 紧图片源必须从自身原点覆盖全图。
    assert_eq!(*src, FrameRect::new(0, 0, image.width(), image.height()));
    // sampled 目标宽度必须与紧图片完全一致。
    assert_eq!(dst.width(), image.width() as f32);
    // sampled 目标高度必须与紧图片完全一致。
    assert_eq!(dst.height(), image.height() as f32);
    // 联合写区不能退化为完整 surface 上传。
    assert!(image.width() < 30 && image.height() < 14);
    // 执行同一命令流的 CPU 参考路径。
    let reference = encoder.render_reference();
    // 三笔半透明完整 coverage 绿色必须先饱和，再与红底得到黄色。
    assert_eq!(
        // 设备像素中心逆映射后位于定向阴影安全内部。
        reference.pixel(6, 3),
        // 逐通道饱和后的最终颜色。
        Some(Color::from_rgb(255, 255, 0).premultiplied())
    );
    // 环境阴影安全内部必须给红底增加蓝色贡献。
    assert_ne!(reference.pixel(16, 3), Some(Color::red().premultiplied()));
    // 后续椭圆也必须在最终参考图中可见。
    assert_ne!(reference.pixel(23, 8), Some(Color::red().premultiplied()));
}

// 两种 Additive 阴影必须共同保留 full-bounds 路径 coverage mask。
#[test]
fn additive_shadows_respect_path_clip_and_opacity_once() {
    // 引入路径构造器生成 AABB 等于完整 surface 的三角形。
    use crate::draw::geometry::path::PathBuilder;

    // 创建与路径边界完全一致的透明画布。
    let mut canvas = FrameRecordingCanvas::new(12, 8);
    // 开始正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("path-clipped additive shadow recording should begin: {error:?}");
    }
    // 选择目标相关 Additive source scratch。
    canvas.set_blend_mode(BlendMode::Additive);
    // 阴影颜色只贡献一半 opacity。
    canvas.set_opacity(0.5);
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
    // 在三角形安全内部绘制零 blur 绿色定向阴影。
    canvas.draw_box_shadow(
        // 内部局部矩形。
        Rect::new(1.0, 1.0, 2.0, 2.0),
        // 零 blur 给出完整内部 coverage。
        0.0,
        // 无额外水平偏移。
        0.0,
        // 无额外垂直偏移。
        0.0,
        // 绿色用于精确检查 opacity。
        Color::green(),
        // 使用直角。
        None,
    );
    // 在三角形内部追加带 blur 的蓝色环境阴影。
    canvas.draw_box_shadow_ambient(
        // 第二个内部矩形。
        Rect::new(3.0, 1.0, 2.0, 2.0),
        // 正 blur 覆盖环境曲线。
        1.0,
        // 无额外水平偏移。
        0.0,
        // 无额外垂直偏移。
        0.0,
        // 蓝色区分第二种阴影。
        Color::blue(),
        // 使用统一小圆角。
        Some(Radius::uniform(0.5)),
    );
    // 在三角形外绘制本应可见的红色阴影，验证 path mask 不只使用 AABB。
    canvas.draw_box_shadow(
        // 右下矩形位于三角形外。
        Rect::new(9.0, 5.0, 2.0, 2.0),
        // 零 blur 避免贡献跨回路径内部。
        0.0,
        // 无水平偏移。
        0.0,
        // 无垂直偏移。
        0.0,
        // 红色便于识别错误泄漏。
        Color::red(),
        // 使用直角。
        None,
    );
    // 恢复完整裁剪，避免后续状态泄漏。
    canvas.pop_clip();
    // 完成记录并取得命令事实。
    let encoder = match canvas.finish_recording() {
        // 保存成功结果。
        Ok(encoder) => encoder,
        // 合法路径阴影不应失败。
        Err(error) => panic!("path-clipped additive shadows should finish: {error:?}"),
    };
    // 三笔阴影必须共同封装为唯一 Additive sampled tile。
    assert!(matches!(
        // 读取稳定命令切片。
        encoder.commands(),
        // 禁止退化为 destination-dependent CpuSegment。
        [
            FrameCommand::Clear { .. },
            FrameCommand::PictureBlit { additive: true, .. }
        ]
    ));
    // 执行 CPU 参考路径检查真实 coverage mask。
    let reference = encoder.render_reference();
    // 安全内部完整 coverage 绿色应只应用一次半透明 opacity。
    assert_eq!(reference.pixel(2, 2), Some(0x7f00_7f00));
    // 环境阴影必须保留非零蓝色贡献。
    assert_ne!(reference.pixel(4, 2), Some(0));
    // 三角形外的红色阴影必须被路径 mask 完全裁掉。
    assert_eq!(reference.pixel(10, 6), Some(0));
}

// blend 与 restore 必须分隔阴影批次，非法输入保持稳定边界。
#[test]
fn additive_shadows_preserve_barriers_and_reject_invalid_state() {
    // 创建容纳三个互不重叠阴影的画布。
    let mut canvas = FrameRecordingCanvas::new(24, 8);
    // 开始正式记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("shadow barrier recording should begin: {error:?}");
    }
    // 非 identity transform 让中间 SrcOver 阴影明确进入普通 CPU segment。
    canvas.set_transform(Transform::scale(2.0, 1.0));
    // 建立随后可由 restore 找回的 Additive 状态。
    canvas.set_blend_mode(BlendMode::Additive);
    // 保存 Additive blend 与 transform。
    canvas.save();
    // 第一笔绿色定向阴影进入 Additive sampled segment。
    canvas.draw_box_shadow(
        // 左侧局部矩形。
        Rect::new(1.0, 2.0, 2.0, 2.0),
        // 零 blur。
        0.0,
        // 无水平偏移。
        0.0,
        // 无垂直偏移。
        0.0,
        // 绿色源。
        Color::green(),
        // 直角阴影。
        None,
    );
    // 切到 SrcOver 必须先封口第一段。
    canvas.set_blend_mode(BlendMode::SrcOver);
    // 中间蓝色环境阴影进入普通 CpuSegment。
    canvas.draw_box_shadow_ambient(
        // 中间局部矩形。
        Rect::new(5.0, 2.0, 2.0, 2.0),
        // 正 blur 覆盖环境分支。
        1.0,
        // 无水平偏移。
        0.0,
        // 无垂直偏移。
        0.0,
        // 蓝色源。
        Color::blue(),
        // 直角阴影。
        None,
    );
    // restore 必须先封口普通段并恢复 Additive。
    canvas.restore();
    // 右侧绿色环境阴影进入新的 Additive sampled segment。
    canvas.draw_box_shadow_ambient(
        // 右侧局部矩形。
        Rect::new(9.0, 2.0, 2.0, 2.0),
        // 正 blur。
        1.0,
        // 无水平偏移。
        0.0,
        // 无垂直偏移。
        0.0,
        // 绿色源。
        Color::green(),
        // 直角阴影。
        None,
    );
    // 完成记录并取得命令顺序。
    let encoder = match canvas.finish_recording() {
        // 保存成功结果。
        Ok(encoder) => encoder,
        // 合法 painter barrier 不应失败。
        Err(error) => panic!("shadow barriers should finish: {error:?}"),
    };
    // 精确匹配 Additive、SrcOver、Additive 三个 painter 分段。
    assert!(matches!(
        // 读取稳定命令切片。
        encoder.commands(),
        // 两个 sampled tile 不得跨 barrier 合并。
        [
            FrameCommand::Clear { .. },
            FrameCommand::PictureBlit { additive: true, .. },
            FrameCommand::CpuSegment { .. },
            FrameCommand::PictureBlit { additive: true, .. }
        ]
    ));

    // 创建独立画布验证透明、空几何和非法效果参数安全 no-op。
    let mut malformed = FrameRecordingCanvas::new(6, 6);
    // 开始 malformed 边界记录。
    if let Err(error) = malformed.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("malformed shadow recording should begin: {error:?}");
    }
    // 选择 Additive source scratch。
    malformed.set_blend_mode(BlendMode::Additive);
    // 空宽度阴影不得产生像素。
    malformed.draw_box_shadow(
        // 零宽局部矩形。
        Rect::new(1.0, 1.0, 0.0, 2.0),
        // 合法 blur。
        1.0,
        // 无水平偏移。
        0.0,
        // 无垂直偏移。
        0.0,
        // 可见颜色仍不得越过空几何门禁。
        Color::red(),
        // 直角阴影。
        None,
    );
    // 非有限 blur 保持安全 no-op。
    malformed.draw_box_shadow_ambient(
        // 合法局部矩形。
        Rect::new(1.0, 1.0, 2.0, 2.0),
        // NaN 无法形成稳定局部 coverage。
        f32::NAN,
        // 无水平偏移。
        0.0,
        // 无垂直偏移。
        0.0,
        // 可见颜色仍不得写入。
        Color::blue(),
        // 直角阴影。
        None,
    );
    // 完成 no-op 记录。
    let malformed_encoder = match malformed.finish_recording() {
        // 保存成功结果。
        Ok(encoder) => encoder,
        // 安全 no-op 不应转成错误。
        Err(error) => panic!("malformed shadows should remain no-op: {error:?}"),
    };
    // 借用 no-op 记录的稳定命令切片。
    let malformed_commands = malformed_encoder.commands();
    // 只有初始 clear 可以存在。
    assert!(matches!(malformed_commands, [FrameCommand::Clear { .. }]));

    // 创建独立画布验证非有限 opacity 的 typed failure。
    let mut non_finite = FrameRecordingCanvas::new(6, 6);
    // 开始错误边界记录。
    if let Err(error) = non_finite.begin_recording(true) {
        // 合法尺寸不得初始化失败。
        panic!("non-finite shadow recording should begin: {error:?}");
    }
    // NaN opacity 必须保持不可烘焙状态。
    non_finite.set_opacity(f32::NAN);
    // 选择会进入 sampled scratch 的 Additive。
    non_finite.set_blend_mode(BlendMode::Additive);
    // 尝试记录一个 otherwise 合法的阴影。
    non_finite.draw_box_shadow(
        // 合法局部矩形。
        Rect::new(1.0, 1.0, 2.0, 2.0),
        // 合法 blur。
        1.0,
        // 无水平偏移。
        0.0,
        // 无垂直偏移。
        0.0,
        // 可见颜色。
        Color::green(),
        // 直角阴影。
        None,
    );
    // 帧边界必须返回延迟的 typed failure。
    let error = match non_finite.finish_recording() {
        // 成功会静默吞掉 NaN，属于语义回归。
        Ok(_) => panic!("non-finite additive shadow opacity should fail"),
        // 保存真实错误供错误码审计。
        Err(error) => error,
    };
    // 保持稳定的未实现错误码。
    assert_eq!(error.code(), crate::core::Errc::NotImplemented);
    // 失败前不得提交半成品阴影命令。
    assert_eq!(
        // 测试模块可直接读取仍保留的内部编码器。
        non_finite
            // begin_recording 已建立编码器。
            .encoder
            // 借用失败后保留的命令载荷。
            .as_ref()
            // 只统计命令数，避免消费画布。
            .map(|encoder| encoder.commands().len()),
        // 初始 clear 是唯一允许存在的命令。
        Some(1)
    );
}

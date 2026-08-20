// 拆分自 canvas2d.rs 内联测试：Additive 基础形状的录制与参考像素回归。
// 与 canvas2d.rs 的 mod tests 共享 `use super::*` 与 `use crate::draw::painting::FrameCommand`。

    fn records_additive_stroke_and_reference_adds_destination() {
        // 创建能够容纳两个互不相交描边的 recorder 画布。
        let mut canvas = FrameRecordingCanvas::new(16, 8);
        // 开始一帧带透明 clear 的正式记录。
        if let Err(error) = canvas.begin_recording(true) {
            // 合法尺寸的记录初始化不得失败。
            panic!("additive stroke recording should begin: {error:?}");
        }
        // 先用不透明红色建立可观察的累计目标。
        canvas.fill_rect(Rect::new(0.0, 0.0, 16.0, 8.0), Color::red(), None);
        // 后续描边切换到目标相关 Additive 混合。
        canvas.set_blend_mode(BlendMode::Additive);
        // 记录一个整数轴对齐直角矩形描边。
        canvas.stroke_rect(Rect::new(1.0, 1.0, 4.0, 4.0), Color::green(), 1.0, None);
        // 记录一个与前一描边互不相交的整数轴对齐圆形描边。
        canvas.stroke_circle(12.0, 3.0, 2.0, Color::green(), 1.0);
        // 完成记录并取得不可变命令流。
        let encoder = match canvas.finish_recording() {
            // 保存成功的编码器供载荷和像素审计。
            Ok(encoder) => encoder,
            // 合法 Additive 描边不应产生 deferred failure。
            Err(error) => panic!("additive stroke recording should finish: {error:?}"),
        };
        // 命令流应严格为 clear、红色底和一个 Additive 描边批次。
        let [FrameCommand::Clear { .. }, FrameCommand::Native {
            operation: FrameRasterOp::FillRect { .. },
        }, FrameCommand::Native {
            operation:
                FrameRasterOp::StrokeRoundedRects {
                    strokes,
                    additive: true,
                    ..
                },
        }] = encoder.commands()
        else {
            // 任何额外 CPU segment 或拆错顺序都说明记录路径退化。
            panic!("expected clear, fill, and one additive stroke batch");
        };
        // 相同 clip/blend 且互不相交的矩形和圆应安全合为一批。
        assert_eq!(strokes.len(), 2);
        // 第二条圆形描边必须保留半径事实而不是退化为直角矩形。
        assert_eq!(strokes[1].radius().to_radius().tl, 2.0);
        // 执行 CPU 参考路径以核验真实目标相关混合。
        let reference = encoder.render_reference();
        // 红底上的绿色 Additive 描边应逐通道饱和为黄色。
        assert_eq!(
            reference.pixel(1, 1),
            Some(Color::from_rgb(255, 255, 0).premultiplied())
        );
        // 描边内部未覆盖像素必须继续保持原始红色目标。
        assert_eq!(reference.pixel(2, 2), Some(Color::red().premultiplied()));
    }

    // 合法 Additive 填充圆必须直接保留为圆角 shape，并对累计目标执行加法。
    #[test]
    fn records_additive_fill_circle_as_rounded_shape() {
        // 创建能够明确区分圆内外像素的录制画布。
        let mut canvas = FrameRecordingCanvas::new(8, 8);
        // 开始一帧带透明 clear 的正式记录。
        if let Err(error) = canvas.begin_recording(true) {
            // 合法尺寸的记录初始化不得失败。
            panic!("additive circle recording should begin: {error:?}");
        }
        // 先用不透明红色建立可观察的累计目标。
        canvas.fill_rect(Rect::new(0.0, 0.0, 8.0, 8.0), Color::red(), None);
        // 后续圆形填充切换到目标相关 Additive 混合。
        canvas.set_blend_mode(BlendMode::Additive);
        // 记录边界完全位于 surface 内的整数圆。
        canvas.fill_circle(4.0, 4.0, 2.0, Color::green());
        // 完成记录并取得不可变命令流。
        let encoder = match canvas.finish_recording() {
            // 保存成功的编码器供载荷和像素审计。
            Ok(encoder) => encoder,
            // 合法 Additive 圆不应产生 deferred failure。
            Err(error) => panic!("additive circle recording should finish: {error:?}"),
        };
        // 精确匹配命令序列，同时证明没有插入透明 CPU segment。
        let [FrameCommand::Clear { .. }, FrameCommand::Native {
            operation: FrameRasterOp::FillRect { .. },
        }, FrameCommand::Native {
            operation:
                FrameRasterOp::FillRoundedRectAdditive {
                    rect,
                    color,
                    radius,
                    clip,
                },
        }] = encoder.commands()
        else {
            // 任何 CPU segment 或普通 blend shape 都说明准入路径仍然错误。
            panic!("expected clear, fill, and one additive rounded circle");
        };
        // 圆应保留为以 (2, 2) 起始的 4×4 正方形。
        assert_eq!(*rect, FrameRect::new(2, 2, 4, 4));
        // Additive shape 必须保留调用方提供的绿色源色。
        assert_eq!(*color, Color::green());
        // 四角半径应等于原始圆半径，避免退化为直角矩形。
        assert_eq!(radius.to_radius(), Radius::uniform(2.0));
        // 未设置局部裁剪时载荷必须显式保存完整 surface。
        assert_eq!(*clip, FrameRect::new(0, 0, 8, 8));
        // 执行 CPU 参考路径以核验真实目标相关混合。
        let reference = encoder.render_reference();
        // 红底圆心叠加绿色后应逐通道饱和为黄色。
        assert_eq!(
            reference.pixel(4, 4),
            Some(Color::from_rgb(255, 255, 0).premultiplied())
        );
        // 圆外像素不得受 Additive shape 影响。
        assert_eq!(reference.pixel(0, 0), Some(Color::red().premultiplied()));
    }

    // Additive 填充与描边必须共用当前局部矩形裁剪，空裁剪则保持 no-op。
    #[test]
    fn additive_shapes_preserve_partial_rect_clip() {
        // 创建能够跨越裁剪边界并保留未裁剪目标的录制画布。
        let mut canvas = FrameRecordingCanvas::new(12, 10);
        // 开始一帧带透明 clear 的正式记录。
        if let Err(error) = canvas.begin_recording(true) {
            // 合法尺寸的记录初始化不得失败。
            panic!("clipped additive recording should begin: {error:?}");
        }
        // 先用不透明红色建立可观察的累计目标。
        canvas.fill_rect(Rect::new(0.0, 0.0, 12.0, 10.0), Color::red(), None);
        // 将后续操作限制在左半侧整数矩形内。
        canvas.push_clip(Rect::new(0.0, 0.0, 6.0, 10.0));
        // 后续填充与描边切换到目标相关 Additive 混合。
        canvas.set_blend_mode(BlendMode::Additive);
        // 记录一个横跨 x=6 裁剪边界的圆形填充。
        canvas.fill_circle(6.0, 3.0, 2.0, Color::green());
        // 记录一个同样横跨裁剪边界的直角矩形描边。
        canvas.stroke_rect(Rect::new(4.0, 6.0, 4.0, 3.0), Color::green(), 1.0, None);
        // 追加与当前裁剪完全不相交的子裁剪以形成空裁剪。
        canvas.push_clip(Rect::new(20.0, 0.0, 2.0, 2.0));
        // 空裁剪下的合法 Additive 矩形必须成为 no-op。
        canvas.fill_rect(Rect::new(1.0, 1.0, 2.0, 2.0), Color::blue(), None);
        // 完成记录并取得不可变命令流。
        let encoder = match canvas.finish_recording() {
            // 保存成功的编码器供载荷和像素审计。
            Ok(encoder) => encoder,
            // 合法局部裁剪不应产生 deferred failure。
            Err(error) => panic!("clipped additive recording should finish: {error:?}"),
        };
        // 精确匹配命令序列，同时证明空裁剪没有追加第五条命令。
        let [FrameCommand::Clear { .. }, FrameCommand::Native {
            operation: FrameRasterOp::FillRect { .. },
        }, FrameCommand::Native {
            operation:
                FrameRasterOp::FillRoundedRectAdditive {
                    clip: fill_clip, ..
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
            // CPU segment、错误批次或空裁剪命令都会破坏这一精确事实。
            panic!("expected clipped additive fill and stroke commands");
        };
        // 填充载荷必须保存左半侧逻辑裁剪。
        assert_eq!(*fill_clip, FrameRect::new(0, 0, 6, 10));
        // 描边批次必须保存与填充完全相同的逻辑裁剪。
        assert_eq!(*stroke_clip, FrameRect::new(0, 0, 6, 10));
        // 当前调用只应产生一条裁剪描边。
        assert_eq!(strokes.len(), 1);
        // 执行 CPU 参考路径以核验裁剪前后的目标相关像素。
        let reference = encoder.render_reference();
        // 裁剪内的圆心左侧像素应由红绿相加得到黄色。
        assert_eq!(
            reference.pixel(5, 3),
            Some(Color::from_rgb(255, 255, 0).premultiplied())
        );
        // 裁剪外的相邻圆内像素必须继续保持原始红色目标。
        assert_eq!(reference.pixel(6, 3), Some(Color::red().premultiplied()));
    }

    // 有限整数 offset 必须平移 Additive 几何，同时保持 surface-space clip 不变。
    #[test]
    fn additive_shapes_map_integral_offsets_without_moving_clip_twice() {
        // 创建能够容纳正负平移后几何的录制画布。
        let mut canvas = FrameRecordingCanvas::new(14, 10);
        // 开始一帧带透明 clear 的正式记录。
        if let Err(error) = canvas.begin_recording(true) {
            // 合法尺寸的记录初始化不得失败。
            panic!("offset additive recording should begin: {error:?}");
        }
        // 先用不透明红色建立可观察的累计目标。
        canvas.fill_rect(Rect::new(0.0, 0.0, 14.0, 10.0), Color::red(), None);
        // 后续几何先应用正整数像素 offset。
        canvas.set_offset(2.0, 1.0);
        // 局部 clip 也在当前状态下映射一次到 surface 坐标。
        canvas.push_clip(Rect::new(1.0, 1.0, 3.0, 4.0));
        // 后续填充与描边切换到目标相关 Additive 混合。
        canvas.set_blend_mode(BlendMode::Additive);
        // 本地圆形边界 (1,1,4,4) 应平移为 surface 矩形 (3,2,4,4)。
        canvas.fill_circle(3.0, 3.0, 2.0, Color::green());
        // 恢复完整 surface clip，避免后一命令与前一命令共享裁剪。
        canvas.pop_clip();
        // 切换到负整数 offset 以覆盖反方向映射。
        canvas.set_offset(-2.0, -1.0);
        // 本地矩形 (4,6,4,3) 应平移为 surface 矩形 (2,5,4,3)。
        canvas.stroke_rect(Rect::new(4.0, 6.0, 4.0, 3.0), Color::green(), 1.0, None);
        // 完成记录并取得不可变命令流。
        let encoder = match canvas.finish_recording() {
            // 保存成功的编码器供载荷和像素审计。
            Ok(encoder) => encoder,
            // 合法整数 offset 不应产生 deferred failure。
            Err(error) => panic!("offset additive recording should finish: {error:?}"),
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
            panic!("expected offset additive fill and stroke commands");
        };
        // 正 offset 必须只平移圆的正方形几何一次。
        assert_eq!(*fill_rect, FrameRect::new(3, 2, 4, 4));
        // 当前 clip 已是 surface 坐标，必须保持 (3,2,3,4) 而不能再次平移。
        assert_eq!(*fill_clip, FrameRect::new(3, 2, 3, 4));
        // 负 offset 下当前调用只应产生一条描边。
        assert_eq!(strokes.len(), 1);
        // 负 offset 必须把本地描边矩形平移到 surface 左上侧。
        assert_eq!(strokes[0].rect(), FrameRect::new(2, 5, 4, 3));
        // pop_clip 后的描边必须恢复完整 surface 裁剪。
        assert_eq!(*stroke_clip, FrameRect::new(0, 0, 14, 10));
        // 执行 CPU 参考路径以核验平移和裁剪后的真实目标像素。
        let reference = encoder.render_reference();
        // 正 offset 后裁剪内的圆形像素应由红绿相加得到黄色。
        assert_eq!(
            reference.pixel(5, 3),
            Some(Color::from_rgb(255, 255, 0).premultiplied())
        );
        // x=6 位于圆内但在 surface-space clip 外，必须保持原始红色。
        assert_eq!(reference.pixel(6, 3), Some(Color::red().premultiplied()));
        // 负 offset 后的描边左上像素也应对红色目标执行加法。
        assert_eq!(
            reference.pixel(2, 5),
            Some(Color::from_rgb(255, 255, 0).premultiplied())
        );
    }

    // 分数 offset 无法进入固定 Native shape 时应保留在 Additive sampled soft 分段中。
    #[test]
    fn additive_fill_preserves_fractional_offset_in_sampled_segment() {
        // 创建一个最小但足以容纳测试矩形的录制画布。
        let mut canvas = FrameRecordingCanvas::new(8, 8);
        // 开始一帧带透明 clear 的正式记录。
        if let Err(error) = canvas.begin_recording(true) {
            // 合法尺寸的记录初始化不得失败。
            panic!("fractional offset recording should begin: {error:?}");
        }
        // 设置不能无损映射为 FrameRect 的水平分数 offset。
        canvas.set_offset(0.5, 0.0);
        // 选择目标相关 Additive 混合。
        canvas.set_blend_mode(BlendMode::Additive);
        // 记录一个 otherwise 合法的整数矩形。
        canvas.fill_rect(Rect::new(1.0, 1.0, 2.0, 2.0), Color::green(), None);
        // 完成记录并取得用于审计命令和像素的编码器。
        let encoder = match canvas.finish_recording() {
            // 合法分数 offset 应由软件光栅保真处理。
            Ok(encoder) => encoder,
            // typed failure 表示新 fallback 没有覆盖该几何。
            Err(error) => panic!("fractional additive offset should finish: {error:?}"),
        };
        // 分数几何不能伪装成整数 Native shape 或 SrcOver CPU segment。
        assert!(matches!(
            encoder.commands(),
            [
                FrameCommand::Clear { .. },
                FrameCommand::PictureBlit { additive: true, .. }
            ]
        ));
        // 矩形内部的完全覆盖像素应保留原始绿色源贡献。
        assert_eq!(
            encoder.render_reference().pixel(2, 2),
            Some(Color::green().premultiplied())
        );
    }

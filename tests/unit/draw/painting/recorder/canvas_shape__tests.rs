// 引入当前模块公开的录制实现与命令值。
use super::*;
// 引入命令枚举供结构断言。
use crate::draw::painting::FrameCommand;

// 分数矩形填充和描边必须保持 GPU 原生命令来源。
#[test]
fn fractional_src_over_rects_remain_gpu_native() {
    // 创建容纳两条亚像素 shape 的最小画布。
    let mut canvas = FrameRecordingCanvas::new(16, 12);
    // 开始带透明清理的正式帧记录。
    if let Err(error) = canvas.begin_recording(true) {
        // 合法尺寸不得拒绝记录。
        panic!("fractional shape recording should begin: {error:?}");
    }
    // 记录一条带分数边界的圆角填充。
    canvas.fill_rect(
        // 保留真实亚像素位置与尺寸。
        Rect::new(1.25, 1.5, 5.5, 4.25),
        // 使用不透明绿色便于参考像素断言。
        Color::green(),
        // 使用合法圆角覆盖 SDF 分支。
        Some(Radius::uniform(1.0)),
    );
    // 记录一条带分数边界的圆角描边。
    canvas.stroke_rect(
        // 保留另一组真实亚像素边界。
        Rect::new(8.25, 2.5, 5.25, 5.0),
        // 使用不透明红色区分描边。
        Color::red(),
        // 使用正有限线宽。
        1.5,
        // 使用合法圆角覆盖描边 SDF 分支。
        Some(Radius::uniform(1.0)),
    );
    // 完成记录并取得命令流。
    let encoder = match canvas.finish_recording() {
        // 保存合法编码器。
        Ok(encoder) => encoder,
        // 任何软件回退或状态错误都直接报告。
        Err(error) => panic!("fractional shape recording should finish: {error:?}"),
    };
    // 命令流必须只包含透明清理和两条亚像素原生命令。
    assert!(matches!(
        encoder.commands(),
        [
            FrameCommand::Clear { .. },
            FrameCommand::Native {
                operation: FrameRasterOp::FillRoundedRectSubpixel { .. }
            },
            FrameCommand::Native {
                operation: FrameRasterOp::StrokeRoundedRectSubpixel { .. }
            }
        ]
    ));
    // GPU 来源审计不得发现 CPU 光栅载荷。
    assert!(encoder.gpu_native_audit().is_gpu_native());
    // CPU 参考执行必须实际覆盖填充内部像素。
    assert_eq!(
        // 读取远离圆角边缘的绿色像素。
        encoder.render_reference().pixel(3, 3),
        // 期望完整绿色源贡献。
        Some(Color::green().premultiplied())
    );
}

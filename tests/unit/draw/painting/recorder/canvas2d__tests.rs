// 引入当前 recorder 实现和 Canvas2D 依赖类型。
use super::*;
// 引入命令枚举以审计实际记录载荷。
use crate::draw::painting::FrameCommand;

// 加载 Additive 基础形状的独立回归测试。
include!("canvas2d_shape_tests.rs");

// 继续在同一测试模块内加载纯平移 transform 的独立回归测试。
include!("canvas2d_test_tail.rs");

// 继续在同一测试模块内加载 Additive sampled soft 分段回归测试。
include!("canvas2d_additive_soft_tests.rs");

// 继续加载 Additive 仿射描边 sampled soft 分段回归测试。
include!("canvas2d_additive_stroke_soft_tests.rs");

// 继续在同一测试模块内加载 Additive opacity 的独立回归测试。
include!("canvas2d_opacity_tests.rs");

// 完整共享图片必须原样进入 FrameImage，并在调用方释放后继续由帧命令拥有。
#[test]
fn shared_full_image_reuses_pixels_until_frame_command_drops() {
    let source = std::sync::Arc::new(vec![0xFFFF_0000, 0xFF00_FF00, 0xFF00_00FF, 0x8000_0080]);
    let source_pointer = source.as_ptr();
    let mut canvas = FrameRecordingCanvas::new(2, 2);
    canvas
        .begin_recording(false)
        .expect("共享图片帧应可开始录制");
    canvas.blit_image_shared(
        std::sync::Arc::clone(&source),
        2,
        Rect::new(0.0, 0.0, 2.0, 2.0),
        Rect::new(0.0, 0.0, 2.0, 2.0),
    );
    let encoder = canvas.finish_recording().expect("共享图片帧应可完成录制");
    let [FrameCommand::PictureBlit { image, .. }] = encoder.commands() else {
        panic!("完整共享图片必须形成单一 PictureBlit");
    };
    assert_eq!(image.pixels().as_ptr(), source_pointer);
    assert_eq!(std::sync::Arc::strong_count(&source), 2);

    drop(source);
    assert_eq!(
        encoder.render_reference().pixels(),
        [0xFFFF_0000, 0xFF00_FF00, 0xFF00_00FF, 0x8000_0080]
    );
}

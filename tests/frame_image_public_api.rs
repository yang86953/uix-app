//! 《组件 · 合成帧与视频帧》的外部消费者值、线程与所有权契约。
//! 不创建渲染后端，不断言 GPU 像素、缓存或内部录制结构。

use std::sync::Arc;

use uix_app::draw::{Color, FrameEncoderError, FrameImage};
use uix_app::prelude::*;

#[test]
fn owned_frame_keeps_tightly_packed_premultiplied_pixels() {
    let pixels = vec![0xffff0000, 0x80008000, 0xff0000ff, 0];
    let frame = FrameImage::new(2, 2, pixels.clone()).unwrap();
    assert_eq!((frame.width(), frame.height()), (2, 2));
    assert_eq!(frame.pixels(), pixels);
}

#[test]
fn invalid_frame_extents_and_lengths_fail_before_drawing() {
    for (width, height) in [(0, 1), (1, 0), (-1, 2), (2, -1)] {
        assert!(matches!(
            FrameImage::new(width, height, vec![]),
            Err(FrameEncoderError::InvalidExtent { .. })
        ));
        assert!(FrameImage::from_shared(width, height, Arc::new(vec![])).is_err());
    }
    for pixels in [vec![0; 3], vec![0; 5]] {
        assert!(matches!(
            FrameImage::new(2, 2, pixels.clone()),
            Err(FrameEncoderError::PixelCountMismatch { .. })
        ));
        assert!(FrameImage::from_shared(2, 2, Arc::new(pixels)).is_err());
    }
}

#[test]
fn solid_frame_converts_straight_color_to_premultiplied_argb() {
    let frame = FrameImage::solid(2, 1, Color::from_rgba(255, 0, 0, 128)).unwrap();
    assert_eq!(frame.pixels(), &[0x80800000, 0x80800000]);
}

#[test]
fn frame_construction_errors_integrate_with_standard_error_propagation() {
    fn build() -> Result<FrameImage, Box<dyn std::error::Error + Send + Sync>> {
        Ok(FrameImage::new(0, 1, vec![])?)
    }
    let error = build().unwrap_err();
    assert!(matches!(
        error.downcast_ref::<FrameEncoderError>(),
        Some(FrameEncoderError::InvalidExtent {
            width: 0,
            height: 1
        })
    ));
}

#[test]
fn shared_frame_retains_pixels_until_last_consumer_drops() {
    let pixels = Arc::new(vec![0xff123456; 4]);
    let lease = Arc::downgrade(&pixels);
    let frame = FrameImage::from_shared(2, 2, Arc::clone(&pixels)).unwrap();
    assert_eq!(frame.pixels().as_ptr(), pixels.as_ptr());
    let retained = frame.clone();
    drop(pixels);
    drop(frame);
    assert!(lease.upgrade().is_some());
    assert_eq!(retained.pixels(), &[0xff123456; 4]);
    drop(retained);
    assert!(lease.upgrade().is_none());
}

#[test]
fn producer_can_transfer_owned_frame_across_thread_without_native_resources() {
    fn require_send_sync<T: Send + Sync>() {}
    require_send_sync::<FrameImage>();

    let pixels = Arc::new(vec![0xffabcdef; 2]);
    let lease = Arc::downgrade(&pixels);
    let frame = std::thread::spawn(move || FrameImage::from_shared(2, 1, pixels).unwrap())
        .join()
        .unwrap();
    assert_eq!(frame.pixels(), &[0xffabcdef; 2]);
    drop(frame);
    assert!(lease.upgrade().is_none());
}

#[test]
fn replacing_and_clearing_application_state_releases_its_own_frame_references() {
    let pixels = Arc::new(vec![0xff112233]);
    let lease = Arc::downgrade(&pixels);
    let current = State::new(Some(FrameImage::from_shared(1, 1, pixels).unwrap()));
    current.set(Some(FrameImage::solid(2, 1, Color::BLUE).unwrap()));
    assert!(lease.upgrade().is_none());
    current.set(None);
    assert!(current.get().is_none());
}

// 编译文档的真实组件绘制入口；不把创建 ViewNode 冒充像素或呈现验收。
#[allow(dead_code)]
fn frame_canvas(current: State<Option<FrameImage>>) -> ViewNode {
    canvas(640.0, 360.0, move |bounds, ctx| {
        ctx.fill_rect(bounds, Color::BLACK, None);
        if let Some(frame) = current.get() {
            ctx.draw_frame_image(&frame, bounds);
        }
    })
}

#[allow(dead_code)]
fn frame_stretch_canvas(frame: FrameImage) -> ViewNode {
    canvas(160.0, 160.0, move |bounds, ctx| {
        ctx.draw_frame_image_fill(&frame, bounds);
        ctx.with_opacity(0.5, |draw| draw.draw_frame_image(&frame, bounds));
    })
}

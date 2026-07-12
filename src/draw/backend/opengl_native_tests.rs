#![cfg(feature = "opengles")]

use crate::core::{DamageRegion, Rect};
use crate::draw::engine::GraphicsFailure;
use crate::draw::gpu_engine::GpuEngine;
use crate::draw::{BlendMode, Color, GraphicsEngine, UpdateStrategy};
use crate::native::traits::present::GraphicsBackend;

fn open_engine(
    title: &str,
    width: i32,
    height: i32,
) -> (
    Box<dyn crate::native::traits::platform::Platform>,
    Box<dyn crate::native::traits::window::PlatformWindow>,
    GpuEngine,
) {
    let mut platform = crate::native::create_platform().expect("platform");
    let window = platform
        .window_manager()
        .create_window(title, width, height)
        .expect("window");
    let context = crate::native::create_gpu_context_with_backend(
        window.native_surface_ptr(),
        width,
        height,
        GraphicsBackend::OpenGlEs,
    )
    .expect("WglContext");
    let mut engine = GpuEngine::new(context).expect("OpenGL ES GpuEngine");
    engine
        .initialize(width, height)
        .expect("initialize GL engine");
    (platform, window, engine)
}

#[test]
fn opengles_native_path_keeps_order_and_bounded_soft_upload() {
    if std::env::consts::OS != "windows" {
        return;
    }

    let (_platform, mut window, mut engine) = open_engine("native GL order", 128, 128);
    let _ = engine.begin_frame(UpdateStrategy::FullRedraw);

    let picture = engine.create_offscreen(32, 32).expect("offscreen picture");
    assert!(engine.begin_offscreen_paint(&picture));
    engine
        .offscreen_canvas(&picture)
        .expect("offscreen canvas")
        .fill_rect(Rect::new(0.0, 0.0, 32.0, 32.0), Color::blue(), None);
    engine.flush_offscreen_paint(&picture);
    engine.end_offscreen_paint();

    engine
        .canvas_2d()
        .fill_ellipse(Rect::new(0.0, 48.0, 32.0, 32.0), Color::red());
    engine.blit_offscreen(&picture, Rect::new(32.0, 32.0, 64.0, 64.0));
    engine
        .canvas_2d()
        .fill_rect(Rect::new(60.0, 60.0, 8.0, 8.0), Color::green(), None);

    let backend = engine
        .session_mut()
        .native_gpu_backend_mut()
        .expect("OpenGL ES NativeGpu backend");
    let pixels = backend.try_readback().expect("read back ordered frame");
    let pixel = |x: usize, y: usize| pixels[y * 128 + x];
    assert_eq!(pixel(20, 64), 0xFF00_00FF, "CPU segment precedes Picture");
    assert_eq!(pixel(40, 40), 0xFFFF_0000, "Picture follows CPU segment");
    assert_eq!(pixel(62, 62), 0xFF00_FF00, "native draw follows Picture");
    assert!(
        backend.last_soft_upload_bytes() < 128 * 128 * 4,
        "bounded CPU segment must not upload a full texture"
    );

    assert!(matches!(
        engine.end_frame(&DamageRegion::full()),
        crate::draw::RenderOutcome::Present(_)
    ));
    engine.destroy_offscreen(picture);
    engine.shutdown();
    window.close().expect("close native window");
}

#[test]
fn opengles_native_path_honors_picture_source_crop() {
    if std::env::consts::OS != "windows" {
        return;
    }

    let (_platform, mut window, mut engine) = open_engine("native GL crop", 128, 128);
    let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
    let picture = engine.create_offscreen(32, 32).expect("offscreen picture");
    assert!(engine.begin_offscreen_paint(&picture));
    let canvas = engine.offscreen_canvas(&picture).expect("offscreen canvas");
    canvas.fill_rect(Rect::new(0.0, 0.0, 32.0, 32.0), Color::green(), None);
    canvas.fill_rect(Rect::new(0.0, 0.0, 16.0, 32.0), Color::red(), None);
    engine.flush_offscreen_paint(&picture);
    engine.end_offscreen_paint();

    engine.blit_offscreen_src(
        &picture,
        Rect::new(0.0, 0.0, 16.0, 32.0),
        Rect::new(0.0, 0.0, 48.0, 48.0),
    );
    engine.blit_offscreen_src(
        &picture,
        Rect::new(16.0, 0.0, 16.0, 32.0),
        Rect::new(64.0, 0.0, 48.0, 48.0),
    );
    engine.blit_offscreen(&picture, Rect::new(0.0, 64.0, 64.0, 64.0));

    let backend = engine
        .session_mut()
        .native_gpu_backend_mut()
        .expect("OpenGL ES NativeGpu backend");
    let pixels = backend.try_readback().expect("read back cropped frame");
    let pixel = |x: usize, y: usize| pixels[y * 128 + x];
    assert_eq!(
        (pixel(24, 104), pixel(88, 104), pixel(16, 24), pixel(48, 24)),
        (0xFF00_00FF, 0xFF00_FF00, 0xFF00_00FF, 0xFF00_FF00),
        "source crop and full source extent preserve Picture pixels"
    );

    assert!(matches!(
        engine.end_frame(&DamageRegion::full()),
        crate::draw::RenderOutcome::Present(_)
    ));
    engine.destroy_offscreen(picture);
    engine.shutdown();
    window.close().expect("close native window");
}

#[test]
fn opengles_native_path_keeps_order_when_a_picture_blits_into_an_active_picture() {
    if std::env::consts::OS != "windows" {
        return;
    }

    let (_platform, mut window, mut engine) = open_engine("native GL nested Picture", 128, 128);
    let _ = engine.begin_frame(UpdateStrategy::FullRedraw);

    let source = engine.create_offscreen(16, 16).expect("source Picture");
    assert!(engine.begin_offscreen_paint(&source));
    engine
        .offscreen_canvas(&source)
        .expect("source canvas")
        .fill_rect(Rect::new(0.0, 0.0, 16.0, 16.0), Color::red(), None);
    engine.flush_offscreen_paint(&source);
    engine.end_offscreen_paint();

    let destination = engine
        .create_offscreen(32, 32)
        .expect("destination Picture");
    assert!(engine.begin_offscreen_paint(&destination));
    engine
        .offscreen_canvas(&destination)
        .expect("destination canvas")
        .fill_rect(Rect::new(0.0, 0.0, 32.0, 32.0), Color::green(), None);
    engine
        .try_blit_offscreen_src(
            &source,
            Rect::new(0.0, 0.0, 16.0, 16.0),
            Rect::new(0.0, 0.0, 16.0, 16.0),
        )
        .expect("source Picture blit into active destination");
    engine
        .try_end_offscreen_paint()
        .expect("restore swapchain after destination Picture");

    engine.blit_offscreen(&destination, Rect::new(0.0, 0.0, 64.0, 64.0));
    let pixels = engine
        .session_mut()
        .native_gpu_backend_mut()
        .expect("OpenGL ES NativeGpu backend")
        .try_readback()
        .expect("read back nested Picture frame");
    let pixel = |x: usize, y: usize| pixels[y * 128 + x];
    assert_eq!(
        (pixel(16, 80), pixel(48, 112)),
        (0xFF00_00FF, 0xFF00_FF00),
        "later source Picture blit must follow the destination's earlier work"
    );

    assert!(matches!(
        engine.end_frame(&DamageRegion::full()),
        crate::draw::RenderOutcome::Present(_)
    ));
    engine.destroy_offscreen(source);
    engine.destroy_offscreen(destination);
    engine.shutdown();
    window.close().expect("close native window");
}

#[test]
fn opengles_native_path_rejects_additive_cpu_fallback() {
    if std::env::consts::OS != "windows" {
        return;
    }

    let (_platform, mut window, mut engine) = open_engine("native GL additive", 96, 96);
    let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
    let canvas = engine.canvas_2d();
    canvas.set_blend_mode(BlendMode::Additive);
    canvas.fill_ellipse(Rect::new(8.0, 8.0, 64.0, 64.0), Color::blue());
    assert!(matches!(
        engine.end_frame(&DamageRegion::full()),
        crate::draw::RenderOutcome::Failed(GraphicsFailure::Other(error))
            if error.code() == crate::core::Errc::NotImplemented
    ));
    engine.shutdown();
    window.close().expect("close native window");
}

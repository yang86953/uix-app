#![cfg(feature = "opengles")]

use crate::draw::backend::RenderBackend;
use crate::draw::engine::GraphicsFailure;
use crate::draw::gpu_engine::GpuEngine;
use crate::draw::pipeline::{
    EncodedFrameExecution, EncodedPictureExecution, FrameCommand, FrameRasterOp,
};
use crate::draw::{BlendMode, GraphicsEngine, Radius, UpdateStrategy};
use crate::tests::common::*;

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
    let context = crate::native::factory::create_gpu_context_with_backend(
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
    engine.try_shutdown().expect("checked shutdown");
    window.close().expect("close native window");
}

#[test]
fn opengles_encoded_glyph_ir_uses_native_atlas_without_soft_upload() {
    if std::env::consts::OS != "windows" {
        return;
    }

    let (_platform, mut window, mut engine) = open_engine("native GL glyph IR", 128, 64);
    let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
    let (width, height) = {
        let size = engine
            .session_mut()
            .native_gpu_backend_mut()
            .expect("OpenGL ES NativeGpu backend")
            .surface()
            .size();
        (size.w as i32, size.h as i32)
    };
    let mut encoder = FrameEncoder::new(width, height).expect("glyph encoder");
    encoder.clear(Color::from_rgb(12, 24, 48));
    let coverage: std::sync::Arc<[u8]> = vec![0, 64, 128, 255, 255, 128, 64, 0].into();
    let glyph = crate::draw::pipeline::FrameGlyphBlit::new(
        8,
        6,
        coverage,
        4,
        2,
        Color::from_rgba(220, 96, 40, 160),
    )
    .expect("validated glyph");
    encoder.native(FrameRasterOp::BlitGlyphs {
        glyphs: vec![glyph],
        clip: FrameRect::new(9, 6, 3, 2),
    });
    let reference = encoder.render_reference();

    let backend = engine
        .session_mut()
        .native_gpu_backend_mut()
        .expect("OpenGL ES NativeGpu backend");
    assert_eq!(
        backend
            .try_execute_encoded_frame(&encoder)
            .expect("execute native glyph IR"),
        EncodedFrameExecution::Executed
    );
    assert_eq!(
        backend.last_soft_upload_bytes(),
        0,
        "native glyph IR must not upload a BGRA fallback tile"
    );
    let pixels = backend.try_readback().expect("glyph IR readback");
    let stride = width as usize;
    for (x, y) in [(8usize, 6usize), (9, 6), (10, 6), (11, 6), (12, 6), (10, 7)] {
        let expected = reference.pixel(x as i32, y as i32).unwrap();
        let observed = wgl_rgba_to_aarrggbb(pixels[(height as usize - 1 - y) * stride + x]);
        for shift in [24, 16, 8, 0] {
            let expected_channel = ((expected >> shift) & 0xFF) as i16;
            let observed_channel = ((observed >> shift) & 0xFF) as i16;
            assert!(
                (expected_channel - observed_channel).abs() <= 1,
                "probe ({x},{y}), channel {shift}: expected {expected:#010X}, got {observed:#010X}"
            );
        }
    }

    engine.try_shutdown().expect("checked shutdown");
    window.close().expect("close native window");
}

#[test]
fn opengles_picture_producer_glyph_ir_matches_reference_without_soft_upload() {
    use crate::draw::pipeline::frame_recording::FrameRecordingEngine;

    if std::env::consts::OS != "windows" {
        return;
    }
    let (_platform, mut window, mut engine) = open_engine("Picture glyph IR", 96, 48);
    let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
    let (width, height) = {
        let size = engine
            .session_mut()
            .native_gpu_backend_mut()
            .expect("OpenGL ES NativeGpu backend")
            .surface()
            .size();
        (size.w as i32, size.h as i32)
    };

    let mut recorder = FrameRecordingEngine::new();
    recorder
        .initialize(width, height)
        .expect("initialize Picture recorder");
    let picture = recorder.create_offscreen(32, 16).expect("Picture");
    let coverage: std::sync::Arc<[u8]> = vec![0, 1, 127, 255, 255, 128, 64, 0].into();
    recorder
        .try_begin_offscreen_paint(&picture)
        .expect("begin Picture");
    {
        let canvas = recorder.offscreen_canvas(&picture).expect("Picture canvas");
        canvas.translate(-8.0, -4.0);
        canvas.fill_rect(
            Rect::new(8.0, 4.0, 32.0, 16.0),
            Color::from_rgb(24, 48, 72),
            Some(Radius::uniform(4.0)),
        );
        canvas.fill_rect(
            Rect::new(20.0, 4.0, 4.0, 4.0),
            Color::from_rgb(72, 48, 24),
            None,
        );
        canvas.fill_rect(
            Rect::new(24.0, 4.0, 4.0, 4.0),
            Color::from_rgb(24, 72, 48),
            None,
        );
        canvas.set_opacity(0.37);
        canvas.blit_glyph_shared(
            21,
            5,
            std::sync::Arc::clone(&coverage),
            4,
            2,
            Color::from_rgba(220, 96, 40, 160),
        );
        canvas.blit_glyph_shared(
            23,
            5,
            std::sync::Arc::clone(&coverage),
            4,
            2,
            Color::from_rgba(40, 180, 220, 192),
        );
    }
    recorder.try_end_offscreen_paint().expect("commit Picture");
    recorder.begin_recording(true).expect("begin main frame");
    recorder.canvas_2d().fill_rect(
        Rect::new(0.0, 0.0, width as f32, height as f32),
        Color::from_rgb(12, 24, 48),
        None,
    );
    recorder
        .try_blit_offscreen_src(
            &picture,
            Rect::new(0.0, 0.0, 32.0, 16.0),
            Rect::new(8.0, 4.0, 32.0, 16.0),
        )
        .expect("splice Picture glyph IR");
    let encoder = recorder.finish_recording().expect("finish main frame");
    assert!(!encoder.commands().iter().any(|command| matches!(
        command,
        FrameCommand::CpuSegment { .. } | FrameCommand::PictureBlit { .. }
    )));
    let reference = encoder.render_reference();

    let backend = engine
        .session_mut()
        .native_gpu_backend_mut()
        .expect("OpenGL ES NativeGpu backend");
    assert_eq!(
        backend
            .try_execute_encoded_frame(&encoder)
            .expect("execute Picture glyph IR"),
        EncodedFrameExecution::Executed
    );
    assert_eq!(backend.last_soft_upload_bytes(), 0);
    let pixels = backend.try_readback().expect("Picture glyph readback");
    let stride = width as usize;
    for (x, y) in [
        (9usize, 6usize),
        (10, 6),
        (11, 6),
        (21, 5),
        (23, 5),
        (24, 6),
        (26, 6),
        (40, 20),
    ] {
        let expected = reference.pixel(x as i32, y as i32).unwrap();
        let observed = wgl_rgba_to_aarrggbb(pixels[(height as usize - 1 - y) * stride + x]);
        for shift in [24, 16, 8, 0] {
            let expected_channel = ((expected >> shift) & 0xff) as i16;
            let observed_channel = ((observed >> shift) & 0xff) as i16;
            assert!(
                (expected_channel - observed_channel).abs() <= 1,
                "probe ({x},{y}), channel {shift}: expected {expected:#010X}, got {observed:#010X}"
            );
        }
    }

    engine.try_shutdown().expect("checked shutdown");
    window.close().expect("close native window");
}

#[test]
fn opengles_encoded_fill_opacity_ir_matches_reference_without_soft_upload() {
    use crate::draw::pipeline::frame_recording::FrameRecordingEngine;

    if std::env::consts::OS != "windows" {
        return;
    }

    let (_platform, mut window, mut engine) = open_engine("native GL fill opacity IR", 64, 40);
    let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
    let (width, height) = {
        let size = engine
            .session_mut()
            .native_gpu_backend_mut()
            .expect("OpenGL ES NativeGpu backend")
            .surface()
            .size();
        (size.w as i32, size.h as i32)
    };
    let mut recorder = FrameRecordingEngine::new();
    recorder
        .initialize(width, height)
        .expect("initialize opacity recorder");
    recorder
        .begin_recording(true)
        .expect("begin opacity recording");
    recorder.canvas_2d().fill_rect(
        Rect::new(0.0, 0.0, width as f32, height as f32),
        Color::from_rgb(24, 48, 72),
        None,
    );
    recorder.canvas_2d().set_opacity(0.37);
    recorder.canvas_2d().fill_rect(
        Rect::new(4.0, 4.0, 24.0, 12.0),
        Color::from_rgba(220, 80, 40, 160),
        None,
    );
    recorder
        .canvas_2d()
        .push_clip(Rect::new(18.0, 10.0, 20.0, 16.0));
    recorder.canvas_2d().fill_rect(
        Rect::new(16.0, 8.0, 28.0, 22.0),
        Color::from_rgba(40, 180, 220, 192),
        Some(Radius {
            tl: 7.0,
            tr: 3.0,
            br: 18.0,
            bl: 0.0,
        }),
    );
    let encoder = recorder
        .finish_recording()
        .expect("finish opacity recording");
    assert_eq!(recorder.scratch_surface_size(), (1, 1));
    assert!(!encoder
        .commands()
        .iter()
        .any(|command| matches!(command, FrameCommand::CpuSegment { .. })));
    let reference = encoder.render_reference();

    let backend = engine
        .session_mut()
        .native_gpu_backend_mut()
        .expect("OpenGL ES NativeGpu backend");
    assert_eq!(
        backend
            .try_execute_encoded_frame(&encoder)
            .expect("execute fill opacity IR"),
        EncodedFrameExecution::Executed
    );
    assert_eq!(
        backend.last_soft_upload_bytes(),
        0,
        "eligible opacity fills must not upload a BGRA fallback tile"
    );
    let pixels = backend.try_readback().expect("fill opacity IR readback");
    let stride = width as usize;
    for (x, y) in [
        (2usize, 2usize),
        (6, 6),
        (17, 9),
        (18, 10),
        (24, 14),
        (37, 24),
        (38, 24),
    ] {
        let expected = reference.pixel(x as i32, y as i32).unwrap();
        let observed = wgl_rgba_to_aarrggbb(pixels[(height as usize - 1 - y) * stride + x]);
        // These two probes stack the rounded fill over the opacity sharp fill,
        // so two independently UNORM-quantized blends may each contribute 1.
        let tolerance = if matches!((x, y), (18, 10) | (24, 14)) {
            2
        } else {
            1
        };
        for shift in [24, 16, 8, 0] {
            let expected_channel = ((expected >> shift) & 0xff) as i16;
            let observed_channel = ((observed >> shift) & 0xff) as i16;
            assert!(
                (expected_channel - observed_channel).abs() <= tolerance,
                "probe ({x},{y}), channel {shift}, tolerance {tolerance}: expected {expected:#010X}, got {observed:#010X}"
            );
        }
    }

    engine.try_shutdown().expect("checked shutdown");
    window.close().expect("close native window");
}

fn record_half_blue_soft_tile(canvas: &mut dyn crate::draw::traits::Canvas2D) {
    canvas.fill_rect(
        Rect::new(0.0, 0.0, 96.0, 96.0),
        Color::from_rgba(0, 255, 0, 255),
        None,
    );
    canvas.push_clip(Rect::new(32.0, 32.0, 32.0, 32.0));
    canvas.fill_ellipse(
        Rect::new(24.0, 24.0, 48.0, 48.0),
        Color::from_rgba(0, 0, 255, 128),
    );
    canvas.pop_clip();
}

fn record_common_hybrid_clip_scene(canvas: &mut dyn crate::draw::traits::Canvas2D) {
    canvas.fill_rect(
        Rect::new(0.0, 0.0, 96.0, 64.0),
        Color::from_rgba(0, 0, 0, 255),
        None,
    );
    canvas.fill_rect(
        Rect::new(16.0, 12.0, 48.0, 32.0),
        Color::from_rgba(255, 0, 0, 255),
        None,
    );
    canvas.push_clip(Rect::new(28.0, 18.0, 24.0, 20.0));
    canvas.fill_ellipse(
        Rect::new(20.0, 10.0, 40.0, 40.0),
        Color::from_rgba(0, 0, 255, 128),
    );
    canvas.pop_clip();
}

fn wgl_rgba_to_aarrggbb(pixel: u32) -> u32 {
    (pixel & 0xFF00_0000)
        | ((pixel & 0x0000_00FF) << 16)
        | (pixel & 0x0000_FF00)
        | ((pixel & 0x00FF_0000) >> 16)
}

#[test]
fn opengles_native_path_matches_software_for_hybrid_clip_and_bounded_tile() {
    if std::env::consts::OS != "windows" {
        return;
    }

    let mut software = SoftwareEngine::new();
    software.initialize(96, 64).expect("initialize software");
    let _ = software.begin_frame(UpdateStrategy::FullRedraw);
    record_common_hybrid_clip_scene(software.canvas_2d());
    let reference = software
        .session()
        .cpu_backend()
        .expect("software CPU backend")
        .pixels()
        .to_vec();

    let (_platform, mut window, mut engine) = open_engine("native GL hybrid parity", 96, 64);
    let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
    record_common_hybrid_clip_scene(engine.canvas_2d());
    let (pixels, stride, upload_bytes) = {
        let backend = engine
            .session_mut()
            .native_gpu_backend_mut()
            .expect("OpenGL ES NativeGpu backend");
        let stride = backend.surface().size().w as usize;
        let pixels = backend.try_readback().expect("read back hybrid clip frame");
        (pixels, stride, backend.last_soft_upload_bytes())
    };

    for (x, y) in [(4usize, 4usize), (20, 16), (22, 12), (40, 28), (85, 55)] {
        let expected = reference[y * 96 + x];
        let actual = wgl_rgba_to_aarrggbb(pixels[(64 - 1 - y) * stride + x]);
        for shift in [24, 16, 8, 0] {
            let expected_channel = ((expected >> shift) & 0xFF) as i16;
            let actual_channel = ((actual >> shift) & 0xFF) as i16;
            assert!(
                (expected_channel - actual_channel).abs() <= 1,
                "probe ({x}, {y}), channel {shift}: expected {expected:#010X}, got {actual:#010X}"
            );
        }
    }
    assert!(
        upload_bytes > 0 && upload_bytes < 96 * 64 * 4,
        "bounded WGL soft fallback must not upload a full frame"
    );

    assert!(matches!(
        engine.end_frame(&DamageRegion::full()),
        crate::draw::RenderOutcome::Present(_)
    ));
    engine.try_shutdown().expect("checked shutdown");
    window.close().expect("close native window");
}

#[test]
fn opengles_native_path_matches_software_for_premultiplied_soft_tile() {
    if std::env::consts::OS != "windows" {
        return;
    }

    let mut software = SoftwareEngine::new();
    software.initialize(96, 96).expect("initialize software");
    let _ = software.begin_frame(UpdateStrategy::FullRedraw);
    record_half_blue_soft_tile(software.canvas_2d());
    let reference = software
        .session()
        .cpu_backend()
        .expect("software CPU backend")
        .pixels()[48 * 96 + 48];

    let (_platform, mut window, mut engine) = open_engine("native GL premultiplied tile", 96, 96);
    let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
    record_half_blue_soft_tile(engine.canvas_2d());
    let (pixels, stride) = {
        let backend = engine
            .session_mut()
            .native_gpu_backend_mut()
            .expect("OpenGL ES NativeGpu backend");
        let stride = backend.surface().size().w as usize;
        let pixels = backend
            .try_readback()
            .expect("read back premultiplied soft tile");
        (pixels, stride)
    };
    assert_eq!(pixels[8 * stride + 8], 0xFF00_FF00, "native base rect");
    let rgba_to_aarrggbb = |pixel: u32| {
        (pixel & 0xFF00_0000)
            | ((pixel & 0x0000_00FF) << 16)
            | (pixel & 0x0000_FF00)
            | ((pixel & 0x00FF_0000) >> 16)
    };
    assert_eq!(rgba_to_aarrggbb(pixels[48 * stride + 48]), reference);

    engine.try_shutdown().expect("checked shutdown");
    window.close().expect("close native window");
}

#[test]
fn opengles_native_path_executes_encoded_cached_picture_in_bound_offscreen() {
    if std::env::consts::OS != "windows" {
        return;
    }

    let (_platform, mut window, mut engine) = open_engine("native GL encoded Picture", 128, 128);
    let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
    let picture = engine.create_offscreen(32, 32).expect("offscreen picture");
    engine
        .try_begin_offscreen_paint(&picture)
        .expect("bind Picture target");

    let mut encoder = FrameEncoder::new(32, 32).expect("FrameEncoder");
    encoder.clear(Color::transparent());
    encoder
        .cpu_segment([FrameRasterOp::FillRect {
            rect: FrameRect::new(8, 8, 16, 16),
            color: Color::blue(),
        }])
        .unwrap();
    assert_eq!(
        engine
            .try_execute_encoded_picture(&picture, &encoder)
            .expect("execute encoded Picture"),
        EncodedPictureExecution::Executed,
        "GpuNative must execute the API-neutral FrameEncoder while its Picture target is bound"
    );
    engine
        .try_flush_offscreen_paint(&picture)
        .expect("flush encoded Picture");
    engine
        .try_end_offscreen_paint()
        .expect("restore swapchain after encoded Picture");
    engine.blit_offscreen(&picture, Rect::new(32.0, 32.0, 64.0, 64.0));

    let (pixels, stride) = {
        let backend = engine
            .session_mut()
            .native_gpu_backend_mut()
            .expect("OpenGL ES NativeGpu backend");
        let stride = backend.surface().size().w as usize;
        let pixels = backend
            .try_readback()
            .expect("read back encoded Picture frame");
        (pixels, stride)
    };
    assert_eq!(
        pixels[64 * stride + 64],
        0xFFFF_0000,
        "FrameEncoder Picture must reach the native target in painter order"
    );

    assert!(matches!(
        engine.end_frame(&DamageRegion::full()),
        crate::draw::RenderOutcome::Present(_)
    ));
    engine.destroy_offscreen(picture);
    engine.try_shutdown().expect("checked shutdown");
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
    engine.try_shutdown().expect("checked shutdown");
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
    engine.try_shutdown().expect("checked shutdown");
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
    engine.try_shutdown().expect("checked shutdown");
    window.close().expect("close native window");
}

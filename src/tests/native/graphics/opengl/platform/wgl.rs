use crate::native::graphics::opengl::platform::wgl::*;
use crate::native::graphics::platform::windows::drawable_size_from_dpi;
use crate::tests::common::*;
use std::ffi::CString;
use std::ptr;

#[test]
fn drawable_size_scales_logical_client_by_monitor_dpi() {
    let drawable = drawable_size_from_dpi(1200, 800, 192);
    assert_eq!((drawable.width, drawable.height), (2400, 1600));
    assert!((drawable.width as f32 / drawable.logical_width as f32 - 2.0).abs() < f32::EPSILON);
}

#[test]
fn wgl_context_rejects_null_hwnd() {
    match WglContext::new(ptr::null_mut(), 800, 600) {
        Err(err) => assert_eq!(err.code(), Errc::PlatformError),
        Ok(_) => panic!("expected null hwnd to fail"),
    }
}

#[test]
fn core_gl_entry_point_falls_back_to_opengl32_export() {
    let name = CString::new("glGetString").expect("valid GL symbol");
    assert!(!load_gl_proc(&name).is_null());
}

/// 对齐 D3D11 `factory_create_d3d11_gpu_native_swapchain_on_real_window`：
/// 真实 HWND + caps + SwapBuffers present + 多帧稳定。
#[cfg(feature = "opengles")]
#[test]
fn factory_create_wgl_gpu_native_swapchain_on_real_window() {
    if std::env::consts::OS != "windows" {
        return;
    }

    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("WGL GPU native test", 320, 240)
        .expect("window");
    let surface = window.native_surface_ptr();
    assert!(
        !surface.is_null(),
        "Windows HWND must be exposed as native_surface_ptr"
    );

    let mut ctx = crate::native::factory::create_gpu_context_with_backend(
        surface,
        320,
        240,
        GraphicsBackend::OpenGlEs,
    )
    .expect("WglContext via factory");
    assert_eq!(ctx.graphics_backend(), GraphicsBackend::OpenGlEs);
    let caps = ctx.caps();
    assert_eq!(caps.raster, RasterMode::GpuNative);
    assert_eq!(caps.present, PresentMode::Swapchain);
    assert!(!ctx.supports_pixel_present());
    assert!(ctx.supports_gl_proc_address());
    ctx.present(&PresentFrame::Swapchain {
        damage: PresentDamage::Full,
    })
    .expect("first swapchain present");

    for _ in 0..16 {
        ctx.present(&PresentFrame::Swapchain {
            damage: PresentDamage::Full,
        })
        .expect("repeated swapchain present");
    }

    ctx.try_shutdown().expect("shutdown");
    window.close().expect("close native window");
}

#[cfg(feature = "opengles")]
#[test]
fn opengles_factory_uses_the_complete_shared_wgpu_raster_baseline() {
    if std::env::consts::OS != "windows" {
        return;
    }

    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("WGL native raster caps", 320, 240)
        .expect("window");
    let mut context = crate::native::factory::create_gpu_context_with_backend(
        window.native_surface_ptr(),
        320,
        240,
        GraphicsBackend::OpenGlEs,
    )
    .expect("WglContext via factory");

    let caps = context.native_raster_caps();
    assert_eq!(caps, NativeRasterCaps::wgpu_full());
    assert!(caps.has_gpu_only_baseline());
    context.try_shutdown().expect("WGL checked shutdown");
    window.close().expect("close native window");
}

#[cfg(feature = "opengles")]
#[test]
fn wgl_glyph_atlas_reuses_coverage_and_matches_cpu_premultiplied_blend() {
    if std::env::consts::OS != "windows" {
        return;
    }

    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("WGL glyph atlas parity", 128, 64)
        .expect("window");
    let mut context = WglContext::new(window.native_surface_ptr(), 128, 64).expect("WGL context");
    let width = context.width();
    let height = context.height();
    let base = Color::from_rgb(18, 36, 72);
    let color = Color::from_rgba(220, 96, 40, 160);
    context
        .clear_render_target(
            base.r as f32 / 255.0,
            base.g as f32 / 255.0,
            base.b as f32 / 255.0,
            1.0,
        )
        .expect("clear glyph target");

    let coverage: std::sync::Arc<[u8]> =
        vec![0, 1, 64, 127, 128, 254, 255, 255, 254, 128, 127, 64, 1, 0].into();
    let glyph = |x| crate::native::traits::present::GpuGlyphBlit {
        x: x as f32,
        y: 3.0,
        w: 7.0,
        h: 2.0,
        corners: crate::native::traits::present::GpuGlyphBlit::axis_aligned_corners(
            x as f32, 3.0, 7.0, 2.0,
        ),
        rgba: [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            color.a as f32 / 255.0,
        ],
        coverage: std::sync::Arc::clone(&coverage),
        cov_w: 7,
        cov_h: 2,
        outline_mesh: None,
    };
    let scissor = Some((4, 3, 16, 2));
    context
        .draw_glyphs(width as f32, height as f32, scissor, &[glyph(2)])
        .expect("first glyph draw");
    assert_eq!(context.glyph_atlas_upload_count(), 1);
    context
        .draw_glyphs(width as f32, height as f32, scissor, &[glyph(13)])
        .expect("reused glyph draw");
    assert_eq!(
        context.glyph_atlas_upload_count(),
        1,
        "the same retained coverage allocation must not be uploaded twice"
    );
    assert!(
        !context.has_soft_texture(),
        "glyph-only drawing must not allocate the full-target RGBA soft texture"
    );

    let actual = context
        .read_pixels(0, 0, width, height)
        .expect("glyph readback");
    let mut expected = vec![base.premultiplied(); (width * height) as usize];
    for x in [2, 13] {
        crate::draw::rasterizer::glyph::blit_glyph(
            &mut expected,
            width,
            height,
            Rect::new(4.0, 3.0, 16.0, 2.0),
            1.0,
            x,
            3,
            coverage.as_ref(),
            7,
            2,
            color,
        );
    }
    for y in 3..5usize {
        for x in 0..22usize {
            let expected_pixel = expected[y * width as usize + x];
            let raw = actual[(height as usize - 1 - y) * width as usize + x];
            let observed = (raw & 0xFF00_0000)
                | ((raw & 0x0000_00FF) << 16)
                | (raw & 0x0000_FF00)
                | ((raw & 0x00FF_0000) >> 16);
            for shift in [24, 16, 8, 0] {
                let expected_channel = ((expected_pixel >> shift) & 0xFF) as i16;
                let observed_channel = ((observed >> shift) & 0xFF) as i16;
                assert!(
                    (expected_channel - observed_channel).abs() <= 1,
                    "({x},{y}), channel={shift}: expected {expected_pixel:#010X}, got {observed:#010X}"
                );
            }
        }
    }

    context.try_shutdown().expect("WGL checked shutdown");
    window.close().expect("close native window");
}

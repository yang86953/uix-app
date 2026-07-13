use super::*;
use crate::native::graphics::platform::windows::drawable_size_from_dpi;
use crate::native::traits::present::{
    GraphicsBackend, PresentDamage, PresentFrame, PresentMode, RasterMode,
};

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
fn wgl_exposes_only_the_native_raster_operations_it_implements() {
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
    assert!(caps.clear_target);
    assert!(caps.clear_rects);
    assert!(caps.soft_blit);
    assert!(caps.solid_rects);
    assert!(caps.offscreen_targets);
    assert!(!caps.stroke_rects);
    assert!(!caps.glyphs);
    assert!(!caps.linear_gradients);
    assert!(!caps.radial_gradients);
    assert!(!caps.solid_meshes);
    assert!(!caps.box_shadows);

    assert!(caps.has_hybrid_baseline());
    context.try_shutdown().expect("WGL checked shutdown");
    window.close().expect("close native window");
}

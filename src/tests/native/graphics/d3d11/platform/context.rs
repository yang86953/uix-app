use crate::tests::common::*;
use std::ffi::c_void;
use crate::core::{ Result };
use crate::native::graphics::platform::windows as win_surface;
use crate::native::traits::present::{ GpuBoxShadow, GpuGlyphBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSolidMesh, GpuSolidRect, GpuStrokeRect, OffscreenTargetId };
use ::windows::Win32::Foundation::{E_OUTOFMEMORY, HMODULE, HWND, TRUE};
use ::windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL,
    D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use ::windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING, D3D11_VIEWPORT,
    D3D11CreateDeviceAndSwapChain, ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView,
    ID3D11ShaderResourceView, ID3D11Texture2D,
};
use ::windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_DESC, DXGI_MODE_SCALING_UNSPECIFIED,
    DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED, DXGI_RATIONAL, DXGI_SAMPLE_DESC,
};
use ::windows::Win32::Graphics::Dxgi::{
    DXGI_ERROR_DEVICE_HUNG, DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET,
    DXGI_ERROR_DRIVER_INTERNAL_ERROR, DXGI_ERROR_REMOTE_OUTOFMEMORY, DXGI_PRESENT,
    DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_DISCARD,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGISwapChain,
};
use ::windows::core::Interface;
use crate::native::graphics::d3d11::platform::context::*;

#[test]
fn swap_chain_desc_uses_bgra_windowed_backbuffer() {
    let desc = swap_chain_desc(std::ptr::dangling_mut::<c_void>(), 800, 600);

    assert_eq!(desc.BufferDesc.Width, 800);
    assert_eq!(desc.BufferDesc.Height, 600);
    assert_eq!(desc.BufferDesc.Format, DXGI_FORMAT_B8G8R8A8_UNORM);
    assert_eq!(desc.SampleDesc.Count, 1);
    assert_eq!(desc.BufferCount, 2);
    assert_eq!(desc.BufferUsage, DXGI_USAGE_RENDER_TARGET_OUTPUT);
    assert_eq!(desc.Windowed, TRUE);
}

#[test]
fn d3d11_rejects_null_hwnd() {
    let err = match D3d11Context::new(std::ptr::null_mut(), 640, 480) {
        Ok(_) => panic!("expected null hwnd to fail"),
        Err(err) => err,
    };
    assert_eq!(err.code(), Errc::PlatformError);
}

#[test]
fn dxgi_present_device_removed_is_a_typed_device_failure() {
    let error = map_dxgi_present_result(::windows::core::HRESULT(0x887A_0005u32 as i32))
        .expect_err("DXGI present failure must propagate");

    assert_eq!(error.code(), Errc::GraphicsDeviceLost);
    assert!(error.what().contains("IDXGISwapChain::Present"));
}

#[test]
fn dxgi_present_out_of_memory_is_typed() {
    let error = map_dxgi_present_result(::windows::core::HRESULT(0x8007_000Eu32 as i32))
        .expect_err("DXGI out-of-memory must propagate");

    assert_eq!(error.code(), Errc::GraphicsOutOfMemory);
}

#[test]
fn factory_create_d3d11_gpu_native_swapchain_on_real_window() {
    let mut platform = crate::native::create_platform().expect("platform");
    let window = platform
        .window_manager()
        .create_window("D3D11 GPU native test", 320, 240)
        .expect("window");
    let surface = window.native_surface_ptr();
    assert!(
        !surface.is_null(),
        "Windows HWND must be exposed as native_surface_ptr"
    );

    let mut ctx = D3d11Context::new(surface, 320, 240).expect("D3d11Context");
    assert_eq!(ctx.graphics_backend(), GraphicsBackend::D3d11);
    let adapter_info = &ctx.adapter_info;
    assert_ne!(adapter_info.description, "unavailable");
    assert!(!adapter_info.description.trim().is_empty());
    assert_ne!(adapter_info.vendor_id, 0);
    assert_ne!(adapter_info.device_id, 0);
    println!(
        "D3D11 real-window adapter: {}",
        adapter_info.diagnostic_summary()
    );
    let caps = ctx.caps();
    assert_eq!(caps.raster, RasterMode::GpuNative);
    assert_eq!(caps.present, PresentMode::Swapchain);
    assert!(!ctx.supports_pixel_present());
    assert!(!ctx.supports_gl_proc_address());

    assert_eq!(ctx.native_raster_caps(), NativeRasterCaps::d3d11_full());
    ctx.clear_render_target(0.1, 0.2, 0.3, 1.0)
        .expect("clear_render_target");
    ctx.draw_solid_rects(
        ctx.width() as f32,
        ctx.height() as f32,
        None,
        &[GpuSolidRect {
            x: 16.0,
            y: 24.0,
            w: 80.0,
            h: 40.0,
            rgba: [1.0, 0.2, 0.1, 1.0],
            radius: [8.0, 8.0, 8.0, 8.0],
        }],
    )
    .expect("draw_solid_rects");
    ctx.draw_stroke_rects(
        ctx.width() as f32,
        ctx.height() as f32,
        None,
        &[
            GpuStrokeRect {
                x: 20.0,
                y: 80.0,
                w: 100.0,
                h: 48.0,
                rgba: [0.1, 0.8, 1.0, 1.0],
                radius: [6.0, 6.0, 6.0, 6.0],
                line_width: 2.0,
            },
            GpuStrokeRect {
                x: 200.0,
                y: 40.0,
                w: 64.0,
                h: 64.0,
                rgba: [1.0, 1.0, 0.2, 1.0],
                radius: [32.0, 32.0, 32.0, 32.0],
                line_width: 3.0,
            },
        ],
    )
    .expect("draw_stroke_rects");
    // Glyph atlas: solid-color coverage blit (identity text path).
    let cov = std::sync::Arc::<[u8]>::from(vec![255u8; 8 * 8]);
    ctx.draw_glyphs(
        ctx.width() as f32,
        ctx.height() as f32,
        None,
        &[GpuGlyphBlit {
            x: 40.0,
            y: 160.0,
            w: 8.0,
            h: 8.0,
            rgba: [1.0, 1.0, 1.0, 1.0],
            coverage: cov,
            cov_w: 8,
            cov_h: 8,
        }],
    )
    .expect("draw_glyphs");
    ctx.draw_linear_gradients(
        ctx.width() as f32,
        ctx.height() as f32,
        None,
        &[GpuLinearGradientRect {
            x: 120.0,
            y: 16.0,
            w: 80.0,
            h: 24.0,
            color_a: [1.0, 0.0, 0.0, 1.0],
            color_b: [0.0, 0.0, 1.0, 1.0],
            dir: 0,
        }],
    )
    .expect("draw_linear_gradients");
    ctx.draw_radial_gradients(
        ctx.width() as f32,
        ctx.height() as f32,
        None,
        &[GpuRadialGradient {
            cx: 260.0,
            cy: 180.0,
            inner_r: 4.0,
            outer_r: 28.0,
            color_inner: [1.0, 1.0, 0.0, 1.0],
            color_outer: [0.0, 0.5, 0.0, 0.0],
        }],
    )
    .expect("draw_radial_gradients");
    // Simple triangle mesh (CPU-tessellated path fill).
    ctx.draw_solid_meshes(
        ctx.width() as f32,
        ctx.height() as f32,
        None,
        &[GpuSolidMesh {
            vertices: std::sync::Arc::<[f32]>::from(vec![
                180.0, 80.0, 220.0, 80.0, 200.0, 120.0,
            ]),
            rgba: [0.2, 1.0, 0.4, 1.0],
        }],
    )
    .expect("draw_solid_meshes");
    ctx.draw_box_shadows(
        ctx.width() as f32,
        ctx.height() as f32,
        None,
        &[
            GpuBoxShadow {
                x: 40.0,
                y: 100.0,
                w: 64.0,
                h: 32.0,
                offset_x: 4.0,
                offset_y: 6.0,
                blur: 8.0,
                rgba: [0.0, 0.0, 0.0, 0.45],
                radius: [6.0, 6.0, 6.0, 6.0],
                ambient: false,
            },
            GpuBoxShadow {
                x: 200.0,
                y: 100.0,
                w: 48.0,
                h: 48.0,
                offset_x: 0.0,
                offset_y: 0.0,
                blur: 12.0,
                rgba: [0.0, 0.0, 0.0, 0.3],
                radius: [24.0, 24.0, 24.0, 24.0],
                ambient: true,
            },
        ],
    )
    .expect("draw_box_shadows");
    // Soft overlay (transparent except one opaque pixel region via alpha).
    let mut soft = vec![0u32; (ctx.width() * ctx.height()) as usize];
    soft[0] = 0xFF00_FF00; // opaque green BGRA
    ctx.blit_soft_fallback(&soft, ctx.width(), ctx.height())
        .expect("blit_soft_fallback");
    let pixels = ctx
        .read_pixels(0, 0, ctx.width(), ctx.height())
        .expect("hardware D3D11 readback");
    assert_eq!(pixels.len(), (ctx.width() * ctx.height()) as usize);
    assert_eq!(pixels[0], 0xFF00_FF00);
    assert!(pixels.iter().any(|pixel| *pixel != pixels[0]));
    ctx.present(&PresentFrame::Swapchain {
        damage: PresentDamage::Full,
    })
    .expect("swapchain present");
    ctx.try_shutdown().expect("shutdown");
    drop(ctx);

    let mut warp_ctx = create_with_driver(
        surface,
        320,
        240,
        &D3D11_FEATURE_LEVELS,
        D3d11DriverKind::Warp,
    )
    .expect("D3D11 WARP context");
    assert_eq!(warp_ctx.adapter_info.driver, D3d11DriverKind::Warp);
    assert_ne!(warp_ctx.adapter_info.description, "unavailable");
    println!(
        "D3D11 WARP real-window adapter: {}",
        warp_ctx.adapter_info.diagnostic_summary()
    );

    warp_ctx
        .clear_render_target(0.25, 0.5, 0.75, 1.0)
        .expect("clear WARP render target");
    let pixels = warp_ctx
        .read_pixels(0, 0, warp_ctx.width(), warp_ctx.height())
        .expect("WARP D3D11 readback");
    assert_eq!(
        pixels.len(),
        (warp_ctx.width() * warp_ctx.height()) as usize
    );
    assert_eq!(pixels[0] >> 24, 0xFF);
    assert_ne!(pixels[0] & 0x00FF_FFFF, 0);
    warp_ctx
        .present(&PresentFrame::Swapchain {
            damage: PresentDamage::Full,
        })
        .expect("WARP swapchain present");
    warp_ctx.try_shutdown().expect("shutdown");
}

#[test]
fn d3d11_soft_blit_ignores_a_previous_native_scissor() {
    let mut platform = crate::native::create_platform().expect("platform");
    let window = platform
        .window_manager()
        .create_window("D3D11 soft scissor", 96, 64)
        .expect("window");
    let mut ctx = D3d11Context::new(window.native_surface_ptr(), 96, 64).expect("context");
    ctx.clear_render_target(0.0, 0.0, 0.0, 1.0)
        .expect("clear target");
    ctx.draw_solid_rects(
        96.0,
        64.0,
        Some((24, 16, 32, 24)),
        &[GpuSolidRect {
            x: 24.0,
            y: 16.0,
            w: 32.0,
            h: 24.0,
            rgba: [1.0, 1.0, 1.0, 1.0],
            radius: [0.0; 4],
        }],
    )
    .expect("draw clipped native rect");

    let mut soft = vec![0u32; 96 * 64];
    soft[2 * 96 + 2] = 0xFF00_FF00; // BGRA opaque green, outside the native scissor.
    ctx.blit_soft_fallback(&soft, 96, 64)
        .expect("blit full soft surface");

    assert_eq!(
        ctx.read_pixels(2, 2, 1, 1).expect("soft-scissor readback"),
        vec![0xFF00_FF00],
        "soft fallback must not inherit the previous native draw scissor"
    );
    ctx.try_shutdown().expect("shutdown");
}

#[test]
fn d3d11_soft_blit_does_not_resample_a_prior_partial_segment() {
    let mut platform = crate::native::create_platform().expect("platform");
    let window = platform
        .window_manager()
        .create_window("D3D11 WARP soft damage", 64, 48)
        .expect("window");
    let mut ctx = create_with_driver(
        window.native_surface_ptr(),
        64,
        48,
        &D3D11_FEATURE_LEVELS,
        D3d11DriverKind::Warp,
    )
    .expect("D3D11 WARP context");
    ctx.clear_render_target(0.0, 0.0, 0.0, 0.0)
        .expect("clear transparent target");

    ctx.blit_soft_fallback_tile(&[0x80FF_0000], SoftFallbackTile::at_destination(4, 5, 1, 1))
        .expect("first compact soft segment");
    let first_before = ctx
        .read_pixels(4, 5, 1, 1)
        .expect("first soft segment readback")[0];
    assert_ne!(first_before, 0, "first segment must reach the target");

    ctx.blit_soft_fallback_tile(
        &[0x8000_00FF],
        SoftFallbackTile::at_destination(36, 19, 1, 1),
    )
    .expect("second compact soft segment");

    assert_eq!(
        ctx.read_pixels(4, 5, 1, 1)
            .expect("first soft segment readback")[0],
        first_before,
        "a later partial upload must not re-blend stale texture data"
    );
    assert_ne!(
        ctx.read_pixels(36, 19, 1, 1)
            .expect("second soft segment readback")[0],
        0,
        "second segment must reach its own target pixel"
    );
    ctx.try_shutdown().expect("shutdown");
}

#[test]
fn d3d11_resize_grows_backbuffer_and_fills_far_corner() {
    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("D3D11 resize grow", 320, 240)
        .expect("window");
    let surface = window.native_surface_ptr();
    let mut ctx = D3d11Context::new(surface, 320, 240).expect("D3d11Context");
    let initial_drawable = win_surface::drawable_size(surface, 320, 240);
    assert_eq!(
        (ctx.width(), ctx.height()),
        (initial_drawable.width, initial_drawable.height)
    );

    // Grow the HWND; D3D11 resize reads GetClientRect.
    window
        .properties_mut()
        .set_size(900, 700)
        .expect("set_size");
    // Pump messages so WM_SIZE updates the client rect before GetClientRect.
    let _ = platform.event_loop().poll_event(&|_| true);

    let drawable = win_surface::drawable_size(surface, 1, 1);
    assert!(
        drawable.logical_width > 320 && drawable.logical_height > 240,
        "client should grow after set_size, got {}x{}",
        drawable.logical_width,
        drawable.logical_height
    );

    let (cw, ch) = (drawable.width, drawable.height);
    ctx.resize(drawable.logical_width, drawable.logical_height)
        .expect("resize after client-size change");
    assert_eq!(
        (ctx.width(), ctx.height()),
        (cw, ch),
        "D3D11 context must adopt the shared physical drawable extent after resize"
    );
    assert!(
        (ctx.caps().device_pixel_ratio - drawable.width as f32 / drawable.logical_width as f32)
            .abs()
            < f32::EPSILON,
        "D3D11 caps must report the drawable-to-logical DPR"
    );

    ctx.clear_render_target(1.0, 0.0, 0.0, 1.0)
        .expect("clear full RT after resize");
    let px = ctx
        .read_pixels(cw - 2, ch - 2, 1, 1)
        .expect("far-corner readback");
    assert_eq!(px.len(), 1, "far-corner readback");
    assert_eq!(
        px[0] >> 24,
        0xFF,
        "far corner must be opaque after clear; got {:#010X}",
        px[0]
    );
    assert_ne!(
        px[0] & 0x00FF_FFFF,
        0,
        "far corner must not stay black after resize clear"
    );

    ctx.present(&PresentFrame::Swapchain {
        damage: PresentDamage::Full,
    })
    .expect("present after resize");
    ctx.try_shutdown().expect("shutdown");
}

#[test]
fn d3d11_offscreen_target_create_bind_clear_blit_destroy() {
    let mut platform = crate::native::create_platform().expect("platform");
    let window = platform
        .window_manager()
        .create_window("D3D11 offscreen RT", 160, 120)
        .expect("window");
    let mut ctx = D3d11Context::new(window.native_surface_ptr(), 160, 120).expect("ctx");
    assert!(
        ctx.native_raster_caps().offscreen_targets,
        "D3D11 advertises Picture offscreen only after crop/scissor semantics are fixed"
    );

    let id = ctx
        .create_offscreen_target(32, 24)
        .expect("create_offscreen_target");
    ctx.bind_offscreen_target(id).expect("bind offscreen");
    ctx.clear_render_target(1.0, 0.0, 0.0, 1.0)
        .expect("clear offscreen");
    ctx.bind_swapchain_target().expect("bind swapchain");
    ctx.clear_render_target(0.0, 0.0, 0.0, 1.0)
        .expect("clear swapchain");
    ctx.blit_offscreen_target(
        id,
        crate::core::Rect::new(0.0, 0.0, 32.0, 24.0),
        crate::core::Rect::new(8.0, 8.0, 32.0, 24.0),
    )
    .expect("blit offscreen");
    ctx.destroy_offscreen_target(id);
    ctx.present(&PresentFrame::Swapchain {
        damage: PresentDamage::Full,
    })
    .expect("present");
    ctx.try_shutdown().expect("shutdown");
}

#[test]
fn d3d11_warp_offscreen_crop_ignores_and_restores_previous_raster_state() {
    let mut platform = crate::native::create_platform().expect("platform");
    let window = platform
        .window_manager()
        .create_window("D3D11 WARP offscreen crop", 96, 64)
        .expect("window");
    let mut ctx = create_with_driver(
        window.native_surface_ptr(),
        96,
        64,
        &D3D11_FEATURE_LEVELS,
        D3d11DriverKind::Warp,
    )
    .expect("D3D11 WARP context");
    assert_eq!(ctx.adapter_info.driver, D3d11DriverKind::Warp);
    assert!(ctx.native_raster_caps().offscreen_targets);

    let offscreen = ctx
        .create_offscreen_target(64, 48)
        .expect("create offscreen target");
    ctx.bind_offscreen_target(offscreen)
        .expect("bind offscreen target");
    ctx.clear_render_target(0.0, 0.0, 1.0, 1.0)
        .expect("clear source blue");
    ctx.draw_solid_rects(
        64.0,
        48.0,
        None,
        &[GpuSolidRect {
            x: 16.0,
            y: 8.0,
            w: 16.0,
            h: 16.0,
            rgba: [1.0, 0.0, 0.0, 1.0],
            radius: [0.0; 4],
        }],
    )
    .expect("paint source crop red");

    ctx.bind_swapchain_target().expect("bind swapchain target");
    ctx.clear_render_target(0.0, 0.0, 0.0, 1.0)
        .expect("clear swapchain black");
    ctx.draw_solid_rects(
        96.0,
        64.0,
        Some((0, 0, 4, 4)),
        &[GpuSolidRect {
            x: 0.0,
            y: 0.0,
            w: 96.0,
            h: 64.0,
            rgba: [0.0, 0.0, 0.0, 1.0],
            radius: [0.0; 4],
        }],
    )
    .expect("preset narrow scissor");

    ctx.blit_offscreen_target(
        offscreen,
        crate::core::Rect::new(16.0, 8.0, 16.0, 16.0),
        crate::core::Rect::new(48.0, 24.0, 16.0, 16.0),
    )
    .expect("blit nonzero source crop");

    assert_eq!(
        ctx.read_pixels(55, 31, 1, 1)
            .expect("offscreen crop readback"),
        vec![0xFFFF_0000],
        "the destination outside the old narrow scissor must sample the requested red source crop"
    );

    let mut viewport_count = 1;
    let mut viewport = D3D11_VIEWPORT::default();
    let mut scissor_count = 1;
    let mut scissor = ::windows::Win32::Foundation::RECT::default();
    unsafe {
        ctx.context
            .RSGetViewports(&mut viewport_count, Some(&mut viewport));
        ctx.context
            .RSGetScissorRects(&mut scissor_count, Some(&mut scissor));
    }
    assert_eq!(viewport_count, 1);
    assert_eq!(
        (
            viewport.TopLeftX,
            viewport.TopLeftY,
            viewport.Width,
            viewport.Height
        ),
        (0.0, 0.0, 96.0, 64.0),
        "offscreen blit must restore the caller viewport"
    );
    assert_eq!(scissor_count, 1);
    assert_eq!(
        (scissor.left, scissor.top, scissor.right, scissor.bottom),
        (0, 0, 4, 4),
        "offscreen blit must restore the caller scissor"
    );

    ctx.destroy_offscreen_target(offscreen);
    ctx.try_shutdown().expect("shutdown");
}

#[test]
fn repeated_present_keeps_single_rtv_and_stays_drawable() {
    // Regression: Present while holding RTV caused DXGI to allocate extra
    // buffers (memory growth). Release-before-Present + recreate-after must
    // keep drawing stable across many frames.
    let mut platform = crate::native::create_platform().expect("platform");
    let window = platform
        .window_manager()
        .create_window("D3D11 present memory", 160, 120)
        .expect("window");
    let surface = window.native_surface_ptr();
    let mut ctx = D3d11Context::new(surface, 160, 120).expect("D3d11Context");
    for i in 0..64 {
        let t = (i as f32) / 64.0;
        ctx.clear_render_target(t, 0.2, 1.0 - t, 1.0)
            .expect("clear");
        ctx.present(&PresentFrame::Swapchain {
            damage: PresentDamage::Full,
        })
        .expect("present");
        assert!(
            ctx.rtv.is_some(),
            "RTV must be recreated after Present for the next frame"
        );
    }
    let pixels = ctx
        .read_pixels(0, 0, 1, 1)
        .expect("repeated present readback");
    assert_eq!(pixels.len(), 1);
    assert_eq!(pixels[0] >> 24, 0xFF, "final frame must remain opaque");
    ctx.try_shutdown().expect("shutdown");
}

#[test]
fn dual_hwnd_theme_palette_clear_present_and_readback() {
    // P6 joint slice: two real HWNDs × D3D11 × light/dark layout palette
    // clear + Present + readback. Theme broadcast itself is covered by
    // FakePlatform; this closes the D3D11 multi-window present gap.
    let mut platform = crate::native::create_platform().expect("platform");
    let primary = platform
        .window_manager()
        .create_window("D3D11 theme primary", 160, 120)
        .expect("primary window");
    let secondary = platform
        .window_manager()
        .create_window("D3D11 theme secondary", 160, 120)
        .expect("secondary window");

    let mut ctx_a =
        D3d11Context::new(primary.native_surface_ptr(), 160, 120).expect("primary D3D11");
    let mut ctx_b =
        D3d11Context::new(secondary.native_surface_ptr(), 160, 120).expect("secondary D3D11");

    // Mirrors ThemePrimitives antd light/dark `color_bg_layout` (native
    // must not import `ui::Theme`).
    let light = (247.0 / 255.0, 247.0 / 255.0, 248.0 / 255.0, 1.0);
    let dark = (20.0 / 255.0, 20.0 / 255.0, 20.0 / 255.0, 1.0);

    let sample = |ctx: &mut D3d11Context, rgba: (f32, f32, f32, f32)| -> u32 {
        ctx.clear_render_target(rgba.0, rgba.1, rgba.2, rgba.3)
            .expect("clear");
        // Read before Present: DXGI_SWAP_EFFECT_DISCARD may drop contents.
        let pixels = ctx.read_pixels(0, 0, 1, 1).expect("theme palette readback");
        assert_eq!(pixels.len(), 1);
        ctx.present(&PresentFrame::Swapchain {
            damage: PresentDamage::Full,
        })
        .expect("present");
        assert!(ctx.rtv.is_some(), "RTV must exist after Present");
        pixels[0]
    };

    let a_light = sample(&mut ctx_a, light);
    let b_light = sample(&mut ctx_b, light);
    assert_eq!(a_light, b_light, "both windows must share light palette");
    assert_eq!(a_light >> 24, 0xFF);

    let a_dark = sample(&mut ctx_a, dark);
    let b_dark = sample(&mut ctx_b, dark);
    assert_eq!(a_dark, b_dark, "both windows must share dark palette");
    assert_ne!(
        a_light & 0x00FF_FFFF,
        a_dark & 0x00FF_FFFF,
        "light and dark layout palettes must differ on GPU"
    );

    ctx_a.try_shutdown().expect("shutdown");
    ctx_b.try_shutdown().expect("shutdown");
}

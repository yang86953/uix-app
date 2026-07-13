use super::*;
use crate::native::traits::present::{PresentMode, RasterMode};

#[test]
fn d3d12_hresult_classifies_device_loss_and_out_of_memory() {
    assert_eq!(
        d3d12_hresult_code(::windows::core::HRESULT(0x887A_0005u32 as i32)),
        Errc::GraphicsDeviceLost
    );
    assert_eq!(
        d3d12_hresult_code(::windows::core::HRESULT(0x8007_000Eu32 as i32)),
        Errc::GraphicsOutOfMemory
    );
}

#[test]
fn copy_mapped_rows_honors_offset_pitch_and_crop() {
    let row_pitch = 32usize;
    let offset = 16usize;
    let mut bytes = vec![0u8; offset + row_pitch * 4];
    for y in 0..4usize {
        for x in 0..5usize {
            let value = 0xFF00_0000u32 | ((y as u32) << 8) | x as u32;
            bytes[offset + y * row_pitch + x * 4..][..4].copy_from_slice(&value.to_ne_bytes());
        }
    }
    let pixels = copy_mapped_bgra_rows(bytes.as_ptr(), offset, row_pitch, 1, 1, 3, 2);
    assert_eq!(
        pixels,
        vec![
            0xFF00_0101,
            0xFF00_0102,
            0xFF00_0103,
            0xFF00_0201,
            0xFF00_0202,
            0xFF00_0203
        ]
    );
}

#[test]
fn d3d12_rejects_null_hwnd() {
    let error = match D3d12Context::new(std::ptr::null_mut(), 64, 64) {
        Ok(_) => panic!("null HWND must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code(), Errc::PlatformError);
}

#[test]
fn d3d12_context_clear_readback_present_resize_and_shutdown_on_real_window() {
    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("D3D12 context test", 65, 37)
        .expect("window");
    let surface = window.native_surface_ptr();
    assert!(!surface.is_null());

    let mut context = D3d12Context::new_with_driver(surface, 65, 37, D3d12DriverKind::Warp)
        .expect("D3D12 WARP context");
    assert_eq!(context.adapter_info.driver, D3d12DriverKind::Warp);
    assert_eq!(context.graphics_backend(), GraphicsBackend::D3d12);
    assert_eq!(context.caps().raster, RasterMode::GpuNative);
    assert_eq!(context.caps().present, PresentMode::Swapchain);
    assert!(!context.caps().partial_present);
    let drawable = win_surface::drawable_size(surface, 65, 37);
    assert_eq!(
        (context.width(), context.height()),
        (drawable.width, drawable.height)
    );
    assert!(
        (context.caps().device_pixel_ratio
            - drawable.width as f32 / drawable.logical_width as f32)
            .abs()
            < f32::EPSILON,
        "D3D12 caps must report the drawable-to-logical DPR"
    );
    assert_eq!(
        context.native_raster_caps(),
        NativeRasterCaps {
            clear_target: true,
            soft_blit: true,
            solid_rects: true,
            ..NativeRasterCaps::default()
        }
    );
    println!(
        "D3D12 WARP foundation adapter: {}",
        context.adapter_info.diagnostic_summary()
    );

    context
        .clear_render_target(0.25, 0.5, 0.75, 1.0)
        .expect("clear");
    let pixels = context
        .read_pixels(3, 5, 17, 11)
        .expect("D3D12 hardware readback");
    assert_eq!(pixels.len(), 17 * 11);
    assert!(pixels.iter().all(|pixel| *pixel == pixels[0]));
    let pixel = pixels[0];
    assert_eq!(pixel >> 24, 0xFF);
    assert!((0xBE..=0xC0).contains(&(pixel & 0xFF)));
    assert!((0x7F..=0x81).contains(&((pixel >> 8) & 0xFF)));
    assert!((0x3F..=0x41).contains(&((pixel >> 16) & 0xFF)));

    context
        .clear_render_target(0.0, 0.0, 1.0, 1.0)
        .expect("clear pipeline scene");
    let scene_w = context.width();
    let scene_h = context.height();
    context
        .draw_solid_rects(
            scene_w as f32,
            scene_h as f32,
            Some((6, 4, 28, 26)),
            &[
                GpuSolidRect {
                    x: 8.0,
                    y: 6.0,
                    w: 32.0,
                    h: 24.0,
                    rgba: [1.0, 0.0, 0.0, 1.0],
                    radius: [10.0, 0.0, 6.0, 0.0],
                },
                GpuSolidRect {
                    x: 12.0,
                    y: 10.0,
                    w: 8.0,
                    h: 8.0,
                    rgba: [0.0, 1.0, 0.0, 0.5],
                    radius: [0.0; 4],
                },
            ],
        )
        .expect("draw rounded/scissored solid rects");
    let mut soft_a = vec![0u32; (scene_w * scene_h) as usize];
    soft_a[0] = 0xFF11_22CC;
    soft_a[16 * scene_w as usize + 24] = 0x8000_0080;
    context
        .blit_soft_fallback(&soft_a, scene_w, scene_h)
        .expect("first soft blit");
    let mut soft_b = vec![0u32; (scene_w * scene_h) as usize];
    soft_b[(scene_w * scene_h) as usize - 1] = 0xFFCC_2211;
    context
        .blit_soft_fallback(&soft_b, scene_w, scene_h)
        .expect("second soft blit in one recording");
    let mixed = context
        .read_pixels_result(0, 0, scene_w, scene_h)
        .expect("mixed readback");
    let mixed_pixel = |x: usize, y: usize| mixed[y * scene_w as usize + x];
    assert_eq!(
        mixed_pixel(0, 0),
        0xFF11_22CC,
        "top-left texture orientation"
    );
    assert_eq!(
        mixed_pixel(scene_w as usize - 1, scene_h as usize - 1),
        0xFFCC_2211,
        "bottom-right texture orientation"
    );
    assert_eq!(mixed_pixel(8, 6), 0xFF00_00FF, "rounded corner stays blue");
    assert_eq!(
        mixed_pixel(32, 7),
        0xFFFF_0000,
        "sharp corner stays native red"
    );
    assert_eq!(
        mixed_pixel(36, 16),
        0xFF00_00FF,
        "scissor clips native rect"
    );
    assert_eq!(
        mixed_pixel(24, 16),
        0xFF7F_0080,
        "premultiplied soft alpha-over"
    );
    let solid_alpha = mixed_pixel(14, 12);
    assert_eq!(solid_alpha >> 24, 0xFF);
    assert!((127..=128).contains(&((solid_alpha >> 16) & 0xFF)));
    assert!((127..=128).contains(&((solid_alpha >> 8) & 0xFF)));
    assert_eq!(solid_alpha & 0xFF, 0);
    assert_eq!(
        mixed_pixel(28, 16),
        0xFFFF_0000,
        "transparent soft keeps native"
    );
    context
        .present(&PresentFrame::Swapchain {
            damage: PresentDamage::Full,
        })
        .expect("present");
    assert_eq!(
        context
            .blit_soft_fallback(&[], scene_w, scene_h)
            .expect_err("short soft input must fail")
            .code(),
        Errc::InvalidArgument
    );
    assert_eq!(
        context
            .blit_soft_fallback(&soft_a, scene_w - 1, scene_h)
            .expect_err("mismatched soft dimensions must fail")
            .code(),
        Errc::InvalidArgument
    );

    let original_size = (context.width(), context.height());
    context
        .clear_render_target(0.5, 0.5, 0.0, 1.0)
        .expect("record before resize");
    window
        .properties_mut()
        .set_size(93, 69)
        .expect("resize native window");
    context.resize_result(93, 69).expect("resize");
    assert_ne!((context.width(), context.height()), original_size);
    context
        .clear_render_target(1.0, 0.0, 0.0, 1.0)
        .expect("clear after resize");
    let resized = context
        .read_pixels(0, 0, context.width(), context.height())
        .expect("D3D12 resized readback");
    assert_eq!(resized.len(), (context.width() * context.height()) as usize);
    assert!(resized.iter().all(|pixel| *pixel == 0xFFFF_0000));
    context
        .present(&PresentFrame::Swapchain {
            damage: PresentDamage::Full,
        })
        .expect("present after resize");
    for (color, expected) in [
        ((0.0, 0.0, 1.0, 1.0), 0xFF00_00FF),
        ((1.0, 1.0, 0.0, 1.0), 0xFFFF_FF00),
        ((1.0, 0.0, 1.0, 1.0), 0xFFFF_00FF),
    ] {
        context
            .clear_render_target(color.0, color.1, color.2, color.3)
            .expect("clear across allocator ring");
        assert_eq!(context.read_pixels_result(0, 0, 1, 1), Ok(vec![expected]));
        context
            .present(&PresentFrame::Swapchain {
                damage: PresentDamage::Full,
            })
            .expect("present across allocator ring");
    }
    context.try_shutdown().expect("shutdown");
    context.try_shutdown().expect("shutdown remains idempotent");
    drop(context);

    let mut warp = D3d12Context::new_with_driver(surface, 65, 37, D3d12DriverKind::Warp)
        .expect("D3D12 WARP context");
    assert_eq!(warp.adapter_info.driver, D3d12DriverKind::Warp);
    println!(
        "D3D12 WARP real-window adapter: {}",
        warp.adapter_info.diagnostic_summary()
    );
    warp.clear_render_target(0.0, 1.0, 0.0, 1.0)
        .expect("WARP clear");
    let warp_pixels = warp
        .read_pixels(0, 0, warp.width(), warp.height())
        .expect("D3D12 WARP readback");
    assert_eq!(warp_pixels.first().copied(), Some(0xFF00_FF00));

    let replace = vec![0xFF00_00FFu32; (warp.width() * warp.height()) as usize];
    warp.upload_surface_pixels(&replace, warp.width(), warp.height())
        .expect("WARP replace upload");
    let replaced = warp
        .read_pixels(0, 0, warp.width(), warp.height())
        .expect("D3D12 WARP readback after replace upload");
    assert!(
        replaced.iter().all(|pixel| *pixel == 0xFF00_00FF),
        "upload_surface_pixels must replace the drawable, not alpha-over it"
    );

    warp.present(&PresentFrame::Swapchain {
        damage: PresentDamage::Full,
    })
    .expect("WARP present");
    warp.clear_render_target(0.0, 0.0, 1.0, 1.0)
        .expect("record work before injected present failure");
    let error = warp
        .latch_present_result(Err(Error::new(
            Errc::PlatformError,
            "injected present failure",
        )))
        .expect_err("failed Present must propagate its error");
    assert_eq!(error.code(), Errc::PlatformError);
    assert!(
        warp.fault
            .as_deref()
            .is_some_and(|fault| fault.starts_with("present:")),
        "failed Present must latch the context before later work"
    );
    let error = warp
        .clear_render_target(1.0, 0.0, 0.0, 1.0)
        .expect_err("faulted context must reject new recording");
    assert_eq!(error.code(), Errc::InvalidState);
    let error = warp
        .resize_result(warp.width() + 1, warp.height() + 1)
        .expect_err("faulted context must reject resize");
    assert_eq!(error.code(), Errc::InvalidState);
    assert!(
        warp.present(&PresentFrame::Swapchain {
            damage: PresentDamage::Full,
        })
        .is_err()
    );
    assert!(warp.read_pixels_result(0, 0, 1, 1).is_err());
    warp.try_shutdown()
        .expect("faulted context should still terminal-fence drain");
    warp.try_shutdown().expect("shutdown remains idempotent");
    drop(warp);

    let hardware_factory: IDXGIFactory4 =
        unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }
            .expect("factory for hardware availability probe");
    match select_hardware_adapter(&hardware_factory) {
        Ok((_adapter, _device, available_info)) => {
            let mut hardware =
                D3d12Context::new_with_driver(surface, 93, 69, D3d12DriverKind::Hardware)
                    .expect("available D3D12 hardware must create a real-window context");
            assert_eq!(hardware.adapter_info.driver, D3d12DriverKind::Hardware);
            assert_eq!(hardware.adapter_info.device_id, available_info.device_id);
            println!(
                "D3D12 hardware real-window adapter: {}",
                hardware.adapter_info.diagnostic_summary()
            );
            assert_eq!(
                hardware.native_raster_caps(),
                NativeRasterCaps {
                    clear_target: true,
                    soft_blit: true,
                    solid_rects: true,
                    ..NativeRasterCaps::default()
                }
            );
            hardware
                .clear_render_target(0.0, 0.0, 1.0, 1.0)
                .expect("hardware clear");
            let hardware_w = hardware.width();
            let hardware_h = hardware.height();
            hardware
                .draw_solid_rects(
                    hardware_w as f32,
                    hardware_h as f32,
                    None,
                    &[GpuSolidRect {
                        x: 10.0,
                        y: 10.0,
                        w: 30.0,
                        h: 20.0,
                        rgba: [1.0, 0.0, 0.0, 1.0],
                        radius: [6.0; 4],
                    }],
                )
                .expect("hardware rounded solid");
            let mut hardware_soft = vec![0u32; (hardware_w * hardware_h) as usize];
            hardware_soft[15 * hardware_w as usize + 20] = 0x8000_8000;
            hardware
                .blit_soft_fallback(&hardware_soft, hardware_w, hardware_h)
                .expect("hardware soft blit");
            let hardware_pixels = hardware
                .read_pixels_result(0, 0, hardware_w, hardware_h)
                .expect("hardware mixed readback");
            let hardware_pixel =
                |x: usize, y: usize| hardware_pixels[y * hardware_w as usize + x];
            assert_eq!(hardware_pixel(10, 10), 0xFF00_00FF);
            assert_eq!(hardware_pixel(20, 15), 0xFF7F_8000);
            hardware
                .present(&PresentFrame::Swapchain {
                    damage: PresentDamage::Full,
                })
                .expect("hardware present");
            hardware.shutdown_result().expect("hardware shutdown");
            drop(hardware);
        }
        Err(error) => println!("D3D12 hardware real-window smoke skipped: {}", error.what()),
    }
    window.close().expect("close window");
}

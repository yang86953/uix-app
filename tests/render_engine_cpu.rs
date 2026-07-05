//! uix-graphics CPU 引擎集成测试。
//! 覆盖 PixelSurface、RasterRenderer、CpuCanvas2D、SoftwareEngine、NullEngine。

use uix::render::color::Color;
use uix::render::engine::cpu::canvas_2d::CpuCanvas2D;
use uix::render::engine::cpu::pixel_surface::PixelSurface;
use uix::render::engine::cpu::raster_renderer::RasterRenderer;
use uix::render::engine::cpu::software::SoftwareEngine;
use uix::render::null_engine::NullEngine;
use uix::render::traits::{Canvas2D, GraphicsEngine, UpdateStrategy};
use uix::render::types::ImageHandle;
use uix::platform::api::geometry::{Rect, Size};

// ════════════════════════════════════════════════════════════════════════════
// PixelSurface 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn pixel_surface_new_zero_size_uses_minimum() {
    let surf = PixelSurface::new(0, 0);
    assert_eq!(surf.width(), 1);
    assert_eq!(surf.height(), 1);
    assert_eq!(surf.surface_size(), Size::new(1.0, 1.0));
    assert_eq!(surf.pixels().len(), 1);
}

#[test]
fn pixel_surface_new_normal_size() {
    let surf = PixelSurface::new(100, 50);
    assert_eq!(surf.width(), 100);
    assert_eq!(surf.height(), 50);
    assert_eq!(surf.surface_size(), Size::new(100.0, 50.0));
    assert_eq!(surf.pixels().len(), 5000);
}

#[test]
fn pixel_surface_all_pixels_start_transparent() {
    let surf = PixelSurface::new(10, 10);
    for &p in surf.pixels() {
        assert_eq!(p, 0x00000000);
    }
}

#[test]
fn pixel_surface_clear_all() {
    let mut surf = PixelSurface::new(8, 8);
    surf.set_clear_color(Color::from_rgba(255, 0, 0, 255));
    surf.clear_all();
    for &p in surf.pixels() {
        assert_eq!(p, 0xFF0000FF); // RGBA
    }
}

#[test]
fn pixel_surface_clear_rect_raw_center() {
    let mut surf = PixelSurface::new(10, 10);
    surf.set_clear_color(Color::from_rgba(0, 255, 0, 255));
    surf.clear_rect_raw(2, 2, 4, 4);

    // 检查 cleared 区域
    for y in 2..6 {
        for x in 2..6 {
            let idx = y * 10 + x;
            assert_eq!(surf.pixels()[idx], 0xFF00FF00);
        }
    }
    // 角落仍为透明
    assert_eq!(surf.pixels()[0], 0x00000000);
}

#[test]
fn pixel_surface_clear_rect_raw_clamped() {
    let mut surf = PixelSurface::new(10, 10);
    surf.set_clear_color(Color::from_rgba(255, 255, 255, 255));
    surf.clear_rect_raw(-5, -5, 20, 20); // 超出边界应被 clamp
    for &p in surf.pixels() {
        assert_eq!(p, 0xFFFFFFFF);
    }
}

#[test]
fn pixel_surface_clear_rect_raw_zero_size_noop() {
    let mut surf = PixelSurface::new(10, 10);
    surf.set_clear_color(Color::from_rgba(255, 0, 0, 255));
    surf.clear_rect_raw(0, 0, 0, 0);
    for &p in surf.pixels() {
        assert_eq!(p, 0x00000000);
    }
}

#[test]
fn pixel_surface_copy_region_within_bounds() {
    use uix::render::traits::RenderingBackend;
    let mut surf = PixelSurface::new(10, 10);
    // 在左上角写一些像素
    surf.set_clear_color(Color::from_rgba(255, 255, 255, 255));
    surf.clear_rect_raw(0, 0, 5, 5);

    // 复制到右下角
    RenderingBackend::copy_region(&mut surf, Rect::new(0.0, 0.0, 5.0, 5.0), 5, 5);

    assert_eq!(surf.pixels()[5 * 10 + 5], 0xFFFFFFFF);
}

#[test]
fn pixel_surface_copy_region_zero_size_noop() {
    use uix::render::traits::RenderingBackend;
    let mut surf = PixelSurface::new(10, 10);
    RenderingBackend::copy_region(&mut surf, Rect::new(0.0, 0.0, 0.0, 0.0), 5, 5);
    // 不应 panic
}

#[test]
fn pixel_surface_pixels_mut_modifies() {
    let mut surf = PixelSurface::new(4, 4);
    surf.pixels_mut()[0] = 0xFF0000FF;
    assert_eq!(surf.pixels()[0], 0xFF0000FF);
}

// ════════════════════════════════════════════════════════════════════════════
// RasterRenderer 测试（状态管理）
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn raster_renderer_new_default_state() {
    let rr = RasterRenderer::new(800, 600);
    assert_eq!(rr.opacity(), 1.0);
    assert_eq!(rr.offset(), (0.0, 0.0));
    assert_eq!(rr.clip_rect(), Rect::new(0.0, 0.0, 800.0, 600.0));
}

#[test]
fn raster_renderer_save_restore() {
    let mut rr = RasterRenderer::new(800, 600);
    rr.set_offset(10.0, 20.0);
    rr.set_opacity(0.5);
    rr.save();

    rr.set_offset(30.0, 40.0);
    rr.set_opacity(0.8);
    assert_eq!(rr.offset(), (30.0, 40.0));
    assert!((rr.opacity() - 0.8).abs() < 1e-6);

    rr.restore();
    assert_eq!(rr.offset(), (10.0, 20.0));
    assert!((rr.opacity() - 0.5).abs() < 1e-6);
}

#[test]
fn raster_renderer_restore_empty_stack_noop() {
    let mut rr = RasterRenderer::new(100, 100);
    rr.restore(); // 空栈不应 panic
    assert_eq!(rr.clip_rect(), Rect::new(0.0, 0.0, 100.0, 100.0));
}

#[test]
fn raster_renderer_push_pop_clip() {
    let mut rr = RasterRenderer::new(800, 600);
    rr.push_clip(Rect::new(10.0, 10.0, 100.0, 100.0));
    assert_eq!(rr.clip_rect(), Rect::new(10.0, 10.0, 100.0, 100.0));

    rr.push_clip(Rect::new(50.0, 50.0, 100.0, 100.0));
    // 交集应为 50,50 ~ 110,110 (50x50 + 100 = 150, clamped to 110)
    assert_eq!(rr.clip_rect(), Rect::new(50.0, 50.0, 60.0, 60.0));

    rr.pop_clip();
    assert_eq!(rr.clip_rect(), Rect::new(10.0, 10.0, 100.0, 100.0));

    rr.pop_clip();
    assert_eq!(rr.clip_rect(), Rect::new(0.0, 0.0, 800.0, 600.0));
}

#[test]
fn raster_renderer_clip_no_intersection() {
    let mut rr = RasterRenderer::new(100, 100);
    rr.push_clip(Rect::new(200.0, 200.0, 50.0, 50.0)); // 无交集
    assert_eq!(rr.clip_rect(), Rect::zero());
}

#[test]
fn raster_renderer_opacity_clamp() {
    let mut rr = RasterRenderer::new(100, 100);
    rr.set_opacity(2.0);
    assert!((rr.opacity() - 1.0).abs() < 1e-6);
    rr.set_opacity(-0.5);
    assert!((rr.opacity()).abs() < 1e-6);
}

#[test]
fn raster_renderer_offset_affects_set_get() {
    let mut rr = RasterRenderer::new(100, 100);
    assert_eq!(rr.offset(), (0.0, 0.0));
    rr.set_offset(15.5, -3.2);
    assert_eq!(rr.offset(), (15.5, -3.2));
}

#[test]
fn raster_renderer_fill_rect_no_transform() {
    let rr = RasterRenderer::new(10, 10);
    let mut pixels = vec![0u32; 100];
    rr.fill_rect(
        &mut pixels,
        10,
        10,
        Rect::new(0.0, 0.0, 5.0, 5.0),
        Color::from_rgba(255, 0, 0, 255),
        None,
    );

    // 前 5x5 像素应为红色
    for y in 0..5 {
        for x in 0..5 {
            assert_eq!(pixels[y * 10 + x], 0xFF0000FF, "pixel ({x},{y}) 应为红色");
        }
    }
    // 第 5 列应为透明
    for y in 0..10 {
        assert_eq!(pixels[y * 10 + 5], 0x00000000, "pixel (5,{y}) 应为透明");
    }
}

#[test]
fn raster_renderer_fill_rect_respects_clip() {
    let mut rr = RasterRenderer::new(10, 10);
    rr.push_clip(Rect::new(2.0, 2.0, 4.0, 4.0));
    let mut pixels = vec![0u32; 100];
    rr.fill_rect(
        &mut pixels,
        10,
        10,
        Rect::new(0.0, 0.0, 10.0, 10.0),
        Color::from_rgba(0, 255, 0, 255),
        None,
    );

    // 只有裁剪区域内被填充
    assert_eq!(pixels[0], 0x00000000);
    for y in 2..6 {
        for x in 2..6 {
            assert_eq!(pixels[y * 10 + x], 0xFF00FF00);
        }
    }
}

#[test]
fn raster_renderer_stroke_rect_fast_path() {
    let rr = RasterRenderer::new(10, 10);
    let mut pixels = vec![0u32; 100];
    // 1px 描边
    rr.stroke_rect(
        &mut pixels,
        10,
        10,
        Rect::new(0.0, 0.0, 10.0, 10.0),
        Color::from_rgba(255, 255, 255, 255),
        1.0,
        None,
    );

    // 边框像素应为白色
    assert_eq!(pixels[0], 0xFFFFFFFF);
    assert_eq!(pixels[9], 0xFFFFFFFF);
    assert_eq!(pixels[90], 0xFFFFFFFF);
    assert_eq!(pixels[99], 0xFFFFFFFF);
    // 内部应为透明
    assert_eq!(pixels[11], 0x00000000);
    assert_eq!(pixels[88], 0x00000000);
}

#[test]
fn raster_renderer_draw_line_horizontal() {
    let rr = RasterRenderer::new(10, 10);
    let mut pixels = vec![0u32; 100];
    rr.draw_line(
        &mut pixels,
        10,
        10,
        0.0,
        5.0,
        9.0,
        5.0,
        Color::from_rgba(255, 255, 255, 255),
        1.0,
    );

    // 水平线 y=5, lw=1, fill_rect_raw 覆盖 (0,4)-(9,5)
    assert_eq!(pixels[4 * 10 + 5], 0xFFFFFFFF);
    // 线上方一个像素为透明
    assert_eq!(pixels[5 * 10 + 5], 0x00000000);
}

// ════════════════════════════════════════════════════════════════════════════
// CpuCanvas2D 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn cpu_canvas_2d_new_has_correct_size() {
    let surf = PixelSurface::new(320, 240);
    let canvas = CpuCanvas2D::new(surf);
    assert_eq!(canvas.surface_size(), Size::new(320.0, 240.0));
    assert_eq!(canvas.width(), 320);
    assert_eq!(canvas.height(), 240);
}

#[test]
fn cpu_canvas_2d_fill_rect_changes_pixels() {
    let surf = PixelSurface::new(10, 10);
    let mut canvas = CpuCanvas2D::new(surf);
    canvas.fill_rect(
        Rect::new(0.0, 0.0, 5.0, 5.0),
        Color::from_rgba(255, 0, 0, 255),
        None,
    );

    let pixels = canvas.surface().pixels();
    assert_eq!(pixels[0], 0xFF0000FF);
    assert_eq!(pixels[5 * 10 + 5], 0x00000000);
}

#[test]
fn cpu_canvas_2d_fill_circle_changes_pixels() {
    let surf = PixelSurface::new(20, 20);
    let mut canvas = CpuCanvas2D::new(surf);
    // 使用红色以便与 to_rgba() 的 ABGR 格式匹配
    canvas.fill_circle(10.0, 10.0, 5.0, Color::from_rgba(255, 0, 0, 255));

    let pixels = canvas.surface().pixels();
    // 圆心附近应有红色像素
    let center_color = pixels[10 * 20 + 10];
    let a = (center_color >> 24) & 0xFF;
    let r = center_color & 0xFF;
    assert!(a > 0, "圆心 alpha 应为正");
    assert!(r > 0, "圆心红色分量应为正");
}

#[test]
fn cpu_canvas_2d_save_restore_clip() {
    let surf = PixelSurface::new(20, 20);
    let mut canvas = CpuCanvas2D::new(surf);

    canvas.push_clip(Rect::new(5.0, 5.0, 10.0, 10.0));
    canvas.save();
    canvas.push_clip(Rect::new(8.0, 8.0, 4.0, 4.0));
    assert_eq!(canvas.current_clip(), Rect::new(8.0, 8.0, 4.0, 4.0));

    canvas.restore();
    assert_eq!(canvas.current_clip(), Rect::new(5.0, 5.0, 10.0, 10.0));
}

#[test]
fn cpu_canvas_2d_opacity_affects_rendering() {
    let surf = PixelSurface::new(10, 10);
    let mut canvas = CpuCanvas2D::new(surf);
    canvas.set_opacity(0.5);
    canvas.fill_rect(
        Rect::new(0.0, 0.0, 5.0, 5.0),
        Color::from_rgba(255, 0, 0, 255),
        None,
    );

    let pixels = canvas.surface().pixels();
    // 半透明度混合结果应与完全不透明不同
    assert_ne!(pixels[0], 0xFF0000FF);
    // alpha 应介于 0 和 255 之间
    let a = (pixels[0] >> 24) & 0xFF;
    assert!(a > 0 && a < 255);
}

#[test]
fn cpu_canvas_2d_translate_shifts_output() {
    let surf = PixelSurface::new(20, 20);
    let mut canvas = CpuCanvas2D::new(surf);
    canvas.translate(5.0, 5.0);
    canvas.fill_rect(
        Rect::new(0.0, 0.0, 5.0, 5.0),
        Color::from_rgba(255, 0, 0, 255),
        None,
    );

    let pixels = canvas.surface().pixels();
    // 偏移后，颜色应在 (5,5) 处而不在 (0,0)
    assert_eq!(pixels[0], 0x00000000);
    assert_eq!(pixels[5 * 20 + 5], 0xFF0000FF);
}

#[test]
fn cpu_canvas_2d_scroll_region() {
    let mut surf = PixelSurface::new(16, 16);
    surf.set_clear_color(Color::from_rgba(255, 255, 255, 255));
    surf.clear_rect_raw(0, 0, 8, 8);
    let mut canvas = CpuCanvas2D::new(surf);

    let viewport = Rect::new(0.0, 0.0, 16.0, 16.0);
    canvas.scroll_region(viewport, 4.0, 0.0);

    let pixels = canvas.surface().pixels();
    // scroll_region dx=4: src=(4,0,16,16), dst=(0,0)
    // 旧 (4,0) 处的像素复制到 (0,0)
    // 原本 (0,0)-(7,7) 是白色，偏移后白色从 (4,0) 复制到 (0,0)
    assert_eq!(pixels[0], 0xFFFFFFFF, "(0,0) 应为白色（从 (4,0) 复制）");
    // 旧 (0,0) 处的像素未被覆盖（复制是移动而非拷贝）
    // 旧 (4,0) 白色被复制到 (0,0)，(4,0) 本身不变（因为 dst <= src）
}

#[test]
fn cpu_canvas_2d_scroll_region_zero_delta_noop() {
    let surf = PixelSurface::new(16, 16);
    let mut canvas = CpuCanvas2D::new(surf);
    let pixels_before = canvas.surface().pixels().to_vec();
    canvas.scroll_region(Rect::new(0.0, 0.0, 16.0, 16.0), 0.0, 0.0);
    let pixels_after = canvas.surface().pixels().to_vec();
    assert_eq!(pixels_before, pixels_after);
}

// ════════════════════════════════════════════════════════════════════════════
// CpuCanvas2D — 补充测试（fill_sector / shadow / blend_mode / clip_path）
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn cpu_canvas_2d_fill_sector_pixels() {
    let surf = PixelSurface::new(20, 20);
    let mut canvas = CpuCanvas2D::new(surf);
    canvas.fill_sector(
        10.0,
        10.0,
        5.0,
        0.0,
        std::f32::consts::PI,
        Color::from_rgba(255, 0, 0, 255),
    );
    let pixels = canvas.surface().pixels();
    // 圆心应在扇形范围内（像素格式 AABBGGRR，R 在最低字节）
    let center = pixels[10 * 20 + 10];
    let r = center & 0xFF;
    assert_eq!(r, 0xFF, "圆心红色分量应为 0xFF");
}

#[test]
fn cpu_canvas_2d_draw_box_shadow_changes_pixels() {
    let surf = PixelSurface::new(20, 20);
    let mut canvas = CpuCanvas2D::new(surf);
    canvas.draw_box_shadow(
        Rect::new(5.0, 5.0, 10.0, 10.0),
        1.0,
        3.0,
        3.0,
        Color::from_rgba(0, 0, 0, 128),
        None,
    );
    let pixels = canvas.surface().pixels();
    // 阴影偏移后应在 (8,8) 附近有像素
    let pixel = pixels[8 * 20 + 8];
    let a = (pixel >> 24) & 0xFF;
    assert!(a > 0, "阴影区域应有非零 alpha");
}

#[test]
fn cpu_canvas_2d_draw_box_shadow_ambient_changes_pixels() {
    let surf = PixelSurface::new(20, 20);
    let mut canvas = CpuCanvas2D::new(surf);
    canvas.draw_box_shadow_ambient(
        Rect::new(5.0, 5.0, 10.0, 10.0),
        2.0,
        0.0,
        0.0,
        Color::from_rgba(0, 0, 0, 128),
        None,
    );
    let pixels = canvas.surface().pixels();
    // 环境阴影应在盒子附近有像素
    let pixel = pixels[10 * 20 + 10];
    let a = (pixel >> 24) & 0xFF;
    assert!(a > 0, "环境阴影区域应有非零 alpha");
}

#[test]
fn cpu_canvas_2d_set_blend_mode_no_panic() {
    let surf = PixelSurface::new(10, 10);
    let mut canvas = CpuCanvas2D::new(surf);
    canvas.set_blend_mode(uix::render::types::BlendMode::Alpha);
    canvas.fill_rect(
        Rect::new(0.0, 0.0, 5.0, 5.0),
        Color::from_rgba(255, 0, 0, 255),
        None,
    );
    let pixels = canvas.surface().pixels();
    // set_blend_mode 后填充应正常工作
    assert_eq!(pixels[0], 0xFF0000FF);
}

#[test]
fn cpu_canvas_2d_push_clip_path_no_panic() {
    use uix::render::path::PathBuilder;
    let surf = PixelSurface::new(10, 10);
    let mut canvas = CpuCanvas2D::new(surf);
    let path = PathBuilder::new()
        .move_to(0.0, 0.0)
        .line_to(10.0, 0.0)
        .line_to(10.0, 10.0)
        .close()
        .build();
    // 路径裁剪当前为空操作，确保不 panic
    canvas.push_clip_path(&path);
    canvas.fill_rect(
        Rect::new(0.0, 0.0, 5.0, 5.0),
        Color::from_rgba(0, 255, 0, 255),
        None,
    );
    let pixels = canvas.surface().pixels();
    assert_eq!(pixels[0], 0xFF00FF00);
}

// ════════════════════════════════════════════════════════════════════════════
// SoftwareEngine 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn software_engine_new_default() {
    let mut engine = SoftwareEngine::new();
    assert_eq!(engine.canvas_2d().width(), 1);
    assert_eq!(engine.canvas_2d().height(), 1);
}

#[test]
fn software_engine_initialize() {
    let mut engine = SoftwareEngine::new();
    let result = engine.initialize(800, 600);
    assert!(result.is_ok());
    assert_eq!(engine.canvas_2d().width(), 800);
    assert_eq!(engine.canvas_2d().height(), 600);
}

#[test]
fn software_engine_begin_frame_full_redraw() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(10, 10).unwrap();
    let result = engine.begin_frame(UpdateStrategy::FullRedraw);
    assert!(matches!(
        result,
        uix::render::engine::RenderOutcome::Present(_)
    ));
    engine.end_frame();
}

#[test]
fn software_engine_begin_frame_overlay() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(10, 10).unwrap();
    let result = engine.begin_frame(UpdateStrategy::Overlay(vec![Rect::new(
        0.0, 0.0, 10.0, 10.0,
    )]));
    assert!(matches!(
        result,
        uix::render::engine::RenderOutcome::Present(_)
    ));
    engine.end_frame();
}

#[test]
fn software_engine_resize() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(800, 600).unwrap();
    engine.resize(400, 300);
    assert_eq!(engine.canvas_2d().width(), 400);
    assert_eq!(engine.canvas_2d().height(), 300);
}

#[test]
fn software_engine_shutdown() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(800, 600).unwrap();
    engine.shutdown();
    assert_eq!(engine.canvas_2d().width(), 1);
    assert_eq!(engine.canvas_2d().height(), 1);
}

#[test]
fn software_engine_canvas_2d_mut_ref() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(50, 50).unwrap();
    let canvas = engine.canvas_2d();
    canvas.fill_rect(
        Rect::new(0.0, 0.0, 10.0, 10.0),
        Color::from_rgba(0, 255, 0, 255),
        None,
    );
}

#[test]
fn software_engine_create_offscreen() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(100, 100).unwrap();
    let handle = engine.create_offscreen(50, 50);
    assert!(handle.is_some());
    let handle = handle.unwrap();
    assert_eq!(handle.0, 0);

    let offscreen_canvas = engine.offscreen_canvas(&handle);
    assert!(offscreen_canvas.is_some());
    assert_eq!(
        offscreen_canvas.unwrap().surface_size(),
        Size::new(50.0, 50.0)
    );
}

#[test]
fn software_engine_create_offscreen_zero_size_returns_none() {
    let mut engine = SoftwareEngine::new();
    let handle = engine.create_offscreen(0, 0);
    assert!(handle.is_none());
    let handle = engine.create_offscreen(-1, 10);
    assert!(handle.is_none());
}

#[test]
fn software_engine_destroy_offscreen() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(100, 100).unwrap();
    let handle = engine.create_offscreen(50, 50).unwrap();
    engine.destroy_offscreen(handle);
    assert!(engine.offscreen_canvas(&handle).is_none());
}

#[test]
fn software_engine_destroy_offscreen_invalid_handle_noop() {
    let mut engine = SoftwareEngine::new();
    engine.destroy_offscreen(ImageHandle(999)); // 不应 panic
}

#[test]
fn software_engine_offscreen_canvas_invalid_handle() {
    let mut engine = SoftwareEngine::new();
    assert!(engine.offscreen_canvas(&ImageHandle(999)).is_none());
}

#[test]
fn software_engine_blit_offscreen() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(100, 100).unwrap();
    let handle = engine.create_offscreen(10, 10).unwrap();

    // 在离屏上绘制
    let offscreen = engine.offscreen_canvas(&handle).unwrap();
    offscreen.fill_rect(
        Rect::new(0.0, 0.0, 10.0, 10.0),
        Color::from_rgba(255, 0, 0, 255),
        None,
    );

    // blit 到主表面
    engine.blit_offscreen(&handle, Rect::new(0.0, 0.0, 10.0, 10.0));
}

#[test]
fn software_engine_blit_offscreen_invalid_handle_noop() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(100, 100).unwrap();
    engine.blit_offscreen(&ImageHandle(999), Rect::new(0.0, 0.0, 10.0, 10.0)); // 不应 panic
}

#[test]
fn software_engine_blit_offscreen_src_basic() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(100, 100).unwrap();
    let handle = engine.create_offscreen(20, 20).unwrap();
    // 在离屏上绘制红色
    let offscreen = engine.offscreen_canvas(&handle).unwrap();
    offscreen.fill_rect(
        Rect::new(0.0, 0.0, 10.0, 10.0),
        Color::from_rgba(255, 0, 0, 255),
        None,
    );
    // 使用 blit_offscreen_src 只 blit 离屏的 (0,0,10,10) 到主画布 (0,0,10,10)
    engine.blit_offscreen_src(
        &handle,
        Rect::new(0.0, 0.0, 10.0, 10.0),
        Rect::new(0.0, 0.0, 10.0, 10.0),
    );
}

#[test]
fn software_engine_blit_offscreen_src_invalid_handle_noop() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(100, 100).unwrap();
    engine.blit_offscreen_src(
        &ImageHandle(999),
        Rect::new(0.0, 0.0, 10.0, 10.0),
        Rect::new(0.0, 0.0, 10.0, 10.0),
    );
}

#[test]
fn software_engine_memory_usage() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(100, 100).unwrap();
    // 100*100*4 = 40000 bytes
    let mem = engine.memory_usage();
    assert!(mem >= 40000);
}

#[test]
fn software_engine_memory_usage_with_offscreen() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(50, 50).unwrap();
    engine.create_offscreen(30, 30);
    let mem = engine.memory_usage();
    // 主表面 50*50*4 = 10000 + 离屏 30*30*4 = 3600 = 13600
    assert!(mem >= 13600);
}

#[test]
fn software_engine_dpi_default() {
    let engine = SoftwareEngine::new();
    assert!((engine.dpi() - 96.0).abs() < 1e-6);
}

#[test]
fn software_engine_diagnose_memory_does_not_panic() {
    let engine = SoftwareEngine::new();
    engine.diagnose_memory(); // 默认实现仅打日志，不应 panic
}

#[test]
fn software_engine_device_pixel_ratio_default() {
    let engine = SoftwareEngine::new();
    assert!((engine.device_pixel_ratio() - 1.0).abs() < 1e-6);
}

#[test]
fn software_engine_orientation_default() {
    let engine = SoftwareEngine::new();
    assert_eq!(
        engine.orientation(),
        uix::render::spatial::Orientation::YDown
    );
}

// ════════════════════════════════════════════════════════════════════════════
// NullEngine 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn null_engine_new() {
    let mut engine = NullEngine::new();
    assert_eq!(engine.canvas_2d().surface_size(), Size::new(0.0, 0.0));
}

#[test]
fn null_engine_default() {
    let mut engine = NullEngine::default();
    assert_eq!(engine.canvas_2d().surface_size(), Size::new(0.0, 0.0));
}

#[test]
fn null_engine_initialize() {
    let mut engine = NullEngine::new();
    assert!(engine.initialize(800, 600).is_ok());
}

#[test]
fn null_engine_begin_frame() {
    let mut engine = NullEngine::new();
    engine.initialize(800, 600).unwrap();
    let result = engine.begin_frame(UpdateStrategy::FullRedraw);
    assert!(matches!(result, uix::render::engine::RenderOutcome::Idle));
}

#[test]
fn null_engine_end_frame() {
    let mut engine = NullEngine::new();
    let result = engine.end_frame();
    assert!(matches!(result, uix::render::engine::RenderOutcome::Idle));
}

#[test]
fn null_engine_resize() {
    let mut engine = NullEngine::new();
    engine.resize(640, 480); // 不应 panic
}

#[test]
fn null_engine_shutdown() {
    let mut engine = NullEngine::new();
    engine.shutdown(); // 不应 panic
}

#[test]
fn null_engine_canvas_2d_ops_noop() {
    let mut engine = NullEngine::new();
    let canvas = engine.canvas_2d();
    canvas.fill_rect(
        Rect::new(0.0, 0.0, 100.0, 100.0),
        Color::from_rgba(255, 0, 0, 255),
        None,
    );
    canvas.save();
    canvas.restore();
    canvas.push_clip(Rect::new(0.0, 0.0, 50.0, 50.0));
    canvas.pop_clip();
    canvas.set_opacity(0.5);
}

#[test]
fn null_engine_create_offscreen_returns_none() {
    let mut engine = NullEngine::new();
    assert!(engine.create_offscreen(100, 100).is_none());
}

#[test]
fn null_engine_destroy_offscreen_noop() {
    let mut engine = NullEngine::new();
    engine.destroy_offscreen(ImageHandle(42)); // 不应 panic
}

#[test]
fn null_engine_offscreen_canvas_none() {
    let mut engine = NullEngine::new();
    assert!(engine.offscreen_canvas(&ImageHandle(0)).is_none());
}

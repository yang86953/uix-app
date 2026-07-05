//! uix-graphics 栅格化扩展模块集成测试。
//! 覆盖 gradient、glyph、image、fill、stroke 模块级纯函数。

use uix::render::color::Color;
use uix::render::path::{FillRule, PathBuilder};
use uix::render::rasterizer::fill;
use uix::render::rasterizer::glyph;
use uix::render::rasterizer::gradient;
use uix::render::rasterizer::image;
use uix::render::rasterizer::stroke;
use uix::render::types::{GradientDirection, Radius};
use uix::platform::api::geometry::Rect;

// ════════════════════════════════════════════════════════════════════════════
// gradient 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn fill_linear_gradient_horizontal() {
    let mut pixels = vec![0u32; 400]; // 20x20
    let clip = Rect::new(0.0, 0.0, 20.0, 20.0);
    gradient::fill_linear_gradient(
        &mut pixels,
        20,
        20,
        clip,
        1.0,
        Rect::new(0.0, 0.0, 20.0, 20.0),
        Color::from_rgba(0, 0, 0, 255),
        Color::from_rgba(255, 255, 255, 255),
        GradientDirection::Horizontal,
    );
    // 左侧接近黑色，右侧接近白色
    let left = pixels[0 * 20 + 0] & 0x00FFFFFF;
    let right = pixels[0 * 20 + 19] & 0x00FFFFFF;
    assert!(
        left < right,
        "左侧应比右侧暗，left={left:#X}, right={right:#X}"
    );
}

#[test]
fn fill_linear_gradient_vertical() {
    let mut pixels = vec![0u32; 400]; // 20x20
    let clip = Rect::new(0.0, 0.0, 20.0, 20.0);
    gradient::fill_linear_gradient(
        &mut pixels,
        20,
        20,
        clip,
        1.0,
        Rect::new(0.0, 0.0, 20.0, 20.0),
        Color::from_rgba(255, 0, 0, 255),
        Color::from_rgba(0, 0, 255, 255),
        GradientDirection::Vertical,
    );
    // ABGR: B at bits 0-7, R at bits 16-23
    let top_r = (pixels[0] >> 16) & 0xFF;
    let top_b = pixels[0] & 0xFF;
    let bot_r = (pixels[19 * 20 + 0] >> 16) & 0xFF;
    let bot_b = pixels[19 * 20 + 0] & 0xFF;
    assert!(top_r > bot_r, "顶部红色分量应大于底部");
    assert!(bot_b > top_b, "底部蓝色分量应大于顶部");
}

#[test]
fn fill_linear_gradient_respects_clip() {
    let mut pixels = vec![0u32; 100]; // 10x10
                                      // clip 只覆盖右下 5x5
    let clip = Rect::new(5.0, 5.0, 5.0, 5.0);
    gradient::fill_linear_gradient(
        &mut pixels,
        10,
        10,
        clip,
        1.0,
        Rect::new(0.0, 0.0, 10.0, 10.0),
        Color::from_rgba(255, 0, 0, 255),
        Color::from_rgba(0, 0, 255, 255),
        GradientDirection::Horizontal,
    );
    // 裁剪区域外的像素应为透明
    assert_eq!(pixels[0], 0x00000000, "左上角应在裁剪区外");
    // 裁剪区域内的像素应有颜色
    assert_ne!(pixels[5 * 10 + 5], 0x00000000, "右下角应在裁剪区内");
}

#[test]
fn fill_radial_gradient_basic() {
    let mut pixels = vec![0u32; 400]; // 20x20
    let clip = Rect::new(0.0, 0.0, 20.0, 20.0);
    gradient::fill_radial_gradient(
        &mut pixels,
        20,
        20,
        clip,
        1.0,
        10.0,
        10.0,
        0.0,
        8.0,
        Color::from_rgba(255, 255, 255, 255),
        Color::from_rgba(0, 0, 0, 255),
    );
    // 圆心附近比远离圆心亮
    let center = pixels[10 * 20 + 10] & 0x00FFFFFF;
    let corner = pixels[0] & 0x00FFFFFF;
    assert!(
        center > corner,
        "圆心应比角落亮，center={center:#X}, corner={corner:#X}"
    );
}

#[test]
fn fill_radial_gradient_inner_radius() {
    let mut pixels = vec![0u32; 400]; // 20x20
    let clip = Rect::new(0.0, 0.0, 20.0, 20.0);
    gradient::fill_radial_gradient(
        &mut pixels,
        20,
        20,
        clip,
        1.0,
        10.0,
        10.0,
        5.0,
        8.0,
        Color::from_rgba(255, 0, 0, 255),
        Color::from_rgba(0, 0, 255, 255),
    );
    // inner_r 内应为红色
    let center = pixels[10 * 20 + 10];
    assert_eq!(center & 0x0000FF, 0x00, "圆心附近红色分量为0");
    // 实际上 inner_r 内填 inner_color (red)，所以 R 应高
    let center_r = (center >> 16) & 0xFF;
    assert!(center_r > 200, "圆心附近红色应饱和");
}

// ════════════════════════════════════════════════════════════════════════════
// glyph 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn blit_glyph_basic() {
    let mut pixels = vec![0u32; 100]; // 10x10
                                      // 2x2 coverage（每个像素 255 = 完全不透明）
    let coverage = vec![255u8, 255, 255, 255];
    let clip = Rect::new(0.0, 0.0, 10.0, 10.0);
    glyph::blit_glyph(
        &mut pixels,
        10,
        10,
        clip,
        1.0,
        0,
        0,
        &coverage,
        2,
        2,
        Color::from_rgba(255, 255, 255, 255),
    );
    // 2x2 区域应为白色
    assert_eq!(pixels[0], 0xFFFFFFFF);
    assert_eq!(pixels[1], 0xFFFFFFFF);
    assert_eq!(pixels[10], 0xFFFFFFFF);
    assert_eq!(pixels[11], 0xFFFFFFFF);
    // 外部应为透明
    assert_eq!(pixels[2], 0x00000000);
}

#[test]
fn blit_glyph_zero_opacity_skips() {
    let mut pixels = vec![0xFFFFFFFF; 100];
    let coverage = vec![255u8; 4];
    let clip = Rect::new(0.0, 0.0, 10.0, 10.0);
    glyph::blit_glyph(
        &mut pixels,
        10,
        10,
        clip,
        0.0,
        0,
        0,
        &coverage,
        2,
        2,
        Color::from_rgba(0, 0, 0, 255),
    );
    // opacity=0 → 不应修改像素
    assert_eq!(pixels[0], 0xFFFFFFFF);
}

#[test]
fn blit_glyph_partial_coverage() {
    let mut pixels = vec![0u32; 100]; // 10x10
    let coverage = vec![128u8, 0, 0, 128]; // 半透明角落
    let clip = Rect::new(0.0, 0.0, 10.0, 10.0);
    glyph::blit_glyph(
        &mut pixels,
        10,
        10,
        clip,
        1.0,
        0,
        0,
        &coverage,
        2,
        2,
        Color::from_rgba(255, 255, 255, 255),
    );
    // 半透明像素的 alpha < 255
    let a0 = (pixels[0] >> 24) & 0xFF;
    assert!(a0 > 0 && a0 < 255, "半透明像素 alpha={a0} 应在 (0,255)");
    // 0-coverage 像素应为透明
    assert_eq!(pixels[1], 0x00000000);
}

// ════════════════════════════════════════════════════════════════════════════
// image 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn blit_image_identity() {
    let mut pixels = vec![0u32; 100]; // 10x10
    let src = vec![0xFFFFFFFF; 25]; // 5x5 白色图像
    let clip = Rect::new(0.0, 0.0, 10.0, 10.0);
    image::blit_image(
        &mut pixels,
        10,
        10,
        clip,
        1.0,
        &src,
        5,
        Rect::new(0.0, 0.0, 5.0, 5.0),
        Rect::new(0.0, 0.0, 5.0, 5.0),
    );
    assert_eq!(pixels[0], 0xFFFFFFFF);
    assert_eq!(pixels[5 * 10 + 5], 0x00000000); // 目标外透明
}

#[test]
fn blit_image_scaled() {
    let mut pixels = vec![0u32; 400]; // 20x20
    let src = vec![0xFF0000FF; 9]; // 3x3 红色图像
    let clip = Rect::new(0.0, 0.0, 20.0, 20.0);
    image::blit_image(
        &mut pixels,
        20,
        20,
        clip,
        1.0,
        &src,
        3,
        Rect::new(0.0, 0.0, 3.0, 3.0),
        Rect::new(0.0, 0.0, 6.0, 6.0), // 放大 2x
    );
    // 目标区域应有红色像素
    assert_eq!(pixels[0] & 0x0000FF, 0xFF); // B channel = 0xFF in ABGR
}

#[test]
fn blit_image_empty_src_noop() {
    let mut pixels = vec![0u32; 100];
    let clip = Rect::new(0.0, 0.0, 10.0, 10.0);
    image::blit_image(
        &mut pixels,
        10,
        10,
        clip,
        1.0,
        &[],
        0,
        Rect::new(0.0, 0.0, 0.0, 0.0),
        Rect::new(0.0, 0.0, 5.0, 5.0),
    );
    assert_eq!(pixels[0], 0x00000000);
}

#[test]
fn blit_image_opacity() {
    let mut pixels = vec![0xFFFFFFFF; 100]; // 10x10 白色背景
    let src = vec![0xFF000000; 25]; // 5x5 黑色（B=0）
    let clip = Rect::new(0.0, 0.0, 10.0, 10.0);
    image::blit_image(
        &mut pixels,
        10,
        10,
        clip,
        0.5,
        &src,
        5,
        Rect::new(0.0, 0.0, 5.0, 5.0),
        Rect::new(0.0, 0.0, 5.0, 5.0),
    );
    // 半透明黑色叠加到白色 → 灰色
    let pixel = pixels[0];
    let r = (pixel >> 16) & 0xFF;
    let g = (pixel >> 8) & 0xFF;
    let b = pixel & 0xFF;
    assert!(r < 255 && r > 0, "半透明叠加应产生中间色，r={r}");
    assert_eq!(r, g, "灰色 R=G");
    assert_eq!(r, b, "灰色 R=B");
}

// ════════════════════════════════════════════════════════════════════════════
// fill 模块级函数测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn fill_rect_module() {
    let mut pixels = vec![0u32; 100]; // 10x10
    let clip = Rect::new(0.0, 0.0, 10.0, 10.0);
    fill::fill_rect(
        &mut pixels,
        10,
        10,
        clip,
        1.0,
        Rect::new(0.0, 0.0, 5.0, 5.0),
        Color::from_rgba(0, 255, 0, 255),
        None,
    );
    assert_eq!(pixels[0], 0xFF00FF00); // ABGR: A=255, B=0, G=255, R=0
    assert_eq!(pixels[5 * 10 + 5], 0x00000000);
}

#[test]
fn fill_rect_module_rounded() {
    let mut pixels = vec![0u32; 100]; // 10x10
    let clip = Rect::new(0.0, 0.0, 10.0, 10.0);
    fill::fill_rect(
        &mut pixels,
        10,
        10,
        clip,
        1.0,
        Rect::new(0.0, 0.0, 5.0, 5.0),
        Color::from_rgba(0, 0, 255, 255),
        Some(Radius::uniform(2.0)),
    );
    // 圆角填充，内部像素应有颜色
    assert_ne!(pixels[2 * 10 + 2], 0x00000000);
}

#[test]
fn fill_rect_module_opacity() {
    let mut pixels = vec![0u32; 100];
    let clip = Rect::new(0.0, 0.0, 10.0, 10.0);
    fill::fill_rect(
        &mut pixels,
        10,
        10,
        clip,
        0.5,
        Rect::new(0.0, 0.0, 5.0, 5.0),
        Color::from_rgba(255, 0, 0, 255),
        None,
    );
    let a = (pixels[0] >> 24) & 0xFF;
    assert!(a > 0 && a < 255, "半透明度 alpha={a} 应在 (0,255)");
}

#[test]
fn fill_circle_module() {
    let mut pixels = vec![0u32; 400]; // 20x20
    let clip = Rect::new(0.0, 0.0, 20.0, 20.0);
    fill::fill_circle(
        &mut pixels,
        20,
        20,
        clip,
        1.0,
        10.0,
        10.0,
        5.0,
        Color::from_rgba(255, 0, 0, 255),
    );
    // fill 模块使用 color_to_premul 格式 AARRGGBB
    // 红色 (255,0,0,255) → 0xFFFF0000
    let pixel = pixels[10 * 20 + 10];
    let r = (pixel >> 16) & 0xFF;
    assert_eq!(r, 0xFF, "圆心红色分量应为 0xFF，得到 {r}");
}

#[test]
fn fill_ellipse_module() {
    let mut pixels = vec![0u32; 100]; // 10x10
    let clip = Rect::new(0.0, 0.0, 10.0, 10.0);
    fill::fill_ellipse(
        &mut pixels,
        10,
        10,
        clip,
        1.0,
        Rect::new(1.0, 2.0, 8.0, 6.0),
        Color::from_rgba(0, 255, 0, 255),
    );
    // 椭圆中心区域应有绿色
    assert_ne!(pixels[5 * 10 + 5], 0x00000000);
}

#[test]
fn fill_path_module() {
    let mut pixels = vec![0u32; 400]; // 20x20
    let path = PathBuilder::new()
        .move_to(5.0, 5.0)
        .line_to(15.0, 5.0)
        .line_to(10.0, 15.0)
        .close()
        .build();
    let clip = Rect::new(0.0, 0.0, 20.0, 20.0);
    fill::fill_path(
        &mut pixels,
        20,
        20,
        clip,
        1.0,
        &path,
        Color::from_rgba(255, 255, 255, 255),
        FillRule::NonZero,
    );
    // 三角形内部应有白色像素
    assert_eq!(pixels[10 * 20 + 10], 0xFFFFFFFF);
}

// ════════════════════════════════════════════════════════════════════════════
// fill_sector 模块级函数测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn fill_sector_module_basic() {
    let mut pixels = vec![0u32; 400]; // 20x20
    let clip = Rect::new(0.0, 0.0, 20.0, 20.0);
    // 半圆（0 到 π），圆心在 (10,10)，半径 5
    // 在屏幕坐标系中 y 向下增长，atan2(dy>0) 返回 0~π，
    // 因此 0~π 扇形覆盖圆的下半部分
    fill::fill_sector(
        &mut pixels,
        20,
        20,
        clip,
        1.0,
        10.0,
        10.0,
        5.0,
        0.0,
        std::f32::consts::PI,
        Color::from_rgba(255, 0, 0, 255),
    );
    // 圆心（下半部分）应在扇形范围内
    let pixel = pixels[10 * 20 + 10];
    let r = (pixel >> 16) & 0xFF;
    assert_eq!(r, 0xFF, "圆心红色分量应为 0xFF，得到 {r}");
    // 圆心上方像素（角度 π~2π，不在 0~π 内）应为透明
    assert_eq!(
        pixels[6 * 20 + 10],
        0x00000000,
        "扇形外（圆心上方）应为透明"
    );
}

#[test]
fn fill_sector_module_zero_radius_noop() {
    let mut pixels = vec![0xFFFFFFFF; 100]; // 10x10
    let clip = Rect::new(0.0, 0.0, 10.0, 10.0);
    fill::fill_sector(
        &mut pixels,
        10,
        10,
        clip,
        1.0,
        5.0,
        5.0,
        0.0,
        0.0,
        std::f32::consts::PI,
        Color::from_rgba(255, 0, 0, 255),
    );
    // 半径为 0，不应修改任何像素
    assert_eq!(pixels[0], 0xFFFFFFFF);
}

#[test]
fn fill_sector_module_three_quarter_circle() {
    let mut pixels = vec![0u32; 400]; // 20x20
    let clip = Rect::new(0.0, 0.0, 20.0, 20.0);
    // 覆盖 3/4 圆（0 到 3π/2），即右→下→左，留出上方 π/2 到 π 区域
    // 注意不能用 TAU 作为终点：TAU.rem_euclid(TAU)=0 → sa=ea=0 匹配不到任何角度
    let three_quarter = std::f32::consts::PI * 1.5;
    fill::fill_sector(
        &mut pixels,
        20,
        20,
        clip,
        1.0,
        10.0,
        10.0,
        5.0,
        0.0,
        three_quarter,
        Color::from_rgba(0, 255, 0, 255),
    );
    // 圆心（在 0~3π/2 内）应为绿色
    let pixel = pixels[10 * 20 + 10];
    let g = (pixel >> 8) & 0xFF;
    assert!(g > 200, "扇形内圆心绿色分量应较高，得到 {g}");
}

// ════════════════════════════════════════════════════════════════════════════
// shadow 模块级函数测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn draw_box_shadow_module_basic() {
    let mut pixels = vec![0u32; 400]; // 20x20
    let clip = Rect::new(0.0, 0.0, 20.0, 20.0);
    // 在 (5,5) 位置，10x10 的盒子绘制阴影，偏移 (3,3)，模糊 2px
    uix::render::rasterizer::shadow::draw_box_shadow(
        &mut pixels,
        20,
        20,
        clip,
        1.0,
        Rect::new(5.0, 5.0, 10.0, 10.0),
        2.0,
        3.0,
        3.0,
        Color::from_rgba(0, 0, 0, 128),
        None,
    );
    // 阴影偏移后应在 (8,8) 附近有像素
    let pixel = pixels[8 * 20 + 8];
    let a = (pixel >> 24) & 0xFF;
    assert!(a > 0, "阴影区域应有非零 alpha，得到 {a}");
}

#[test]
fn draw_box_shadow_module_zero_blur() {
    let mut pixels = vec![0u32; 400]; // 20x20
    let clip = Rect::new(0.0, 0.0, 20.0, 20.0);
    // blur_radius=0 时退化为清晰投影
    uix::render::rasterizer::shadow::draw_box_shadow(
        &mut pixels,
        20,
        20,
        clip,
        1.0,
        Rect::new(5.0, 5.0, 10.0, 10.0),
        0.0,
        3.0,
        3.0,
        Color::from_rgba(0, 0, 0, 255),
        None,
    );
    // 偏移后的阴影区域应有像素
    assert_ne!(pixels[8 * 20 + 8], 0x00000000);
    // 偏移前的盒子位置（原始 rect）应为透明
    assert_eq!(pixels[5 * 20 + 5], 0x00000000);
}

#[test]
fn draw_box_shadow_ambient_module_basic() {
    let mut pixels = vec![0u32; 400]; // 20x20
    let clip = Rect::new(0.0, 0.0, 20.0, 20.0);
    uix::render::rasterizer::shadow::draw_box_shadow_ambient(
        &mut pixels,
        20,
        20,
        clip,
        1.0,
        Rect::new(5.0, 5.0, 10.0, 10.0),
        3.0,
        0.0,
        0.0,
        Color::from_rgba(0, 0, 0, 128),
        None,
    );
    // 环境阴影围绕盒子扩散，盒子内部或附近应有像素
    let pixel = pixels[8 * 20 + 8];
    let a = (pixel >> 24) & 0xFF;
    assert!(a > 0, "环境阴影区域应有非零 alpha，得到 {a}");
}

#[test]
fn draw_box_shadow_transparent_color_noop() {
    let mut pixels = vec![0xFFFFFFFF; 100]; // 10x10
    let clip = Rect::new(0.0, 0.0, 10.0, 10.0);
    uix::render::rasterizer::shadow::draw_box_shadow(
        &mut pixels,
        10,
        10,
        clip,
        1.0,
        Rect::new(0.0, 0.0, 5.0, 5.0),
        2.0,
        1.0,
        1.0,
        Color::from_rgba(0, 0, 0, 0),
        None,
    );
    // 颜色 alpha=0，不应修改任何像素
    assert_eq!(pixels[0], 0xFFFFFFFF);
}

// ════════════════════════════════════════════════════════════════════════════
// stroke 模块级函数测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn stroke_rect_module() {
    let mut pixels = vec![0u32; 100]; // 10x10
    let clip = Rect::new(0.0, 0.0, 10.0, 10.0);
    stroke::stroke_rect(
        &mut pixels,
        10,
        10,
        clip,
        1.0,
        Rect::new(0.0, 0.0, 10.0, 10.0),
        Color::from_rgba(255, 255, 255, 255),
        1.0,
        None,
    );
    // 边框应有白色
    assert_eq!(pixels[0], 0xFFFFFFFF);
    // 内部应为透明（1px 边框）
    assert_eq!(pixels[1 * 10 + 1], 0x00000000);
}

#[test]
fn stroke_circle_module() {
    let mut pixels = vec![0u32; 400]; // 20x20
    let clip = Rect::new(0.0, 0.0, 20.0, 20.0);
    stroke::stroke_circle(
        &mut pixels,
        20,
        20,
        clip,
        1.0,
        10.0,
        10.0,
        5.0,
        Color::from_rgba(255, 0, 0, 255),
        1.0,
    );
    // 圆环上应有红色
    assert_ne!(pixels[10 * 20 + 15], 0x00000000);
    // 圆心应为透明（空心）
    assert_eq!(pixels[10 * 20 + 10], 0x00000000);
}

#[test]
fn draw_line_module() {
    let mut pixels = vec![0u32; 100]; // 10x10
    let clip = Rect::new(0.0, 0.0, 10.0, 10.0);
    stroke::draw_line(
        &mut pixels,
        10,
        10,
        clip,
        1.0,
        0.0,
        5.0,
        9.0,
        5.0,
        Color::from_rgba(255, 255, 255, 255),
        1.0,
    );
    // 水平线 y=5, lw=1: fill_rect_raw 从 y=4.5 开始 → y0=4
    // 白色 ABGR = 0xFFFFFFFF
    // 检查线上方和下方像素, 至少有一个白色
    let upper = pixels[4 * 10 + 5];
    let lower = pixels[5 * 10 + 5];
    assert!(
        upper == 0xFFFFFFFF || lower == 0xFFFFFFFF,
        "水平线上应至少有一个白色像素"
    );
}

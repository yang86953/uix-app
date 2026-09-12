//! Shared rasterizer helpers used by the software renderer and raster modules.

use crate::core::Rect;
use crate::draw::geometry::color::Color;
use crate::draw::geometry::types::Radius;

#[inline]
pub(crate) fn premul(c: u32) -> u32 {
    let a = (c >> 24) & 0xFF;
    if a == 0xFF {
        return c;
    }
    let r = (((c >> 16) & 0xFF) * a / 255).min(255);
    let g = (((c >> 8) & 0xFF) * a / 255).min(255);
    let b = ((c & 0xFF) * a / 255).min(255);
    (a << 24) | (r << 16) | (g << 8) | b
}

/// CSS 式圆角半径规范化的唯一共享规则：非有限或负半径收敛为 0；相邻角
/// 半径之和超过对应边长时按最大容许比例统一缩小，保持圆角轮廓不相交。
///
/// CPU SDF 填充/描边/阴影与 GPU path tessellation、shape/shadow lowering
/// 都在各自工作空间按此规则做每 draw call 一次的规范化，超半径输入在
/// 两个媒体产生同一轮廓。
pub(crate) fn normalize_corner_radii(
    width: f32,
    height: f32,
    tl: f32,
    tr: f32,
    br: f32,
    bl: f32,
) -> (f32, f32, f32, f32) {
    // 异常半径先收敛，避免非有限值污染缩放比例。
    let finite = |value: f32| {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    };
    let tl = finite(tl);
    let tr = finite(tr);
    let br = finite(br);
    let bl = finite(bl);
    // 按相邻边总长统一缩放，保持不相交的圆角轮廓。
    let mut factor = 1.0_f32;
    for (sum, extent) in [
        (tl + tr, width),
        (bl + br, width),
        (tl + bl, height),
        (tr + br, height),
    ] {
        if sum > extent && sum > 0.0 && extent.is_finite() {
            factor = factor.min((extent / sum).max(0.0));
        }
    }
    (tl * factor, tr * factor, br * factor, bl * factor)
}

/// 对 [`Radius`] 应用 CSS 式规范化，返回不会相交的圆角半径。
pub(crate) fn normalize_corner_radius(width: f32, height: f32, radius: Radius) -> Radius {
    let (tl, tr, br, bl) =
        normalize_corner_radii(width, height, radius.tl, radius.tr, radius.br, radius.bl);
    Radius { tl, tr, br, bl }
}

#[inline]
pub(crate) fn blend_srcover(
    src_a: u32,
    dst_a: u32,
    src_r: u32,
    src_g: u32,
    src_b: u32,
    dst_r: u32,
    dst_g: u32,
    dst_b: u32,
) -> u32 {
    let out_a = src_a + dst_a - (src_a * dst_a / 255);
    if out_a == 0 {
        return 0;
    }
    let out_r = src_r + (dst_r * (255 - src_a) / 255);
    let out_g = src_g + (dst_g * (255 - src_a) / 255);
    let out_b = src_b + (dst_b * (255 - src_a) / 255);
    (out_a.min(255) << 24) | (out_r.min(255) << 16) | (out_g.min(255) << 8) | out_b.min(255)
}

#[inline]
pub(crate) fn apply_opacity(color: u32, opacity: f32) -> u32 {
    if opacity >= 1.0 - 1e-6 {
        return color;
    }
    let a = ((color >> 24) & 0xFF) as f32 * opacity;
    let r = ((color >> 16) & 0xFF) as f32 * opacity;
    let g = ((color >> 8) & 0xFF) as f32 * opacity;
    let b = (color & 0xFF) as f32 * opacity;
    ((a as u32).min(255) << 24)
        | ((r as u32).min(255) << 16)
        | ((g as u32).min(255) << 8)
        | (b as u32).min(255)
}

/// Encodes the CPU fill path's premultiplied opacity result back into a
/// straight-alpha color whose [`Color::premultiplied`] value is bit-exact.
#[inline]
pub(crate) fn color_with_premultiplied_opacity(color: Color, opacity: f32) -> Color {
    if opacity >= 1.0 - 1e-6 {
        return color;
    }
    let premultiplied = apply_opacity(color.premultiplied(), opacity);
    let alpha = (premultiplied >> 24) & 0xff;
    if alpha == 0 {
        return Color::transparent();
    }
    let to_straight = |channel: u32| (channel * 255).div_ceil(alpha).min(255) as u8;
    Color::from_rgba(
        to_straight((premultiplied >> 16) & 0xff),
        to_straight((premultiplied >> 8) & 0xff),
        to_straight(premultiplied & 0xff),
        alpha as u8,
    )
}

/// Folds Canvas opacity into a glyph's straight alpha before premultiplication.
///
/// Glyph rasterization intentionally quantizes `color.a * opacity` first and
/// only then applies premultiplication and coverage. Keeping that order avoids
/// the one-bit drift produced by scaling an already-premultiplied color.
#[inline]
pub(crate) fn color_with_glyph_opacity(color: Color, opacity: f32) -> Color {
    Color::from_rgba(color.r, color.g, color.b, (color.a as f32 * opacity) as u8)
}

#[inline]
pub(crate) fn color_to_premul(r: u8, g: u8, b: u8, a: u8, opacity: f32) -> u32 {
    let a = (a as f32 * opacity) as u8;
    let ra = a as u32;
    if ra == 0 {
        return 0;
    }
    let r = (r as u32 * ra / 255).min(255);
    let g = (g as u32 * ra / 255).min(255);
    let b = (b as u32 * ra / 255).min(255);
    (ra << 24) | (r << 16) | (g << 8) | b
}

/// 按 coverage 调制预乘颜色——参考执行与 SharedRasterizer 字形 blit 的
/// 唯一共享量化规则，保持旧字形路径的整数向下取整。
#[inline]
pub(crate) fn modulate_coverage(color: u32, coverage: u8) -> u32 {
    // 把单字节 coverage 提升到通道乘法宽度。
    let factor = coverage as u32;
    // 每个通道独立执行乘法再除以 255。
    let channel = |shift: u32| ((color >> shift) & 0xff) * factor / 255;
    // 重新组合预乘 AARRGGBB 像素。
    (channel(24) << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

#[inline]
fn pixel_index(pixels_len: usize, stride: i32, x: i32, y: i32) -> Option<usize> {
    if stride <= 0 || x < 0 || y < 0 {
        return None;
    }
    (y as usize)
        .checked_mul(stride as usize)?
        .checked_add(x as usize)
        .filter(|index| *index < pixels_len)
}

#[inline]
pub(crate) fn put_pixel(
    pixels: &mut [u32],
    stride: i32,
    x: i32,
    y: i32,
    clip_x0: i32,
    clip_y0: i32,
    clip_x1: i32,
    clip_y1: i32,
    color: u32,
) {
    if x < clip_x0 || y < clip_y0 || x >= clip_x1 || y >= clip_y1 {
        return;
    }
    let Some(idx) = pixel_index(pixels.len(), stride, x, y) else {
        return;
    };
    let src_a = (color >> 24) & 0xFF;
    if src_a == 0 {
        return;
    }
    let dst = pixels[idx];
    let dst_a = (dst >> 24) & 0xFF;
    if src_a == 0xFF && dst_a == 0 {
        pixels[idx] = color;
        return;
    }
    let out = blend_srcover(
        src_a,
        dst_a,
        (color >> 16) & 0xFF,
        (color >> 8) & 0xFF,
        color & 0xFF,
        (dst >> 16) & 0xFF,
        (dst >> 8) & 0xFF,
        dst & 0xFF,
    );
    pixels[idx] = out;
}

#[inline]
pub(crate) fn fill_span(
    pixels: &mut [u32],
    stride: i32,
    x0: i32,
    x1: i32,
    y: i32,
    clip_x0: i32,
    clip_y0: i32,
    clip_x1: i32,
    clip_y1: i32,
    color: u32,
) {
    let x_start = x0.max(clip_x0).max(0);
    let x_end = x1.min(clip_x1).min(stride);
    if y < clip_y0 || y >= clip_y1 || y < 0 || x_start >= x_end {
        return;
    }
    if (color >> 24) == 0xFF {
        let Some(start) = pixel_index(pixels.len(), stride, x_start, y) else {
            return;
        };
        let len = (x_end - x_start) as usize;
        if let Some(end) = start.checked_add(len).filter(|end| *end <= pixels.len()) {
            pixels[start..end].fill(color);
        }
    } else {
        for x in x_start..x_end {
            put_pixel(
                pixels, stride, x, y, clip_x0, clip_y0, clip_x1, clip_y1, color,
            );
        }
    }
}

#[inline]
pub(crate) fn clip_to_int(r: &Rect) -> (i32, i32, i32, i32) {
    (
        (r.x + 0.5).floor() as i32,
        (r.y + 0.5).floor() as i32,
        (r.x + r.w + 0.5).floor() as i32,
        (r.y + r.h + 0.5).floor() as i32,
    )
}

#[inline]
pub(crate) fn intersect_rect(a: &Rect, b: &Rect) -> Option<Rect> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let rgt = (a.x + a.w).min(b.x + b.w);
    let bot = (a.y + a.h).min(b.y + b.h);
    if x < rgt && y < bot {
        Some(Rect::new(x, y, rgt - x, bot - y))
    } else {
        None
    }
}

/// 圆角矩形对齐物理像素网格：亚像素坐标下 SDF 弧线端点与像素中心错位，
/// 导致四角取整不对称（顶/底圆角视觉半径不一致）。按左右/上下边界分别
/// 就近取整，保证每侧偏移 < 0.5px 且宽度守恒；尺寸对齐后非正则返回 `None`。
#[inline]
pub(crate) fn align_rounded_rect(rect: Rect) -> Option<Rect> {
    let x0 = rect.x.round();
    let y0 = rect.y.round();
    let x1 = (rect.x + rect.w).round();
    let y1 = (rect.y + rect.h).round();
    let w = x1 - x0;
    let h = y1 - y0;
    if w <= 0.0 || h <= 0.0 {
        None
    } else {
        Some(Rect::new(x0, y0, w, h))
    }
}

#[inline]
pub(crate) fn rounded_rect_sdf(ux: f32, uy: f32, r: &Rect, rad: &Radius) -> f32 {
    // 直角矩形共享直边分支。
    if rad.tl == 0.0 && rad.tr == 0.0 && rad.bl == 0.0 && rad.br == 0.0 {
        let dx = (r.x - ux).max(ux - (r.x + r.w)).max(0.0);
        let dy = (r.y - uy).max(uy - (r.y + r.h)).max(0.0);
        let outside = (dx * dx + dy * dy).sqrt();
        let inside = (r.x - ux)
            .max(ux - (r.x + r.w))
            .max((r.y - uy).max(uy - (r.y + r.h)));
        return if inside < 0.0 { inside } else { outside };
    }

    // 圆角轮廓是直角矩形与各角圆弧约束的交集：对每个采样点，先取直边
    // 矩形的距离，再对采样点落入切线方形的每个角取"圆外为正"的弧距离，
    // 以最大者作为合成有符号距离。归一化只保证相邻角切线方形不相交，
    // 对角方形可以重叠（如 100x100 的 tl=br=80），重叠区域必须同时满足
    // 两个角的弧约束——这正是形状定义，不是实现选择。
    let lx = ux - r.x;
    let ly = uy - r.y;
    let mut combined = {
        let dx = (r.x - ux).max(ux - (r.x + r.w)).max(0.0);
        let dy = (r.y - uy).max(uy - (r.y + r.h)).max(0.0);
        let outside = (dx * dx + dy * dy).sqrt();
        let inside = (r.x - ux)
            .max(ux - (r.x + r.w))
            .max((r.y - uy).max(uy - (r.y + r.h)));
        if inside < 0.0 { inside } else { outside }
    };
    if rad.tl > 0.0 && lx < rad.tl && ly < rad.tl {
        combined = combined.max((lx - rad.tl).hypot(ly - rad.tl) - rad.tl);
    }
    if rad.tr > 0.0 && lx > r.w - rad.tr && ly < rad.tr {
        combined = combined.max((lx - (r.w - rad.tr)).hypot(ly - rad.tr) - rad.tr);
    }
    if rad.br > 0.0 && lx > r.w - rad.br && ly > r.h - rad.br {
        combined = combined.max((lx - (r.w - rad.br)).hypot(ly - (r.h - rad.br)) - rad.br);
    }
    if rad.bl > 0.0 && lx < rad.bl && ly > r.h - rad.bl {
        combined = combined.max((lx - rad.bl).hypot(ly - (r.h - rad.bl)) - rad.bl);
    }
    combined
}

#[inline]
pub(crate) fn line_segment_sdf(ux: f32, uy: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let length_sq = dx * dx + dy * dy;
    if length_sq < 1e-12 {
        let dx0 = ux - x1;
        let dy0 = uy - y1;
        return (dx0 * dx0 + dy0 * dy0).sqrt();
    }
    let t = ((ux - x1) * dx + (uy - y1) * dy) / length_sq;
    let t = t.clamp(0.0, 1.0);
    let px = x1 + t * dx;
    let py = y1 + t * dy;
    ((ux - px).powi(2) + (uy - py).powi(2)).sqrt()
}

#[inline]
pub(crate) fn sdf_to_coverage(sd: f32) -> f32 {
    ((0.5 - sd) / (2.0 * 0.5)).clamp(0.0, 1.0)
}

#[inline]
pub(crate) fn sdf_to_coverage_aa(sd: f32, aa_half: f32) -> f32 {
    ((aa_half - sd) / (2.0 * aa_half)).clamp(0.0, 1.0)
}

#[inline]
pub(crate) fn shadow_coverage(sd: f32, blur: f32) -> f32 {
    let t = ((blur - sd) / (2.0 * blur)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub(crate) fn shadow_coverage_ambient(sd: f32, blur: f32) -> f32 {
    let half = blur * 0.5;
    let t = ((half - sd) / (blur + half)).clamp(0.0, 1.0);
    let t2 = t * t;
    t2 * t2 * (5.0 - 4.0 * t)
}

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../../tests-src/draw/raster/rasterizer/core_tests.rs"]
mod core_tests;

// 以下完整测试实现位于 tests-src，经模块级 include! 保持原私有作用域；
// 文件内各项自带 cfg(test)，生产构建展开为空。
#[cfg(test)]
include!("../../../../tests-src/draw/raster/rasterizer/core_test_fns.rs");

//! 从 TrueType 轮廓生成 GPU atlas 用边列表（距离 coverage / MSDF，不含 CPU 像素）。
//!
//! 坐标与 `ab_glyph::OutlinedGlyph::draw` 一致：相对 `px_bounds` 左上的本地像素空间，
//! Y 向下。填充规则为 NonZero（与 TrueType 一致）。
//! 数据布局：`[ax, ay, bx, by, …]` 折线边；槽位四周 fringe ≥ [`MSDF_RANGE`]，
//! 供缩放路径 MSDF 使用（过窄 fringe 会裁切距离场）。近 1:1 UI 字使用字体
//! 光栅器给出的真实面积覆盖率 R8。
//!

//! MSDF：Chlumsky 真边着色（角点切换 CMY 双通道色），编码为
//! `0.5 + sd / MSDF_RANGE`；采样时取 `median(r,g,b)` 再转 coverage，利于大字号缩放保角。

use std::sync::Arc;

use ab_glyph::{Outline, OutlineCurve, Point as AbPoint, PxScaleFactor, Rect as AbRect};

use crate::core::Point;
use crate::draw::geometry::flattener;
use crate::draw::geometry::path::PathBuilder;
use crate::draw::raster::rasterizer::core::sdf_to_coverage_aa;

/// 距离 coverage 的 AA 半宽（像素）；仅供兼容回退和测试使用。
const AA_HALF: f32 = 0.5;
/// 展平容差（像素）。
const FLATTEN_TOLERANCE: f32 = 0.25;
/// 单字形边数上限，防止异常轮廓撑爆 atlas / storage。
const MAX_EDGES: usize = 4096;
/// MSDF 距离编码半宽（像素）；与 GPU cover / sample 共用。
pub(crate) const MSDF_RANGE: f32 = 4.0;
/// atlas 槽位单边 fringe：至少容纳完整 MSDF 范围，并多 1px 供线性采样。
pub(crate) const ATLAS_PAD: usize = MSDF_RANGE as usize + 1;
/// 角点判定外角阈值（弧度），与 msdfgen `edgeColoringSimple` 示例一致（≈172°）。
const CORNER_ANGLE_THRESHOLD: f32 = 3.0;
/// 端点连接容差（像素）。
const ENDPOINT_EPS: f32 = 1e-3;

/// EdgeColor 位掩码（与 msdfgen 一致）：至少两通道开启。
pub(crate) const EDGE_RED: u8 = 1;
pub(crate) const EDGE_GREEN: u8 = 2;
pub(crate) const EDGE_BLUE: u8 = 4;
pub(crate) const EDGE_YELLOW: u8 = EDGE_RED | EDGE_GREEN;
pub(crate) const EDGE_MAGENTA: u8 = EDGE_RED | EDGE_BLUE;
pub(crate) const EDGE_CYAN: u8 = EDGE_GREEN | EDGE_BLUE;
pub(crate) const EDGE_WHITE: u8 = EDGE_RED | EDGE_GREEN | EDGE_BLUE;

type GlyphOutlineMesh = (usize, usize, f32, f32, Arc<[f32]>);

/// 轮廓 → NonZero 边列表；失败时返回 `None`（调用方回退 CPU coverage）。
pub(crate) fn mesh_from_outline(
    outline: &Outline,
    scale_factor: PxScaleFactor,
    px_bounds: AbRect,
    position: AbPoint,
) -> Option<GlyphOutlineMesh> {
    let inner_w = (px_bounds.max.x - px_bounds.min.x).ceil();
    let inner_h = (px_bounds.max.y - px_bounds.min.y).ceil();
    if !(inner_w.is_finite() && inner_h.is_finite()) || inner_w <= 0.0 || inner_h <= 0.0 {
        return None;
    }
    let fringe = ATLAS_PAD.saturating_mul(2);
    let width = (inner_w as usize).saturating_add(fringe);
    let height = (inner_h as usize).saturating_add(fringe);
    if width == 0 || height == 0 {
        return None;
    }
    let h_factor = scale_factor.horizontal;
    let v_factor = -scale_factor.vertical;
    let pad = ATLAS_PAD as f32;
    let offset_x = position.x - px_bounds.min.x + pad;
    let offset_y = position.y - px_bounds.min.y + pad;
    let map = |p: AbPoint| Point::new(p.x * h_factor + offset_x, p.y * v_factor + offset_y);

    let mut builder = PathBuilder::new();
    let mut pen: Option<Point> = None;
    let near = |a: Point, b: Point| (a.x - b.x).abs() <= 1e-3 && (a.y - b.y).abs() <= 1e-3;

    for curve in &outline.curves {
        match *curve {
            OutlineCurve::Line(p0, p1) => {
                let a = map(p0);
                let b = map(p1);
                if pen.map(|p| !near(p, a)).unwrap_or(true) {
                    if pen.is_some() {
                        builder.close();
                    }
                    builder.move_to(a.x, a.y);
                }
                builder.line_to(b.x, b.y);
                pen = Some(b);
            }
            OutlineCurve::Quad(p0, p1, p2) => {
                let a = map(p0);
                let c = map(p1);
                let b = map(p2);
                if pen.map(|p| !near(p, a)).unwrap_or(true) {
                    if pen.is_some() {
                        builder.close();
                    }
                    builder.move_to(a.x, a.y);
                }
                builder.quad_to(c.x, c.y, b.x, b.y);
                pen = Some(b);
            }
            OutlineCurve::Cubic(p0, p1, p2, p3) => {
                let a = map(p0);
                let c1 = map(p1);
                let c2 = map(p2);
                let b = map(p3);
                if pen.map(|p| !near(p, a)).unwrap_or(true) {
                    if pen.is_some() {
                        builder.close();
                    }
                    builder.move_to(a.x, a.y);
                }
                builder.cubic_to(c1.x, c1.y, c2.x, c2.y, b.x, b.y);
                pen = Some(b);
            }
        }
    }
    if pen.is_some() {
        builder.close();
    }

    let path = builder.build();
    if path.is_empty() {
        return None;
    }
    let edges = edges_from_path(&path)?;
    if edges.len() < 4 || !edges.len().is_multiple_of(4) || edges.len() / 4 > MAX_EDGES {
        return None;
    }
    Some((
        width,
        height,
        px_bounds.min.x - pad,
        px_bounds.min.y - pad,
        Arc::<[f32]>::from(edges),
    ))
}

/// 将展平后的闭合子路径转为边列表。
fn edges_from_path(path: &crate::draw::geometry::path::Path) -> Option<Vec<f32>> {
    let subpaths = flattener::flatten_subpaths(path.segments(), FLATTEN_TOLERANCE);
    if subpaths.is_empty() {
        return None;
    }
    let mut edges = Vec::new();
    for sub in subpaths {
        if !sub.closed || sub.points.len() < 2 {
            continue;
        }
        let points = &sub.points;
        let n = points.len();
        let last = points[n - 1];
        let first = points[0];
        let wrapped = (last.x - first.x).abs() > 1e-3 || (last.y - first.y).abs() > 1e-3;
        let seg_count = if wrapped { n } else { n - 1 };
        if seg_count == 0 {
            continue;
        }
        for i in 0..seg_count {
            let a = points[i];
            let b = if i + 1 < n { points[i + 1] } else { first };
            if (a.x - b.x).abs() <= 1e-6 && (a.y - b.y).abs() <= 1e-6 {
                continue;
            }
            edges.push(a.x);
            edges.push(a.y);
            edges.push(b.x);
            edges.push(b.y);
            if edges.len() / 4 > MAX_EDGES {
                return None;
            }
        }
    }
    if edges.len() < 4 {
        return None;
    }
    Some(edges)
}

/// 由展平边列表生成像素中心距离 coverage。
///
/// 这不是轮廓的真实面积覆盖率，只用于没有字体光栅结果的兼容回退；正常
/// 近 1:1 字形必须使用 `OutlinedGlyph::draw` 的 R8 coverage。
pub(crate) fn coverage_from_edges(edges: &[f32], width: usize, height: usize) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || edges.len() < 4 || !edges.len().is_multiple_of(4) {
        return None;
    }
    let pixel_count = width.checked_mul(height)?;
    let mut coverage = Vec::new();
    if coverage.try_reserve_exact(pixel_count).is_err() {
        return None;
    }
    coverage.resize(pixel_count, 0u8);
    for y in 0..height {
        for x in 0..width {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let (winding, dist) = sample_edges(edges, px, py);
            let sd = if winding != 0 { -dist } else { dist };
            let cov = sdf_to_coverage_aa(sd, AA_HALF);
            coverage[y * width + x] = (cov * 255.0).clamp(0.0, 255.0) as u8;
        }
    }
    Some(coverage)
}

/// 由边列表生成 RGBA MSDF（RGB=编码距离，A=255）；与 GPU `glyph_cover` 公式一致。
pub(crate) fn msdf_from_edges(edges: &[f32], width: usize, height: usize) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || edges.len() < 4 || !edges.len().is_multiple_of(4) {
        return None;
    }
    let colors = colorize_edges(edges)?;
    msdf_from_colored_edges(edges, &colors, width, height)
}

/// 使用已着色边生成 MSDF（GPU cover / 单测共用）。
pub(crate) fn msdf_from_colored_edges(
    edges: &[f32],
    colors: &[u8],
    width: usize,
    height: usize,
) -> Option<Vec<u8>> {
    let edge_count = edges.len() / 4;
    if width == 0
        || height == 0
        || edge_count == 0
        || !edges.len().is_multiple_of(4)
        || colors.len() < edge_count
    {
        return None;
    }
    let pixel_count = width.checked_mul(height)?;
    let byte_count = pixel_count.checked_mul(4)?;
    let mut rgba = Vec::new();
    if rgba.try_reserve_exact(byte_count).is_err() {
        return None;
    }
    rgba.resize(byte_count, 0u8);
    for y in 0..height {
        for x in 0..width {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let (winding, channel_dist, min_dist) = sample_edges_msdf(edges, colors, px, py);
            let sign = if winding != 0 { -1.0f32 } else { 1.0 };
            let fallback = if min_dist.is_finite() { min_dist } else { 0.0 };
            let mut rgb = [0u8; 3];
            for c in 0..3 {
                let d = if channel_dist[c].is_finite() {
                    channel_dist[c]
                } else {
                    fallback
                };
                let encoded = (0.5 + (sign * d) / MSDF_RANGE).clamp(0.0, 1.0);
                rgb[c] = (encoded * 255.0).round().clamp(0.0, 255.0) as u8;
            }
            let o = (y * width + x) * 4;
            rgba[o] = rgb[0];
            rgba[o + 1] = rgb[1];
            rgba[o + 2] = rgb[2];
            rgba[o + 3] = 255;
        }
    }
    Some(rgba)
}

/// MSDF 三通道 → coverage（median + 线性 AA）；供 soft / 单测对齐 GPU sample。
// 非测试构建下暂无生产调用（仅供测试对齐），保留实现并精确标注。
#[cfg_attr(not(test), allow(dead_code))]
#[inline]
pub(crate) fn msdf_encoded_to_coverage(r: f32, g: f32, b: f32) -> f32 {
    let m = median3(r, g, b);
    // 与 `sdf_to_coverage_aa` 在 range=MSDF_RANGE、AA_HALF=0.5 时等价：
    // sd = (m - 0.5) * RANGE；cov = clamp((0.5 - sd) / 1.0, 0, 1)
    ((0.5 - (m - 0.5) * MSDF_RANGE) / (2.0 * AA_HALF)).clamp(0.0, 1.0)
}

/// 将 MSDF RGBA 栅格转为 R8 coverage（soft 验证 / 调试）。
// 非测试构建下暂无生产调用（仅供测试验证），保留实现并精确标注。
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn coverage_from_msdf(msdf_rgba: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    let expected = width.checked_mul(height)?.checked_mul(4)?;
    if width == 0 || height == 0 || msdf_rgba.len() < expected {
        return None;
    }
    let mut coverage = Vec::new();
    if coverage.try_reserve_exact(width * height).is_err() {
        return None;
    }
    coverage.resize(width * height, 0u8);
    for (i, pixel) in coverage.iter_mut().enumerate() {
        let o = i * 4;
        let r = msdf_rgba[o] as f32 / 255.0;
        let g = msdf_rgba[o + 1] as f32 / 255.0;
        let b = msdf_rgba[o + 2] as f32 / 255.0;
        let cov = msdf_encoded_to_coverage(r, g, b);
        *pixel = (cov * 255.0).clamp(0.0, 255.0) as u8;
    }
    Some(coverage)
}

// 仅被测试使用的 msdf_encoded_to_coverage 链引用，非测试构建下同样精确标注。
#[cfg_attr(not(test), allow(dead_code))]
#[inline]
fn median3(a: f32, b: f32, c: f32) -> f32 {
    a.max(b).min(a.max(c)).min(b.max(c))
}

/// Chlumsky `edgeColoringSimple`：按角点在闭合子路径上切换 CMY 双通道色。
///
/// 返回与边一一对应的色掩码；不修改边几何（展平后 teardrop 少于 3 边极少，退化为可用着色）。
pub(crate) fn colorize_edges(edges: &[f32]) -> Option<Vec<u8>> {
    let n = edges.len() / 4;
    if n == 0 || !edges.len().is_multiple_of(4) {
        return None;
    }
    let mut colors = vec![EDGE_CYAN; n];
    let mut seed: u64 = 0;
    let mut color = init_color(&mut seed);
    let cross_threshold = CORNER_ANGLE_THRESHOLD.sin();
    for range in contour_ranges(edges) {
        if range.is_empty() {
            continue;
        }
        colorize_contour(
            edges,
            &range,
            &mut colors,
            &mut color,
            &mut seed,
            cross_threshold,
        );
    }
    Some(colors)
}

/// 连续端点相连的边划为一条闭合子路径（contour）。
fn contour_ranges(edges: &[f32]) -> Vec<std::ops::Range<usize>> {
    let n = edges.len() / 4;
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < n {
        let start = i;
        let mut cur = i;
        i += 1;
        while i < n {
            let (_ax, _ay, bx, by) = edge_ends(edges, cur);
            let (nx, ny, _bx, _by) = edge_ends(edges, i);
            if !near_pt(bx, by, nx, ny) {
                break;
            }
            cur = i;
            i += 1;
        }
        out.push(start..i);
    }
    out
}

fn colorize_contour(
    edges: &[f32],
    range: &std::ops::Range<usize>,
    colors: &mut [u8],
    color: &mut u8,
    seed: &mut u64,
    cross_threshold: f32,
) {
    let m = range.end - range.start;
    if m == 0 {
        return;
    }
    let mut corners = Vec::new();
    let mut prev_dir = edge_dir(edges, range.start + m - 1);
    for local in 0..m {
        let idx = range.start + local;
        let dir = edge_dir(edges, idx);
        if is_corner(prev_dir, dir, cross_threshold) {
            corners.push(local);
        }
        prev_dir = dir;
    }

    if corners.is_empty() {
        // 光滑闭合：整圈同色（双通道）。
        switch_color(color, seed, None);
        for local in 0..m {
            colors[range.start + local] = *color;
        }
        return;
    }

    if corners.len() == 1 {
        // Teardrop：三色对称分布；边数不足 3 时尽力分配（展平轮廓极少如此）。
        switch_color(color, seed, None);
        let c0 = *color;
        let c1 = EDGE_WHITE;
        switch_color(color, seed, None);
        let c2 = *color;
        let palette = [c0, c1, c2];
        let corner = corners[0];
        if m >= 3 {
            for i in 0..m {
                let slot = 1 + symmetrical_trichotomy(i, m);
                colors[range.start + (corner + i) % m] = palette[slot as usize];
            }
        } else {
            for i in 0..m {
                colors[range.start + (corner + i) % m] = palette[i.min(2)];
            }
        }
        return;
    }

    // 多角点：角点之间的 spline 切换颜色；末段避开与首色冲突。
    let corner_count = corners.len();
    let start = corners[0];
    switch_color(color, seed, None);
    let initial = *color;
    let mut spline = 0usize;
    for i in 0..m {
        let index = (start + i) % m;
        if spline + 1 < corner_count && corners[spline + 1] == index {
            spline += 1;
            let banned = if spline == corner_count - 1 {
                Some(initial)
            } else {
                None
            };
            switch_color(color, seed, banned);
        }
        colors[range.start + index] = *color;
    }
}

#[inline]
fn edge_ends(edges: &[f32], idx: usize) -> (f32, f32, f32, f32) {
    let o = idx * 4;
    (edges[o], edges[o + 1], edges[o + 2], edges[o + 3])
}

#[inline]
fn edge_dir(edges: &[f32], idx: usize) -> (f32, f32) {
    let (ax, ay, bx, by) = edge_ends(edges, idx);
    normalize2(bx - ax, by - ay)
}

#[inline]
fn near_pt(ax: f32, ay: f32, bx: f32, by: f32) -> bool {
    (ax - bx).abs() <= ENDPOINT_EPS && (ay - by).abs() <= ENDPOINT_EPS
}

#[inline]
fn normalize2(x: f32, y: f32) -> (f32, f32) {
    let len = (x * x + y * y).sqrt();
    if len <= 1e-12 {
        (0.0, 0.0)
    } else {
        (x / len, y / len)
    }
}

#[inline]
fn is_corner(a: (f32, f32), b: (f32, f32), cross_threshold: f32) -> bool {
    let dot = a.0 * b.0 + a.1 * b.1;
    let cross = a.0 * b.1 - a.1 * b.0;
    dot <= 0.0 || cross.abs() > cross_threshold
}

/// 位置相对 n 的三段划分：-1 / 0 / 1 → 映射到 palette 下标偏移。
fn symmetrical_trichotomy(position: usize, n: usize) -> i32 {
    if n <= 1 {
        return 0;
    }
    let t = 3.0 + 2.875 * (position as f32) / ((n - 1) as f32) - 1.4375 + 0.5;
    (t.floor() as i32) - 3
}

fn init_color(seed: &mut u64) -> u8 {
    const COLORS: [u8; 3] = [EDGE_CYAN, EDGE_MAGENTA, EDGE_YELLOW];
    COLORS[seed_extract3(seed)]
}

fn switch_color(color: &mut u8, seed: &mut u64, banned: Option<u8>) {
    if let Some(banned) = banned {
        let combined = *color & banned;
        if combined == EDGE_RED || combined == EDGE_GREEN || combined == EDGE_BLUE {
            *color = combined ^ EDGE_WHITE;
            return;
        }
    }
    let shifted = (*color as u32) << (1 + seed_extract2(seed));
    *color = ((shifted | (shifted >> 3)) & (EDGE_WHITE as u32)) as u8;
}

fn seed_extract2(seed: &mut u64) -> u32 {
    let v = (*seed & 1) as u32;
    *seed >>= 1;
    v
}

fn seed_extract3(seed: &mut u64) -> usize {
    let v = (*seed % 3) as usize;
    *seed /= 3;
    v
}

fn sample_edges(edges: &[f32], px: f32, py: f32) -> (i32, f32) {
    // 兼容距离 coverage 只需要全局最近距离，不走通道着色。
    let mut winding = 0i32;
    let mut min_dist = f32::INFINITY;
    let mut i = 0usize;
    while i + 3 < edges.len() {
        let ax = edges[i];
        let ay = edges[i + 1];
        let bx = edges[i + 2];
        let by = edges[i + 3];
        i += 4;
        min_dist = min_dist.min(point_segment_dist(px, py, ax, ay, bx, by));
        if (ay > py) == (by > py) {
            continue;
        }
        let dy = by - ay;
        if dy.abs() <= 1e-8 {
            continue;
        }
        let t = (py - ay) / dy;
        let x_int = ax + t * (bx - ax);
        if px < x_int {
            winding += if dy > 0.0 { 1 } else { -1 };
        }
    }
    if !min_dist.is_finite() {
        min_dist = 0.0;
    }
    (winding, min_dist)
}

fn sample_edges_msdf(edges: &[f32], colors: &[u8], px: f32, py: f32) -> (i32, [f32; 3], f32) {
    let mut winding = 0i32;
    let mut min_dist = f32::INFINITY;
    let mut channel_dist = [f32::INFINITY; 3];
    let mut i = 0usize;
    let mut edge_idx = 0usize;
    while i + 3 < edges.len() {
        let ax = edges[i];
        let ay = edges[i + 1];
        let bx = edges[i + 2];
        let by = edges[i + 3];
        i += 4;
        let d = point_segment_dist(px, py, ax, ay, bx, by);
        min_dist = min_dist.min(d);
        let mask = colors.get(edge_idx).copied().unwrap_or(EDGE_WHITE);
        edge_idx += 1;
        if mask & EDGE_RED != 0 {
            channel_dist[0] = channel_dist[0].min(d);
        }
        if mask & EDGE_GREEN != 0 {
            channel_dist[1] = channel_dist[1].min(d);
        }
        if mask & EDGE_BLUE != 0 {
            channel_dist[2] = channel_dist[2].min(d);
        }
        // NonZero：向 +X 射线的穿越累计。
        if (ay > py) == (by > py) {
            continue;
        }
        let dy = by - ay;
        if dy.abs() <= 1e-8 {
            continue;
        }
        let t = (py - ay) / dy;
        let x_int = ax + t * (bx - ax);
        if px < x_int {
            winding += if dy > 0.0 { 1 } else { -1 };
        }
    }
    if !min_dist.is_finite() {
        min_dist = 0.0;
    }
    (winding, channel_dist, min_dist)
}

fn point_segment_dist(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let abx = bx - ax;
    let aby = by - ay;
    let apx = px - ax;
    let apy = py - ay;
    let ab_len_sq = abx * abx + aby * aby;
    let t = if ab_len_sq <= 1e-12 {
        0.0
    } else {
        ((apx * abx + apy * aby) / ab_len_sq).clamp(0.0, 1.0)
    };
    let cx = ax + abx * t - px;
    let cy = ay + aby * t - py;
    (cx * cx + cy * cy).sqrt()
}

/// 判断是否为合法边列表（供 GPU / soft 入口共用）。
#[inline]
pub(crate) fn is_outline_edges(data: &[f32]) -> bool {
    data.len() >= 4 && data.len().is_multiple_of(4)
}

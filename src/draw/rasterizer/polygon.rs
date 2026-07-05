//! 扫描线栅格化器 — 将展平后的多边形填充到像素缓冲。
//!
//! 算法：
//! 1. 构建边的全局表（每条边：ymin, ymax, x_at_ymin, dxdy）
//! 2. 从上到下扫描每行，维护活跃边表（AET）
//! 3. 按 x 排序活跃边，配对填充
//! 4. 支持 NonZero 和 EvenOdd 填充规则

use crate::draw::primitives::path::FillRule;
use crate::native::{Point, Rect};

use super::fill_span;

/// 一条扫描线边。
#[derive(Debug, Clone, Copy)]
pub struct Edge {
    /// 当前扫描线的 x 坐标（随 y 递增更新）。
    x: f32,
    /// y 每增加 1 时 x 的变化量。
    dxdy: f32,
    /// 边的最大 y（扫描到此处移除）。
    ymax: f32,
    /// 缠绕数方向（1=顺时针, -1=逆时针）。
    winding: i32,
}

/// 从两个点构建一条边。
fn make_edge(p0: Point, p1: Point) -> Edge {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    let (top, bottom) = if p0.y <= p1.y { (p0, p1) } else { (p1, p0) };
    // 排除完全水平的边
    let dxdy = if dy.abs() > 0.0001 { dx / dy } else { 0.0 };
    // 缠绕方向：从上往下看，向右为正
    let winding = if p1.y > p0.y { 1 } else { -1 };

    Edge {
        x: top.x,
        dxdy,
        ymax: bottom.y,
        winding,
    }
}

/// 将展平后的多边形填充到像素缓冲中。
///
/// `polys`: 展平后的多边形列表（每个 Vec<Point> 为一个闭合多边形）。
/// `pixels`: BGRA 32bit 像素缓冲。
/// `width`, `height`: 缓冲尺寸。
/// `clip`: 裁剪矩形（像素坐标）。
/// `color`: 填充颜色（BGRA 32bit premultiplied）。
/// `fill_rule`: 填充规则。
pub fn fill_polygons(
    polys: &[Vec<Point>],
    pixels: &mut [u32],
    width: i32,
    height: i32,
    clip: Rect,
    color: u32,
    fill_rule: FillRule,
    global_edges: &mut Vec<(i32, Edge)>,
    active_edges: &mut Vec<Edge>,
) {
    if color & 0xFF000000 == 0 || polys.is_empty() {
        return;
    }

    let clip_x0 = clip.x.max(0.0) as i32;
    let clip_y0 = clip.y.max(0.0) as i32;
    let clip_x1 = (clip.x + clip.w).min(width as f32) as i32;
    let clip_y1 = (clip.y + clip.h).min(height as f32) as i32;
    if clip_x0 >= clip_x1 || clip_y0 >= clip_y1 {
        return;
    }

    // 1. 构建全局边表（GET）
    global_edges.clear();

    for poly in polys {
        if poly.len() < 2 {
            continue;
        }
        for i in 0..poly.len() - 1 {
            let p0 = poly[i];
            let p1 = poly[i + 1];
            // 跳过水平边（dy=0）
            if (p1.y - p0.y).abs() < 0.0001 {
                continue;
            }
            let edge = make_edge(p0, p1);
            let ymin = p0.y.min(p1.y).ceil() as i32;
            if ymin <= clip_y1 {
                global_edges.push((ymin, edge));
            }
        }
    }

    if global_edges.is_empty() {
        return;
    }

    // 按 ymin 排序
    global_edges.sort_by_key(|e| e.0);

    // global_edges 非空（上面 is_empty 已返回），直接索引安全
    let y_start = global_edges[0].0.max(clip_y0);
    let y_end = clip_y1;

    // 2. 逐行扫描
    active_edges.clear();
    let mut edge_idx = 0;

    for y in y_start..y_end {
        let yf = y as f32;

        // 添加新边到 AET
        while edge_idx < global_edges.len() && global_edges[edge_idx].0 <= y {
            let mut e = global_edges[edge_idx].1;
            // 将 x 调整到当前扫描线中心
            e.x += e.dxdy * (yf - (yf - 0.5).floor() - 0.5);
            active_edges.push(e);
            edge_idx += 1;
        }

        // 移除已完成的边
        active_edges.retain(|e| yf < e.ymax);

        if active_edges.is_empty() {
            continue;
        }

        // 按 x 坐标排序
        active_edges.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

        // 配对填充（NonZero / EvenOdd）
        let stride = width;
        let _row_offset = (y * stride) as usize;

        match fill_rule {
            FillRule::NonZero => {
                let mut i = 0;
                while i + 1 < active_edges.len() {
                    let mut winding = active_edges[i].winding;
                    let span_start = active_edges[i].x.max(clip_x0 as f32) as i32;
                    let mut j = i + 1;
                    while j < active_edges.len() {
                        winding += active_edges[j].winding;
                        if winding == 0 {
                            let span_end = (active_edges[j].x).ceil() as i32;
                            let sx = span_start.max(clip_x0);
                            let ex = span_end.min(clip_x1);
                            if sx < ex {
                                fill_span(
                                    pixels, stride, sx, ex, y, clip_x0, clip_y0, clip_x1, clip_y1,
                                    color,
                                );
                            }
                            i = j + 1;
                            break;
                        }
                        j += 1;
                    }
                    if j >= active_edges.len() {
                        break;
                    }
                }
            }
            FillRule::EvenOdd => {
                let mut i = 0;
                while i + 1 < active_edges.len() {
                    let span_start = active_edges[i].x.max(clip_x0 as f32) as i32;
                    let span_end = (active_edges[i + 1].x).ceil() as i32;
                    let sx = span_start.max(clip_x0);
                    let ex = span_end.min(clip_x1);
                    if sx < ex {
                        fill_span(
                            pixels, stride, sx, ex, y, clip_x0, clip_y0, clip_x1, clip_y1, color,
                        );
                    }
                    i += 2;
                }
            }
        }

        // 更新每条边的 x 为下一扫描线做准备
        for e in active_edges.iter_mut() {
            e.x += e.dxdy;
        }
    }
}

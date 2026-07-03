//! 路径描边器 — 将路径描边转换为填充多边形。
//!
//! 算法：沿路径两侧偏移半个线宽，生成闭合填充区域。
//! 支持 LineCap::Butt/Round/Square、LineJoin::Miter/Bevel。

use super::flattener;
use super::path::{LineCap, LineJoin, Path, PathBuilder, PathSegment};
use uix_platform::Point;

/// 描边参数。
#[derive(Debug, Clone, Copy)]
pub struct StrokeOptions {
    pub width: f32,
    pub cap: LineCap,
    pub join: LineJoin,
    pub miter_limit: f32,
}

impl Default for StrokeOptions {
    fn default() -> Self {
        Self {
            width: 1.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter_limit: 4.0,
        }
    }
}

/// 将路径描边转换为填充路径（供 rasterizer 填充）。
///
/// 返回的 Path 可再用 rasterizer 填充。
pub fn stroke_path(path: &Path, options: &StrokeOptions) -> Path {
    let half_w = options.width * 0.5;
    if half_w < 0.0001 {
        return Path {
            segments: Vec::new(),
        };
    }

    // 展平路径为线段
    let polys = flattener::flatten(path.segments(), 0.25);
    if polys.is_empty() {
        return Path {
            segments: Vec::new(),
        };
    }

    let mut builder = PathBuilder::new();
    let mut first = true;

    for poly in &polys {
        if poly.len() < 2 {
            continue;
        }

        if first {
            first = false;
            // 左侧偏移轮廓
            build_offset_outline(&mut builder, poly, half_w, options, true);
            // 右侧偏移轮廓（反向）
            let reversed: Vec<Point> = poly.iter().rev().copied().collect();
            build_offset_outline(&mut builder, &reversed, half_w, options, false);
            // 闭合
            builder.close();
        } else {
            // 多个子路径
            builder.move_to(poly[0].x, poly[0].y);
            let mut inner_builder = PathBuilder::new();
            build_offset_outline(&mut inner_builder, poly, half_w, options, true);
            let reversed: Vec<Point> = poly.iter().rev().copied().collect();
            build_offset_outline(&mut inner_builder, &reversed, half_w, options, false);
            inner_builder.close();
            let inner = inner_builder.build();
            for seg in inner.segments() {
                match *seg {
                    PathSegment::MoveTo(p) => {
                        builder.move_to(p.x, p.y);
                    }
                    PathSegment::LineTo(p) => {
                        builder.line_to(p.x, p.y);
                    }
                    _ => {}
                }
            }
        }
    }

    builder.build()
}

/// 沿多边形构建偏移轮廓（一侧）。
fn build_offset_outline(
    builder: &mut PathBuilder,
    poly: &[Point],
    half_w: f32,
    options: &StrokeOptions,
    is_left: bool,
) {
    if poly.is_empty() {
        return;
    }

    let dir: f32 = if is_left { 1.0 } else { -1.0 };

    // 第一个点前处理 cap
    if poly.len() >= 2 {
        let p0 = poly[0];
        let p1 = poly[1];
        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;
        let len = (dx * dx + dy * dy).sqrt().max(0.0001);
        let nx = -dy / len * dir;
        let ny = dx / len * dir;

        add_cap(builder, p0, nx, ny, half_w, options.cap, true);
    }

    // 上一个法线（用于 join 计算）
    let mut prev_nx: f32 = 0.0;
    let mut prev_ny: f32 = 0.0;

    for i in 0..poly.len().saturating_sub(1) {
        let p0 = poly[i];
        let p1 = poly[i + 1];
        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;
        let len = (dx * dx + dy * dy).sqrt().max(0.0001);
        let nx = -dy / len * dir;
        let ny = dx / len * dir;

        let ox0 = p0.x + nx * half_w;
        let oy0 = p0.y + ny * half_w;
        let ox1 = p1.x + nx * half_w;
        let oy1 = p1.y + ny * half_w;

        if i == 0 {
            builder.move_to(ox0, oy0);
        } else if (prev_nx - nx).abs() > 0.0001 || (prev_ny - ny).abs() > 0.0001 {
            add_join(
                builder, p0.x, p0.y, prev_nx, prev_ny, nx, ny, half_w, options,
            );
        }
        builder.line_to(ox1, oy1);

        prev_nx = nx;
        prev_ny = ny;
    }

    // 最后一个点后处理 cap
    if poly.len() >= 2 {
        let last = poly[poly.len() - 1];
        let prev = poly[poly.len() - 2];
        let dx = last.x - prev.x;
        let dy = last.y - prev.y;
        let len = (dx * dx + dy * dy).sqrt().max(0.0001);
        let nx = -dy / len * dir;
        let ny = dx / len * dir;
        add_cap(builder, last, nx, ny, half_w, options.cap, false);
    }
}

/// 添加线段端点 cap。
fn add_cap(
    builder: &mut PathBuilder,
    p: Point,
    nx: f32,
    ny: f32,
    half_w: f32,
    cap: LineCap,
    _is_start: bool,
) {
    match cap {
        LineCap::Butt => {
            // 不需要额外处理，偏移线就是平的
        }
        LineCap::Square => {
            // 延伸半个线宽
            let ex = p.x + nx * half_w;
            let ey = p.y + ny * half_w;
            builder.line_to(ex, ey);
        }
        LineCap::Round => {
            // 用多个线段近似半圆
            let cx = p.x;
            let cy = p.y;
            let steps = 8;
            let start_angle = ny.atan2(nx);
            for i in 1..=steps {
                let angle = start_angle + std::f32::consts::PI * (i as f32) / (steps as f32);
                let rx = cx + angle.cos() * half_w;
                let ry = cy + angle.sin() * half_w;
                builder.line_to(rx, ry);
            }
        }
    }
}

/// 添加线段连接。
///
/// 将当前路径位置（上一段的偏移终点）连接到下一段的偏移起点，
/// 根据 join 类型可能经过中间点（miter 交点或圆弧）。
fn add_join(
    builder: &mut PathBuilder,
    vx: f32,
    vy: f32, // 顶点位置（poly[i]）
    pn_x: f32,
    pn_y: f32, // 上一段的法线方向
    cn_x: f32,
    cn_y: f32, // 当前段的法线方向
    half_w: f32,
    options: &StrokeOptions,
) {
    let off2x = vx + cn_x * half_w;
    let off2y = vy + cn_y * half_w;

    match options.join {
        LineJoin::Bevel => {
            builder.line_to(off2x, off2y);
        }
        LineJoin::Miter => {
            // 两条偏移线的方向 = 原线段方向（法线逆时针旋转 90°）
            let sdx1 = pn_y;
            let sdy1 = -pn_x;
            let sdx2 = cn_y;
            let sdy2 = -cn_x;
            let off1x = vx + pn_x * half_w;
            let off1y = vy + pn_y * half_w;

            // 直线交点：off1 + t * sdir1 = off2 + u * sdir2
            let denom = sdx1 * sdy2 - sdy1 * sdx2;
            if denom.abs() > 0.0001 {
                let t = ((off2x - off1x) * sdy2 - (off2y - off1y) * sdx2) / denom;
                // t > 0 表示交点在偏移线的正向（外扩方向）
                if t > 0.0 {
                    let mx = off1x + t * sdx1;
                    let my = off1y + t * sdy1;
                    let miter_len = ((mx - vx).powi(2) + (my - vy).powi(2)).sqrt();
                    if miter_len <= half_w * options.miter_limit {
                        builder.line_to(mx, my);
                    }
                }
            }
            builder.line_to(off2x, off2y);
        }
        LineJoin::Round => {
            let angle_from = pn_y.atan2(pn_x);
            let angle_to = cn_y.atan2(cn_x);
            let mut da = angle_to - angle_from;
            // 取最短弧方向
            if da > std::f32::consts::PI {
                da -= std::f32::consts::TAU;
            } else if da < -std::f32::consts::PI {
                da += std::f32::consts::TAU;
            }
            let steps = 8;
            for i in 1..steps {
                let t = i as f32 / steps as f32;
                let angle = angle_from + da * t;
                builder.line_to(vx + angle.cos() * half_w, vy + angle.sin() * half_w);
            }
            builder.line_to(off2x, off2y);
        }
    }
}

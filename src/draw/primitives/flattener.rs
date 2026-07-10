//! Bezier 曲线展平器 — 将二次/三次贝塞尔曲线递归细分为直线段。
//!
//! 参考 tiny-skia 的路径展平算法。容差 0.25 像素。

use super::path::PathSegment;
use crate::core::Point;

const MAX_FLATTEN_DEPTH: u8 = 16;

/// 展平后的线段序列：（起点, 终点）。
/// 连续的线段构成一个子路径，Close 表示回到子路径起点。
pub enum FlatSegment {
    MoveTo(Point),
    LineTo(Point),
    Close,
}

/// 展平后的子路径，同时保留 `Close` 的语义。
///
/// 不能仅通过首尾点相等判断闭合：显式回到起点但未调用 `Close` 的开放
/// 子路径仍应绘制端帽。
#[derive(Debug, Clone)]
pub struct FlatSubpath {
    pub points: Vec<Point>,
    pub closed: bool,
}

/// 将路径段序列展平为直线段序列。
/// `tolerance` 控制细分精度（像素单位，越小越精确）。
pub fn flatten(segments: &[PathSegment], tolerance: f32) -> Vec<Vec<Point>> {
    flatten_subpaths(segments, tolerance)
        .into_iter()
        .map(|subpath| subpath.points)
        .collect()
}

/// 将路径段展平，并保留每个子路径是否由 `Close` 显式闭合。
pub fn flatten_subpaths(segments: &[PathSegment], tolerance: f32) -> Vec<FlatSubpath> {
    if segments.iter().any(|segment| !segment_is_finite(segment)) {
        return Vec::new();
    }
    let tol_sq = (tolerance * tolerance).max(0.0001);
    let mut result: Vec<FlatSubpath> = Vec::new();
    let mut current_subpath: Option<Vec<Point>> = None;
    let mut current_point = Point::new(0.0, 0.0);
    let mut start_point = Point::new(0.0, 0.0);

    for seg in segments {
        match *seg {
            PathSegment::MoveTo(p) => {
                // 结束上一个子路径
                if let Some(sub) = current_subpath.take() {
                    result.push(FlatSubpath {
                        points: sub,
                        closed: false,
                    });
                }
                current_point = p;
                start_point = p;
                current_subpath = Some(vec![p]);
            }
            PathSegment::LineTo(p) => {
                if current_subpath.is_none() {
                    start_point = current_point;
                    current_subpath = Some(vec![current_point]);
                }
                if let Some(ref mut sub) = current_subpath {
                    sub.push(p);
                }
                current_point = p;
            }
            PathSegment::QuadTo(c, e) => {
                if current_subpath.is_none() {
                    start_point = current_point;
                    current_subpath = Some(vec![current_point]);
                }
                if let Some(ref mut sub) = current_subpath {
                    flatten_quad(sub, current_point, c, e, tol_sq, 0);
                }
                current_point = e;
            }
            PathSegment::CubicTo(c1, c2, e) => {
                if current_subpath.is_none() {
                    start_point = current_point;
                    current_subpath = Some(vec![current_point]);
                }
                if let Some(ref mut sub) = current_subpath {
                    flatten_cubic(sub, current_point, c1, c2, e, tol_sq, 0);
                }
                current_point = e;
            }
            PathSegment::Close => {
                if let Some(mut sub) = current_subpath.take() {
                    // 如果终点不是起点，加一条回到起点的线段
                    let last = sub.last().copied().unwrap_or(start_point);
                    if (last.x - start_point.x).abs() > 0.001
                        || (last.y - start_point.y).abs() > 0.001
                    {
                        sub.push(start_point);
                    }
                    result.push(FlatSubpath {
                        points: sub,
                        closed: true,
                    });
                }
                current_point = start_point;
            }
        }
    }

    // 提交最后一个子路径
    if let Some(sub) = current_subpath {
        result.push(FlatSubpath {
            points: sub,
            closed: false,
        });
    }

    result
}

/// 将二次贝塞尔曲线细分为直线段。
/// 使用递归中点分割：计算曲线到弦的最大偏差，超过容差则分裂。
fn flatten_quad(out: &mut Vec<Point>, p0: Point, p1: Point, p2: Point, tol_sq: f32, depth: u8) {
    // 如果曲线够平直，直接输出终点
    if depth >= MAX_FLATTEN_DEPTH || is_quad_flat(p0, p1, p2, tol_sq) {
        out.push(p2);
        return;
    }
    // 在 t=0.5 处用 de Casteljau 分裂
    // 左曲线: (p0, (p0+p1)/2, (p0+2*p1+p2)/4)
    // 右曲线: ((p0+2*p1+p2)/4, (p1+p2)/2, p2)
    let left_c = stable_midpoint(p0, p1);
    let right_c = stable_midpoint(p1, p2);
    let split = stable_midpoint(left_c, right_c);

    flatten_quad(out, p0, left_c, split, tol_sq, depth + 1);
    flatten_quad(out, split, right_c, p2, tol_sq, depth + 1);
}

/// 将三次贝塞尔曲线细分为直线段。
fn flatten_cubic(
    out: &mut Vec<Point>,
    p0: Point,
    p1: Point,
    p2: Point,
    p3: Point,
    tol_sq: f32,
    depth: u8,
) {
    if depth >= MAX_FLATTEN_DEPTH || is_cubic_flat(p0, p1, p2, p3, tol_sq) {
        out.push(p3);
        return;
    }
    // de Casteljau 算法在 t=0.5 处分裂
    let mid01 = stable_midpoint(p0, p1);
    let mid12 = stable_midpoint(p1, p2);
    let mid23 = stable_midpoint(p2, p3);
    let mid012 = stable_midpoint(mid01, mid12);
    let mid123 = stable_midpoint(mid12, mid23);
    let split = stable_midpoint(mid012, mid123);

    flatten_cubic(out, p0, mid01, mid012, split, tol_sq, depth + 1);
    flatten_cubic(out, split, mid123, mid23, p3, tol_sq, depth + 1);
}

/// 判断二次曲线是否足够平直（用弦的最大偏差衡量）。
fn is_quad_flat(p0: Point, p1: Point, p2: Point, tol_sq: f32) -> bool {
    // 计算控制点到弦的距离平方
    let chord_dx = p2.x as f64 - p0.x as f64;
    let chord_dy = p2.y as f64 - p0.y as f64;
    let len_sq = chord_dx * chord_dx + chord_dy * chord_dy;
    if len_sq < 0.0001 {
        // 弦极短，用控制点到端点的距离
        return distance_squared_f64(p1, p0) < tol_sq as f64;
    }
    // 控制点到弦的距离: |(P1-P0) × (P2-P0)|² / |P2-P0|²
    let cross = (p1.x as f64 - p0.x as f64) * chord_dy - (p1.y as f64 - p0.y as f64) * chord_dx;
    (cross * cross) / len_sq < tol_sq as f64
}

/// 判断三次曲线是否足够平直。
fn is_cubic_flat(p0: Point, p1: Point, p2: Point, p3: Point, tol_sq: f32) -> bool {
    // 使用控制点到基线的最大距离
    let dx = p3.x as f64 - p0.x as f64;
    let dy = p3.y as f64 - p0.y as f64;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 0.0001 {
        return distance_squared_f64(p1, p0).max(distance_squared_f64(p2, p0)) < tol_sq as f64;
    }
    // 计算 P1 和 P2 到弦 (P0-P3) 的距离平方
    let cross1 = (p1.x as f64 - p0.x as f64) * dy - (p1.y as f64 - p0.y as f64) * dx;
    let cross2 = (p2.x as f64 - p0.x as f64) * dy - (p2.y as f64 - p0.y as f64) * dx;
    let d1 = (cross1 * cross1) / len_sq;
    let d2 = (cross2 * cross2) / len_sq;
    d1.max(d2) < tol_sq as f64
}

fn segment_is_finite(segment: &PathSegment) -> bool {
    match *segment {
        PathSegment::MoveTo(point) | PathSegment::LineTo(point) => point_is_finite(point),
        PathSegment::QuadTo(control, end) => point_is_finite(control) && point_is_finite(end),
        PathSegment::CubicTo(control1, control2, end) => {
            point_is_finite(control1) && point_is_finite(control2) && point_is_finite(end)
        }
        PathSegment::Close => true,
    }
}

fn point_is_finite(point: Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn stable_midpoint(a: Point, b: Point) -> Point {
    Point::new(
        ((a.x as f64 + b.x as f64) * 0.5) as f32,
        ((a.y as f64 + b.y as f64) * 0.5) as f32,
    )
}

fn distance_squared_f64(a: Point, b: Point) -> f64 {
    let dx = a.x as f64 - b.x as f64;
    let dy = a.y as f64 - b.y as f64;
    dx * dx + dy * dy
}

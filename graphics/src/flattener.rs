//! Bezier 曲线展平器 — 将二次/三次贝塞尔曲线递归细分为直线段。
//!
//! 参考 tiny-skia 的路径展平算法。容差 0.25 像素。

use uix_platform::Point;
use super::path::PathSegment;

/// 展平后的线段序列：（起点, 终点）。
/// 连续的线段构成一个子路径，Close 表示回到子路径起点。
pub enum FlatSegment {
    MoveTo(Point),
    LineTo(Point),
    Close,
}

/// 将路径段序列展平为直线段序列。
/// `tolerance` 控制细分精度（像素单位，越小越精确）。
pub fn flatten(segments: &[PathSegment], tolerance: f32) -> Vec<Vec<Point>> {
    let tol_sq = (tolerance * tolerance).max(0.0001);
    let mut result: Vec<Vec<Point>> = Vec::new();
    let mut current_subpath: Option<Vec<Point>> = None;
    let mut current_point = Point::new(0.0, 0.0);
    let mut start_point = Point::new(0.0, 0.0);

    for seg in segments {
        match *seg {
            PathSegment::MoveTo(p) => {
                // 结束上一个子路径
                if let Some(sub) = current_subpath.take() {
                    result.push(sub);
                }
                current_point = p;
                start_point = p;
                current_subpath = Some(vec![p]);
            }
            PathSegment::LineTo(p) => {
                if let Some(ref mut sub) = current_subpath {
                    sub.push(p);
                }
                current_point = p;
            }
            PathSegment::QuadTo(c, e) => {
                if let Some(ref mut sub) = current_subpath {
                    flatten_quad(sub, current_point, c, e, tol_sq);
                }
                current_point = e;
            }
            PathSegment::CubicTo(c1, c2, e) => {
                if let Some(ref mut sub) = current_subpath {
                    flatten_cubic(sub, current_point, c1, c2, e, tol_sq);
                }
                current_point = e;
            }
            PathSegment::Close => {
                if let Some(ref mut sub) = current_subpath {
                    // 如果终点不是起点，加一条回到起点的线段
                    let last = sub.last().copied().unwrap_or(start_point);
                    if (last.x - start_point.x).abs() > 0.001
                        || (last.y - start_point.y).abs() > 0.001
                    {
                        sub.push(start_point);
                    }
                }
                current_point = start_point;
            }
        }
    }

    // 提交最后一个子路径
    if let Some(sub) = current_subpath {
        result.push(sub);
    }

    result
}

/// 将二次贝塞尔曲线细分为直线段。
/// 使用递归中点分割：计算曲线到弦的最大偏差，超过容差则分裂。
fn flatten_quad(out: &mut Vec<Point>, p0: Point, p1: Point, p2: Point, tol_sq: f32) {
    // 如果曲线够平直，直接输出终点
    if is_quad_flat(p0, p1, p2, tol_sq) {
        out.push(p2);
        return;
    }
    // 在 t=0.5 处用 de Casteljau 分裂
    // 左曲线: (p0, (p0+p1)/2, (p0+2*p1+p2)/4)
    // 右曲线: ((p0+2*p1+p2)/4, (p1+p2)/2, p2)
    let left_c = Point::midpoint(p0, p1);
    let split = Point::new(
        (p0.x + 2.0 * p1.x + p2.x) / 4.0,
        (p0.y + 2.0 * p1.y + p2.y) / 4.0,
    );
    let right_c = Point::midpoint(p1, p2);

    flatten_quad(out, p0, left_c, split, tol_sq);
    flatten_quad(out, split, right_c, p2, tol_sq);
}

/// 将三次贝塞尔曲线细分为直线段。
fn flatten_cubic(
    out: &mut Vec<Point>,
    p0: Point, p1: Point, p2: Point, p3: Point,
    tol_sq: f32,
) {
    if is_cubic_flat(p0, p1, p2, p3, tol_sq) {
        out.push(p3);
        return;
    }
    // de Casteljau 算法在 t=0.5 处分裂
    let mid01 = Point::midpoint(p0, p1);
    let mid12 = Point::midpoint(p1, p2);
    let mid23 = Point::midpoint(p2, p3);
    let mid012 = Point::midpoint(mid01, mid12);
    let mid123 = Point::midpoint(mid12, mid23);
    let split = Point::midpoint(mid012, mid123);

    flatten_cubic(out, p0, mid01, mid012, split, tol_sq);
    flatten_cubic(out, split, mid123, mid23, p3, tol_sq);
}

/// 判断二次曲线是否足够平直（用弦的最大偏差衡量）。
fn is_quad_flat(p0: Point, p1: Point, p2: Point, tol_sq: f32) -> bool {
    // 计算控制点到弦的距离平方
    let chord_dx = p2.x - p0.x;
    let chord_dy = p2.y - p0.y;
    let len_sq = chord_dx * chord_dx + chord_dy * chord_dy;
    if len_sq < 0.0001 {
        // 弦极短，用控制点到端点的距离
        return (p1.x - p0.x) * (p1.x - p0.x) + (p1.y - p0.y) * (p1.y - p0.y) < tol_sq;
    }
    // 控制点到弦的距离: |(P1-P0) × (P2-P0)|² / |P2-P0|²
    let cross = (p1.x - p0.x) * chord_dy - (p1.y - p0.y) * chord_dx;
    (cross * cross) / len_sq < tol_sq
}

/// 判断三次曲线是否足够平直。
fn is_cubic_flat(p0: Point, p1: Point, p2: Point, p3: Point, tol_sq: f32) -> bool {
    // 使用控制点到基线的最大距离
    let dx = p3.x - p0.x;
    let dy = p3.y - p0.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 0.0001 {
        return true;
    }
    // 计算 P1 和 P2 到弦 (P0-P3) 的距离平方
    let cross1 = (p1.x - p0.x) * dy - (p1.y - p0.y) * dx;
    let cross2 = (p2.x - p0.x) * dy - (p2.y - p0.y) * dx;
    let d1 = (cross1 * cross1) / len_sq;
    let d2 = (cross2 * cross2) / len_sq;
    d1.max(d2) < tol_sq
}

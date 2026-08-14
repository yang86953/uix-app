// 拆分自 tessellator.rs：几何谓词与 ear-clip 三角剖分辅助函数群。
// 引入基础点类型与复杂路径分解共用的双精度点。
use crate::core::Point;
// 引入复杂路径分解共用的双精度点与相对容差比较。
use super::{F64Point, approximately_equal};

pub(super) fn ordered_or_snapped(left: f64, right: f64) -> Option<(f64, f64)> {
    if left <= right {
        return Some((left, right));
    }
    if approximately_equal(left, right) {
        let midpoint = left + (right - left) * 0.5;
        return Some((midpoint, midpoint));
    }
    None
}

/// f64 二维叉积。
pub(super) fn cross_f64(a: F64Point, b: F64Point) -> f64 {
    a.x * b.y - a.y * b.x
}

/// 鞋带公式计算多边形有符号面积（f64）。
pub(super) fn polygon_area_f64(pts: &[Point]) -> f64 {
    let mut area = 0.0f64;
    for i in 0..pts.len() {
        let point = pts[i];
        let next = pts[(i + 1) % pts.len()];
        area += point.x as f64 * next.y as f64 - next.x as f64 * point.y as f64;
    }
    area * 0.5
}

/// 计算环的包围盒 (min_x, min_y, max_x, max_y)。
pub(super) fn ring_bounds(ring: &[Point]) -> (f32, f32, f32, f32) {
    ring.iter().fold(
        (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ),
        |(min_x, min_y, max_x, max_y), point| {
            (
                min_x.min(point.x),
                min_y.min(point.y),
                max_x.max(point.x),
                max_y.max(point.y),
            )
        },
    )
}

/// 包围盒是否重叠（含 epsilon 容差）。
pub(super) fn bounds_overlap(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    const EPSILON: f32 = 1e-5;
    a.0 <= b.2 + EPSILON && a.2 + EPSILON >= b.0 && a.1 <= b.3 + EPSILON && a.3 + EPSILON >= b.1
}

/// 两条边是否在环中相邻（共享端点）。
pub(super) fn edges_are_adjacent(a: usize, b: usize, len: usize) -> bool {
    a == b || (a + 1) % len == b || (b + 1) % len == a
}

/// 两个环的任意边对是否相交或相触。
pub(super) fn rings_intersect_or_touch(a: &[Point], b: &[Point]) -> bool {
    for i in 0..a.len() {
        let a0 = a[i];
        let a1 = a[(i + 1) % a.len()];
        for j in 0..b.len() {
            let b0 = b[j];
            let b1 = b[(j + 1) % b.len()];
            if segments_intersect_or_touch(a0, a1, b0, b1) {
                return true;
            }
        }
    }
    false
}

/// 判断两线段是否正规相交或端点相触（含 epsilon 容差）。
pub(super) fn segments_intersect_or_touch(a0: Point, a1: Point, b0: Point, b1: Point) -> bool {
    const EPSILON: f32 = 1e-5;

    let o1 = cross(a0, a1, b0);
    let o2 = cross(a0, a1, b1);
    let o3 = cross(b0, b1, a0);
    let o4 = cross(b0, b1, a1);
    let proper = ((o1 > EPSILON && o2 < -EPSILON) || (o1 < -EPSILON && o2 > EPSILON))
        && ((o3 > EPSILON && o4 < -EPSILON) || (o3 < -EPSILON && o4 > EPSILON));
    proper
        || (o1.abs() <= EPSILON && point_on_segment(b0, a0, a1, EPSILON))
        || (o2.abs() <= EPSILON && point_on_segment(b1, a0, a1, EPSILON))
        || (o3.abs() <= EPSILON && point_on_segment(a0, b0, b1, EPSILON))
        || (o4.abs() <= EPSILON && point_on_segment(a1, b0, b1, EPSILON))
}

/// 点在包围盒意义上是否位于线段上（配合 epsilon）。
pub(super) fn point_on_segment(p: Point, a: Point, b: Point, epsilon: f32) -> bool {
    p.x >= a.x.min(b.x) - epsilon
        && p.x <= a.x.max(b.x) + epsilon
        && p.y >= a.y.min(b.y) - epsilon
        && p.y <= a.y.max(b.y) + epsilon
}

/// 射线法判断点是否位于环内。
pub(super) fn point_in_ring(point: Point, ring: &[Point]) -> bool {
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[j];
        let crosses = (a.y > point.y) != (b.y > point.y);
        if crosses {
            let x = (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x;
            if point.x < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// 鞋带公式计算多边形有符号面积（f32，供 ear-clip 定向使用）。
pub(super) fn polygon_area(pts: &[Point]) -> f32 {
    let n = pts.len();
    let mut a = 0.0;
    for i in 0..n {
        let p = pts[i];
        let q = pts[(i + 1) % n];
        a += p.x * q.y - q.x * p.y;
    }
    a * 0.5
}

/// 以 o 为原点的二维叉积（判断三点转向）。
pub(crate) fn cross(o: Point, a: Point, b: Point) -> f32 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

/// 点是否位于三角形内（含边界，同侧法）。
pub(crate) fn point_in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let c1 = cross(a, b, p);
    let c2 = cross(b, c, p);
    let c3 = cross(c, a, p);
    let has_neg = (c1 < 0.0) || (c2 < 0.0) || (c3 < 0.0);
    let has_pos = (c1 > 0.0) || (c2 > 0.0) || (c3 > 0.0);
    !(has_neg && has_pos)
}

/// 顶点在给定环绕方向下是否凸出。
pub(super) fn is_convex(prev: Point, curr: Point, next: Point, ccw: bool) -> bool {
    let c = cross(prev, curr, next);
    if ccw {
        c > 1e-6
    } else {
        c < -1e-6
    }
}

/// 判断顶点是否为「耳朵」：凸出且三角形内不包含其他顶点。
pub(super) fn is_ear(pts: &[Point], indices: &[usize], ear_i: usize, ccw: bool) -> bool {
    let n = indices.len();
    let i_prev = indices[(ear_i + n - 1) % n];
    let i_curr = indices[ear_i];
    let i_next = indices[(ear_i + 1) % n];
    let a = pts[i_prev];
    let b = pts[i_curr];
    let c = pts[i_next];
    if !is_convex(a, b, c, ccw) {
        return false;
    }
    for (j, &idx) in indices.iter().enumerate() {
        if j == (ear_i + n - 1) % n || j == ear_i || j == (ear_i + 1) % n {
            continue;
        }
        if point_in_triangle(pts[idx], a, b, c) {
            return false;
        }
    }
    true
}

/// 自实现 ear-clip：对无洞简单多边形输出三角形列表，失败返回 None。
pub(super) fn ear_clip(ring: &[Point]) -> Option<Vec<f32>> {
    let area = polygon_area(ring);
    // 零面积退化环没有可切内容。
    if area.abs() < 1e-8 {
        return None;
    }
    // 由面积符号确定环绕方向。
    let ccw = area > 0.0;
    // 用剩余顶点索引表模拟不断收缩的多边形。
    let mut indices: Vec<usize> = (0..ring.len()).collect();
    let mut tris: Vec<f32> = Vec::with_capacity((ring.len().saturating_sub(2)) * 6);
    // 防死循环守卫：每轮必须切下一个耳朵，否则放弃。
    let mut guard = ring.len() * ring.len() + 8;
    while indices.len() > 3 {
        if guard == 0 {
            return None;
        }
        guard -= 1;
        // 扫描剩余顶点找第一个耳朵。
        let n = indices.len();
        let mut found = None;
        for i in 0..n {
            if is_ear(ring, &indices, i, ccw) {
                found = Some(i);
                break;
            }
        }
        // 找不到耳朵说明多边形退化，放弃。
        let i = found?;
        // 切下耳朵：输出三角形并删除该顶点。
        let i_prev = indices[(i + n - 1) % n];
        let i_curr = indices[i];
        let i_next = indices[(i + 1) % n];
        let a = ring[i_prev];
        let b = ring[i_curr];
        let c = ring[i_next];
        tris.extend_from_slice(&[a.x, a.y, b.x, b.y, c.x, c.y]);
        indices.remove(i);
    }
    // 最后三个顶点构成收尾三角形。
    let a = ring[indices[0]];
    let b = ring[indices[1]];
    let c = ring[indices[2]];
    tris.extend_from_slice(&[a.x, a.y, b.x, b.y, c.x, c.y]);
    Some(tris)
}

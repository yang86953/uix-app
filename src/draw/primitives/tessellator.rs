//! Simple-polygon tessellation for GPU-native path fills/strokes (#169).
//!
//! - **Fill**: flatten → ear-clip simple rings (disjoint rings OK).
//! - **Stroke**: flatten → thick-line segment quads (butt ends; joins overlap).
//!
//! Complex cases (self-intersections, holes, ear-clip failure) return `None`
//! so callers can soft-fallback.

use super::flattener;
use super::path::{FillRule, Path};
use super::stroker::StrokeOptions;
use crate::core::Point;

/// Max vertices in a ring we attempt to ear-clip (keeps GPU upload bounded).
const MAX_RING_VERTS: usize = 512;

/// Tessellate a path into a triangle-list of xy pairs (`[x0,y0, x1,y1, …]`).
///
/// Native slice constraints:
/// - identity caller (transform applied upstream)
/// - simple polygon rings after flatten (no holes / self-intersections)
/// - `FillRule` accepted; EvenOdd ≡ NonZero for non-self-intersecting rings
pub fn tessellate_fill(path: &Path, fill_rule: FillRule) -> Option<Vec<f32>> {
    let _ = fill_rule;
    let polys = flattener::flatten(path.segments(), 0.25);
    if polys.is_empty() {
        return None;
    }
    let mut tris: Vec<f32> = Vec::new();
    for poly in &polys {
        let ring = match clean_ring(poly) {
            Some(r) => r,
            None => continue,
        };
        if ring.len() > MAX_RING_VERTS {
            return None;
        }
        let ear = ear_clip(&ring)?;
        tris.extend(ear);
    }
    if tris.len() < 6 {
        return None;
    }
    Some(tris)
}

/// Tessellate a stroke as thick-line segment quads (triangle list).
///
/// Uses butt-style segment ends; adjacent segments overlap at joins (fine for
/// opaque Alpha/SrcOver). Round/Square caps are not modeled exactly.
pub fn tessellate_stroke(path: &Path, opts: &StrokeOptions) -> Option<Vec<f32>> {
    let half = opts.width * 0.5;
    if half < 1e-4 {
        return None;
    }
    let polys = flattener::flatten(path.segments(), 0.25);
    if polys.is_empty() {
        return None;
    }
    let mut tris: Vec<f32> = Vec::new();
    for poly in &polys {
        if poly.len() < 2 {
            continue;
        }
        for i in 0..poly.len() - 1 {
            let p0 = poly[i];
            let p1 = poly[i + 1];
            let dx = p1.x - p0.x;
            let dy = p1.y - p0.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-4 {
                continue;
            }
            let nx = -dy / len * half;
            let ny = dx / len * half;
            let a = Point::new(p0.x + nx, p0.y + ny);
            let b = Point::new(p1.x + nx, p1.y + ny);
            let c = Point::new(p1.x - nx, p1.y - ny);
            let d = Point::new(p0.x - nx, p0.y - ny);
            // Two triangles: a-b-c, a-c-d
            tris.extend_from_slice(&[a.x, a.y, b.x, b.y, c.x, c.y]);
            tris.extend_from_slice(&[a.x, a.y, c.x, c.y, d.x, d.y]);
        }
    }
    if tris.len() < 6 {
        return None;
    }
    Some(tris)
}

fn clean_ring(pts: &[Point]) -> Option<Vec<Point>> {
    if pts.len() < 3 {
        return None;
    }
    let mut out: Vec<Point> = Vec::with_capacity(pts.len());
    for p in pts {
        if let Some(last) = out.last() {
            if (last.x - p.x).abs() < 1e-4 && (last.y - p.y).abs() < 1e-4 {
                continue;
            }
        }
        out.push(*p);
    }
    if out.len() >= 2 {
        let first = out[0];
        let &last = out.last()?;
        if (first.x - last.x).abs() < 1e-4 && (first.y - last.y).abs() < 1e-4 {
            out.pop();
        }
    }
    if out.len() < 3 {
        return None;
    }
    Some(out)
}

fn polygon_area(pts: &[Point]) -> f32 {
    let n = pts.len();
    let mut a = 0.0;
    for i in 0..n {
        let p = pts[i];
        let q = pts[(i + 1) % n];
        a += p.x * q.y - q.x * p.y;
    }
    a * 0.5
}

fn cross(o: Point, a: Point, b: Point) -> f32 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

fn point_in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let c1 = cross(a, b, p);
    let c2 = cross(b, c, p);
    let c3 = cross(c, a, p);
    let has_neg = (c1 < 0.0) || (c2 < 0.0) || (c3 < 0.0);
    let has_pos = (c1 > 0.0) || (c2 > 0.0) || (c3 > 0.0);
    !(has_neg && has_pos)
}

fn is_convex(prev: Point, curr: Point, next: Point, ccw: bool) -> bool {
    let c = cross(prev, curr, next);
    if ccw {
        c > 1e-6
    } else {
        c < -1e-6
    }
}

fn is_ear(pts: &[Point], indices: &[usize], ear_i: usize, ccw: bool) -> bool {
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

fn ear_clip(ring: &[Point]) -> Option<Vec<f32>> {
    let area = polygon_area(ring);
    if area.abs() < 1e-8 {
        return None;
    }
    let ccw = area > 0.0;
    let mut indices: Vec<usize> = (0..ring.len()).collect();
    let mut tris: Vec<f32> = Vec::with_capacity((ring.len().saturating_sub(2)) * 6);
    let mut guard = ring.len() * ring.len() + 8;
    while indices.len() > 3 {
        if guard == 0 {
            return None;
        }
        guard -= 1;
        let n = indices.len();
        let mut found = None;
        for i in 0..n {
            if is_ear(ring, &indices, i, ccw) {
                found = Some(i);
                break;
            }
        }
        let i = found?;
        let i_prev = indices[(i + n - 1) % n];
        let i_curr = indices[i];
        let i_next = indices[(i + 1) % n];
        let a = ring[i_prev];
        let b = ring[i_curr];
        let c = ring[i_next];
        tris.extend_from_slice(&[a.x, a.y, b.x, b.y, c.x, c.y]);
        indices.remove(i);
    }
    let a = ring[indices[0]];
    let b = ring[indices[1]];
    let c = ring[indices[2]];
    tris.extend_from_slice(&[a.x, a.y, b.x, b.y, c.x, c.y]);
    Some(tris)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::primitives::path::PathBuilder;

    #[test]
    fn tessellates_triangle() {
        let mut pb = PathBuilder::new();
        pb.move_to(0.0, 0.0)
            .line_to(10.0, 0.0)
            .line_to(5.0, 8.0)
            .close();
        let v = tessellate_fill(&pb.build(), FillRule::NonZero).expect("tri");
        assert_eq!(v.len(), 6);
    }

    #[test]
    fn tessellates_quad_to_two_tris() {
        let mut pb = PathBuilder::new();
        pb.move_to(0.0, 0.0)
            .line_to(20.0, 0.0)
            .line_to(20.0, 10.0)
            .line_to(0.0, 10.0)
            .close();
        let v = tessellate_fill(&pb.build(), FillRule::NonZero).expect("quad");
        assert_eq!(v.len(), 12);
    }

    #[test]
    fn tessellates_two_disjoint_tris() {
        let mut pb = PathBuilder::new();
        pb.move_to(0.0, 0.0)
            .line_to(10.0, 0.0)
            .line_to(5.0, 8.0)
            .close();
        pb.move_to(20.0, 0.0)
            .line_to(30.0, 0.0)
            .line_to(25.0, 8.0)
            .close();
        let v = tessellate_fill(&pb.build(), FillRule::NonZero).expect("two");
        assert_eq!(v.len(), 12);
    }

    #[test]
    fn stroke_line_tessellates() {
        let mut pb = PathBuilder::new();
        pb.move_to(0.0, 0.0).line_to(40.0, 0.0);
        let opts = StrokeOptions {
            width: 4.0,
            ..Default::default()
        };
        let v = tessellate_stroke(&pb.build(), &opts).expect("stroke");
        assert_eq!(v.len(), 12); // one segment → 2 tris
    }
}

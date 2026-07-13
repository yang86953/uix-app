//! Conservative tessellation for GPU-native path fills/strokes (#169).
//!
//! - **Fill**: strict contour forests use the compact ear-clip / `earcut` fast
//!   path; intersecting, touching and self-intersecting contours use a
//!   fill-rule-aware continuous y-band decomposition into trapezoid triangles.
//! - **Stroke**: build a shared cap/join-aware stroke outline → tessellate the
//!   resulting fill region through the same strict/complex paths.
//!
//! Non-finite input, resource-budget overflow or numerically ambiguous
//! tessellation returns `None` so callers can soft-fallback.

use super::flattener;
use super::path::{FillRule, Path, PathSegment};
use super::stroker::{self, StrokeOptions};
use crate::core::Point;

/// Max vertices in a ring we attempt to tessellate (keeps GPU upload bounded).
const MAX_RING_VERTS: usize = 512;
/// Max flattened vertices across all rings before the topology guard falls back.
const MAX_PATH_VERTS: usize = 2048;
/// Max unique vertex/intersection y events in the complex-path decomposition.
const MAX_COMPLEX_EVENTS: usize = 8192;
/// Max cumulative pair, edge-band scan and crossing-sort work.
const MAX_COMPLEX_WORK: usize = 4_194_304;
/// Max triangles emitted by the complex-path decomposition.
pub(crate) const MAX_COMPLEX_TRIANGLES: usize = 65_536;

/// Tessellate a path into a triangle-list of xy pairs (`[x0,y0, x1,y1, …]`).
///
/// Native constraints:
/// - identity caller (transform applied upstream)
/// - bounded flatten output and tessellation work
/// - arbitrary contour order/direction, intersections, touches and self-crosses
/// - exact `EvenOdd` parity and `NonZero` accumulated-winding state per y-band
pub fn tessellate_fill(path: &Path, fill_rule: FillRule) -> Option<Vec<f32>> {
    if !path_is_finite(path) {
        return None;
    }
    let polys = flattener::flatten(path.segments(), 0.25);
    if polys.is_empty() {
        return Some(Vec::new());
    }
    let mut rings = Vec::with_capacity(polys.len());
    let mut path_vertices = 0usize;
    for poly in &polys {
        let Some(ring) = clean_ring(poly).ok()? else {
            continue;
        };
        path_vertices = path_vertices.checked_add(ring.len())?;
        if ring.len() > MAX_RING_VERTS || path_vertices > MAX_PATH_VERTS {
            return None;
        }
        rings.push(ring);
    }
    if rings.is_empty() {
        return Some(Vec::new());
    }

    if rings.iter().all(|ring| ring_is_simple(ring)) {
        if let Some(tris) = tessellate_strict_fill(&rings, fill_rule) {
            return Some(tris);
        }
    }
    tessellate_complex_fill(&rings, fill_rule)
}

fn path_is_finite(path: &Path) -> bool {
    let finite = |point: Point| point.x.is_finite() && point.y.is_finite();
    path.segments().iter().all(|segment| match *segment {
        PathSegment::MoveTo(point) | PathSegment::LineTo(point) => finite(point),
        PathSegment::QuadTo(control, end) => finite(control) && finite(end),
        PathSegment::CubicTo(control1, control2, end) => {
            finite(control1) && finite(control2) && finite(end)
        }
        PathSegment::Close => true,
    })
}

fn tessellate_strict_fill(rings: &[Vec<Point>], fill_rule: FillRule) -> Option<Vec<f32>> {
    let groups = classify_fill_groups(rings, fill_rule)?;
    let mut tris = Vec::new();
    for group in &groups {
        tris.extend(tessellate_fill_group(rings, group)?);
    }
    if tris.len() < 6 {
        return None;
    }
    Some(tris)
}

/// Tessellate a stroke outline into a non-overlapping triangle list.
///
/// Cap/join geometry is shared with the CPU rasterizer. Unsupported outline
/// topology returns `None`, so callers retain the existing soft fallback.
pub fn tessellate_stroke(path: &Path, opts: &StrokeOptions) -> Option<Vec<f32>> {
    let rings = stroker::stroke_outline_rings(path, opts)?;
    if rings.is_empty() {
        return Some(Vec::new());
    }
    tessellate_fill(&stroker::path_from_outline_rings(rings), FillRule::NonZero)
}

fn clean_ring(pts: &[Point]) -> Result<Option<Vec<Point>>, ()> {
    if pts.len() < 3 {
        return Ok(None);
    }
    let mut out: Vec<Point> = Vec::with_capacity(pts.len());
    for p in pts {
        if !p.x.is_finite() || !p.y.is_finite() {
            return Err(());
        }
        if let Some(last) = out.last() {
            if (last.x - p.x).abs() < 1e-4 && (last.y - p.y).abs() < 1e-4 {
                continue;
            }
        }
        out.push(*p);
    }
    if out.len() >= 2 {
        let first = out[0];
        let &last = out.last().ok_or(())?;
        if (first.x - last.x).abs() < 1e-4 && (first.y - last.y).abs() < 1e-4 {
            out.pop();
        }
    }
    if out.len() < 3 {
        return Ok(None);
    }
    Ok(Some(out))
}

fn ring_is_simple(ring: &[Point]) -> bool {
    let len = ring.len();
    for i in 0..len {
        let a0 = ring[i];
        let a1 = ring[(i + 1) % len];
        for j in i + 1..len {
            if edges_are_adjacent(i, j, len) {
                continue;
            }
            let b0 = ring[j];
            let b1 = ring[(j + 1) % len];
            if segments_intersect_or_touch(a0, a1, b0, b1) {
                return false;
            }
        }
    }
    true
}

#[derive(Debug)]
struct FillGroup {
    outer: usize,
    holes: Vec<usize>,
}

/// Build a strict containment forest, then classify every ring by the fill
/// state immediately outside and inside that boundary.
fn classify_fill_groups(rings: &[Vec<Point>], fill_rule: FillRule) -> Option<Vec<FillGroup>> {
    let areas: Vec<f64> = rings.iter().map(|ring| polygon_area_f64(ring)).collect();
    if areas
        .iter()
        .any(|area| !area.is_finite() || area.abs() <= 1e-8)
    {
        return None;
    }
    let bounds: Vec<_> = rings.iter().map(|ring| ring_bounds(ring)).collect();
    let mut parents = vec![None; rings.len()];
    for i in 0..rings.len() {
        for j in i + 1..rings.len() {
            if !bounds_overlap(bounds[i], bounds[j]) {
                continue;
            }
            if rings_intersect_or_touch(&rings[i], &rings[j]) {
                return None;
            }
            let i_in_j = point_in_ring(rings[i][0], &rings[j]);
            let j_in_i = point_in_ring(rings[j][0], &rings[i]);
            if i_in_j && j_in_i {
                return None;
            }
            if i_in_j {
                update_parent(&mut parents, &areas, i, j)?;
            } else if j_in_i {
                update_parent(&mut parents, &areas, j, i)?;
            }
        }
    }

    let mut order: Vec<usize> = (0..rings.len()).collect();
    order.sort_by(|&a, &b| {
        areas[b]
            .abs()
            .total_cmp(&areas[a].abs())
            .then_with(|| a.cmp(&b))
    });

    let mut inside_winding = vec![None::<i32>; rings.len()];
    let mut inside_parity = vec![None::<bool>; rings.len()];
    let mut active_group = vec![None::<usize>; rings.len()];
    let mut groups = Vec::<FillGroup>::new();

    for ring in order {
        let (outside_winding, outside_parity, outside_group) = match parents[ring] {
            Some(parent) => (
                inside_winding[parent]?,
                inside_parity[parent]?,
                active_group[parent],
            ),
            None => (0, false, None),
        };
        let winding_delta = if areas[ring] > 0.0 { 1 } else { -1 };
        let winding = outside_winding.checked_add(winding_delta)?;
        let parity = !outside_parity;
        let outside_filled = match fill_rule {
            FillRule::EvenOdd => outside_parity,
            FillRule::NonZero => outside_winding != 0,
        };
        let inside_filled = match fill_rule {
            FillRule::EvenOdd => parity,
            FillRule::NonZero => winding != 0,
        };

        let group = match (outside_filled, inside_filled) {
            (false, true) => {
                if outside_group.is_some() {
                    return None;
                }
                let group = groups.len();
                groups.push(FillGroup {
                    outer: ring,
                    holes: Vec::new(),
                });
                Some(group)
            }
            (true, false) => {
                let group = outside_group?;
                groups.get_mut(group)?.holes.push(ring);
                None
            }
            (true, true) => Some(outside_group?),
            (false, false) => return None,
        };

        inside_winding[ring] = Some(winding);
        inside_parity[ring] = Some(parity);
        active_group[ring] = group;
    }

    if groups.is_empty() {
        None
    } else {
        Some(groups)
    }
}

fn update_parent(
    parents: &mut [Option<usize>],
    areas: &[f64],
    child: usize,
    candidate: usize,
) -> Option<()> {
    if areas[candidate].abs() <= areas[child].abs() {
        return None;
    }
    match parents[child] {
        Some(current) if areas[current].abs() <= areas[candidate].abs() => {}
        _ => parents[child] = Some(candidate),
    }
    Some(())
}

fn tessellate_fill_group(rings: &[Vec<Point>], group: &FillGroup) -> Option<Vec<f32>> {
    let outer = rings.get(group.outer)?;
    let expected_area =
        group
            .holes
            .iter()
            .try_fold(polygon_area_f64(outer).abs(), |area, &hole| {
                let remaining = area - polygon_area_f64(rings.get(hole)?).abs();
                remaining.is_finite().then_some(remaining)
            })?;
    if expected_area <= 1e-8 {
        return None;
    }

    let tris = if group.holes.is_empty() {
        ear_clip(outer)?
    } else {
        let vertex_count = group.holes.iter().try_fold(outer.len(), |count, &hole| {
            count.checked_add(rings.get(hole)?.len())
        })?;
        let mut vertices = Vec::<[f32; 2]>::with_capacity(vertex_count);
        vertices.extend(outer.iter().map(|point| [point.x, point.y]));
        let mut hole_indices = Vec::with_capacity(group.holes.len());
        for &hole in &group.holes {
            hole_indices.push(vertices.len());
            vertices.extend(rings.get(hole)?.iter().map(|point| [point.x, point.y]));
        }

        let mut indices = Vec::<usize>::new();
        earcut::Earcut::<f32>::new().earcut(vertices.iter().copied(), &hole_indices, &mut indices);
        if indices.len() < 3 || !indices.chunks_exact(3).remainder().is_empty() {
            return None;
        }

        let mut triangles = Vec::with_capacity(indices.len().checked_mul(2)?);
        for triangle in indices.chunks_exact(3) {
            let a = *vertices.get(triangle[0])?;
            let b = *vertices.get(triangle[1])?;
            let c = *vertices.get(triangle[2])?;
            triangles.extend_from_slice(&[a[0], a[1], b[0], b[1], c[0], c[1]]);
        }
        triangles
    };

    validate_triangle_area(&tris, expected_area).then_some(tris)
}

fn validate_triangle_area(tris: &[f32], expected_area: f64) -> bool {
    if tris.len() < 6 || !tris.chunks_exact(6).remainder().is_empty() {
        return false;
    }
    let mut triangulated_area = 0.0f64;
    for triangle in tris.chunks_exact(6) {
        if triangle.iter().any(|coordinate| !coordinate.is_finite()) {
            return false;
        }
        let (ax, ay) = (triangle[0] as f64, triangle[1] as f64);
        let (bx, by) = (triangle[2] as f64, triangle[3] as f64);
        let (cx, cy) = (triangle[4] as f64, triangle[5] as f64);
        triangulated_area += ((bx - ax) * (cy - ay) - (by - ay) * (cx - ax)).abs() * 0.5;
    }
    let tolerance = expected_area.max(1.0) * 1e-4;
    (triangulated_area - expected_area).abs() <= tolerance
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct F64Point {
    pub(crate) x: f64,
    pub(crate) y: f64,
}

impl From<Point> for F64Point {
    fn from(point: Point) -> Self {
        Self {
            x: point.x as f64,
            y: point.y as f64,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ComplexEdge {
    id: usize,
    start: F64Point,
    end: F64Point,
    ymin: f64,
    ymax: f64,
    winding: i32,
}

impl ComplexEdge {
    fn x_at(self, y: f64) -> Option<f64> {
        let dy = self.end.y - self.start.y;
        if dy == 0.0 {
            return None;
        }
        let x = self.start.x + (y - self.start.y) * (self.end.x - self.start.x) / dy;
        x.is_finite().then_some(x)
    }
}

#[derive(Debug, Clone, Copy)]
struct BandCrossing {
    edge_id: usize,
    x_mid: f64,
    x0: f64,
    x1: f64,
    winding: i32,
}

#[derive(Debug, Clone, Copy)]
struct BoundaryLine {
    x0: f64,
    x1: f64,
}

/// Decompose arbitrary directed contours into non-overlapping filled
/// trapezoids. Every vertex and proper crossing y is a band boundary, so edge
/// order is stable inside each open band.
pub(crate) fn tessellate_complex_fill(rings: &[Vec<Point>], fill_rule: FillRule) -> Option<Vec<f32>> {
    let mut edges = Vec::<ComplexEdge>::new();
    let mut events = Vec::<f64>::new();
    for ring in rings {
        events.extend(ring.iter().map(|point| point.y as f64));
        for index in 0..ring.len() {
            let start = F64Point::from(ring[index]);
            let end = F64Point::from(ring[(index + 1) % ring.len()]);
            let dx = end.x - start.x;
            let dy = end.y - start.y;
            if dx == 0.0 && dy == 0.0 {
                continue;
            }
            if dy == 0.0 {
                continue;
            }
            edges.push(ComplexEdge {
                id: edges.len(),
                start,
                end,
                ymin: start.y.min(end.y),
                ymax: start.y.max(end.y),
                winding: if end.y > start.y { 1 } else { -1 },
            });
        }
    }
    if edges.is_empty() {
        return Some(Vec::new());
    }

    let mut work = edges
        .len()
        .checked_mul(edges.len().saturating_sub(1))?
        .checked_div(2)?;
    if work > MAX_COMPLEX_WORK {
        return None;
    }

    let max_event_candidates = MAX_COMPLEX_EVENTS.checked_mul(8)?;
    for left in 0..edges.len() {
        for right in left + 1..edges.len() {
            if !complex_edge_bounds_overlap(edges[left], edges[right]) {
                continue;
            }
            if let Some(y) = proper_intersection_y(edges[left], edges[right])? {
                events.push(y);
                if events.len() > max_event_candidates {
                    return None;
                }
            }
        }
    }

    events.sort_by(f64::total_cmp);
    events.dedup_by(|a, b| *a == *b);
    if events.len() > MAX_COMPLEX_EVENTS {
        return None;
    }
    if events.len() < 2 {
        return Some(Vec::new());
    }
    work = work.checked_add(edges.len().checked_mul(events.len() - 1)?)?;
    if work > MAX_COMPLEX_WORK {
        return None;
    }

    let mut triangles = Vec::<f32>::new();
    let mut expected_area = 0.0f64;
    for band in events.windows(2) {
        let (y0, y1) = (band[0], band[1]);
        if y1 <= y0 {
            continue;
        }
        if approximately_equal(y0, y1) {
            // Do not merge distinct near-coincident events: their ordering is
            // numerically ambiguous, so preserve correctness via soft fallback.
            return None;
        }
        let y_mid = y0 + (y1 - y0) * 0.5;
        let mut crossings = Vec::<BandCrossing>::new();
        for &edge in &edges {
            if !(edge.ymin < y_mid && y_mid < edge.ymax) {
                continue;
            }
            crossings.push(BandCrossing {
                edge_id: edge.id,
                x_mid: edge.x_at(y_mid)?,
                x0: edge.x_at(y0)?,
                x1: edge.x_at(y1)?,
                winding: edge.winding,
            });
        }
        if crossings.is_empty() {
            continue;
        }
        let sort_levels = usize::BITS as usize - crossings.len().leading_zeros() as usize;
        work = work.checked_add(crossings.len().checked_mul(sort_levels)?)?;
        if work > MAX_COMPLEX_WORK {
            return None;
        }
        crossings.sort_by(|a, b| {
            a.x_mid
                .total_cmp(&b.x_mid)
                .then_with(|| a.x0.total_cmp(&b.x0))
                .then_with(|| a.x1.total_cmp(&b.x1))
                .then_with(|| a.edge_id.cmp(&b.edge_id))
        });

        let mut winding = 0i32;
        let mut parity = false;
        let mut left_boundary = None::<BoundaryLine>;
        let mut index = 0usize;
        while index < crossings.len() {
            let mut end = index + 1;
            while end < crossings.len()
                && approximately_equal(crossings[index].x_mid, crossings[end].x_mid)
            {
                if !same_band_line(crossings[index], crossings[end], &edges) {
                    // Approximate proximity is not proof of coincidence. A
                    // distinct near line or lost crossing is ambiguous.
                    return None;
                }
                end += 1;
            }

            let before = fill_state(fill_rule, winding, parity);
            match fill_rule {
                FillRule::EvenOdd => {
                    if (end - index) % 2 == 1 {
                        parity = !parity;
                    }
                }
                FillRule::NonZero => {
                    let delta = crossings[index..end]
                        .iter()
                        .try_fold(0i32, |sum, crossing| sum.checked_add(crossing.winding))?;
                    winding = winding.checked_add(delta)?;
                }
            }
            let after = fill_state(fill_rule, winding, parity);
            let boundary = BoundaryLine {
                x0: crossings[index].x0,
                x1: crossings[index].x1,
            };
            match (before, after) {
                (false, true) => {
                    if left_boundary.replace(boundary).is_some() {
                        return None;
                    }
                }
                (true, false) => {
                    let left = left_boundary.take()?;
                    expected_area += append_complex_span(&mut triangles, left, boundary, y0, y1)?;
                }
                (false, false) | (true, true) => {}
            }
            index = end;
        }
        if fill_state(fill_rule, winding, parity) || left_boundary.is_some() {
            return None;
        }
    }

    if triangles.is_empty() {
        return Some(triangles);
    }
    validate_triangle_area(&triangles, expected_area).then_some(triangles)
}

fn complex_edge_bounds_overlap(a: ComplexEdge, b: ComplexEdge) -> bool {
    let a_min_x = a.start.x.min(a.end.x);
    let a_max_x = a.start.x.max(a.end.x);
    let b_min_x = b.start.x.min(b.end.x);
    let b_max_x = b.start.x.max(b.end.x);
    a_min_x <= b_max_x && b_min_x <= a_max_x && a.ymin <= b.ymax && b.ymin <= a.ymax
}

/// Return `Some(Some(y))` for a proper interior crossing, `Some(None)` for no
/// crossing/parallel lines, and `None` for non-finite arithmetic.
fn proper_intersection_y(a: ComplexEdge, b: ComplexEdge) -> Option<Option<f64>> {
    let a_direction = F64Point {
        x: a.end.x - a.start.x,
        y: a.end.y - a.start.y,
    };
    let b_direction = F64Point {
        x: b.end.x - b.start.x,
        y: b.end.y - b.start.y,
    };
    let denominator = cross_f64(a_direction, b_direction);
    if !denominator.is_finite() {
        return None;
    }
    if denominator == 0.0 {
        return Some(None);
    }
    let delta = F64Point {
        x: b.start.x - a.start.x,
        y: b.start.y - a.start.y,
    };
    let t = cross_f64(delta, b_direction) / denominator;
    let u = cross_f64(delta, a_direction) / denominator;
    if !t.is_finite() || !u.is_finite() {
        return None;
    }
    if !(0.0 < t && t < 1.0 && 0.0 < u && u < 1.0) {
        return Some(None);
    }
    let y = a.start.y + t * a_direction.y;
    y.is_finite().then_some(Some(y))
}

fn fill_state(fill_rule: FillRule, winding: i32, parity: bool) -> bool {
    match fill_rule {
        FillRule::EvenOdd => parity,
        FillRule::NonZero => winding != 0,
    }
}

fn same_band_line(a: BandCrossing, b: BandCrossing, edges: &[ComplexEdge]) -> bool {
    let a_edge = edges[a.edge_id];
    let b_edge = edges[b.edge_id];
    let a_direction = F64Point {
        x: a_edge.end.x - a_edge.start.x,
        y: a_edge.end.y - a_edge.start.y,
    };
    let b_direction = F64Point {
        x: b_edge.end.x - b_edge.start.x,
        y: b_edge.end.y - b_edge.start.y,
    };
    let start_delta = F64Point {
        x: b_edge.start.x - a_edge.start.x,
        y: b_edge.start.y - a_edge.start.y,
    };
    if cross_f64(a_direction, b_direction) != 0.0 || cross_f64(a_direction, start_delta) != 0.0 {
        return false;
    }
    approximately_equal(a.x_mid, b.x_mid)
        && approximately_equal(a.x0, b.x0)
        && approximately_equal(a.x1, b.x1)
}

fn approximately_equal(a: f64, b: f64) -> bool {
    let scale = a.abs().max(b.abs()).max(1.0);
    (a - b).abs() <= scale * 1e-10
}

fn append_complex_span(
    triangles: &mut Vec<f32>,
    left: BoundaryLine,
    right: BoundaryLine,
    y0: f64,
    y1: f64,
) -> Option<f64> {
    let (left0, right0) = ordered_or_snapped(left.x0, right.x0)?;
    let (left1, right1) = ordered_or_snapped(left.x1, right.x1)?;
    let width0 = right0 - left0;
    let width1 = right1 - left1;
    let area = (width0 + width1) * 0.5 * (y1 - y0);
    if !area.is_finite() || area < 0.0 {
        return None;
    }
    if area == 0.0 {
        return Some(0.0);
    }

    let top_left = F64Point { x: left0, y: y0 };
    let top_right = F64Point { x: right0, y: y0 };
    let bottom_right = F64Point { x: right1, y: y1 };
    let bottom_left = F64Point { x: left1, y: y1 };
    let before = triangles.len();
    append_complex_triangle(triangles, top_left, top_right, bottom_right)?;
    append_complex_triangle(triangles, top_left, bottom_right, bottom_left)?;
    if triangles.len() == before {
        return None;
    }
    Some(area)
}

pub(crate) fn append_complex_triangle(
    triangles: &mut Vec<f32>,
    a: F64Point,
    b: F64Point,
    c: F64Point,
) -> Option<()> {
    let vertices = [
        a.x as f32, a.y as f32, b.x as f32, b.y as f32, c.x as f32, c.y as f32,
    ];
    if vertices.iter().any(|coordinate| !coordinate.is_finite()) {
        return None;
    }
    let (ax, ay) = (vertices[0] as f64, vertices[1] as f64);
    let (bx, by) = (vertices[2] as f64, vertices[3] as f64);
    let (cx, cy) = (vertices[4] as f64, vertices[5] as f64);
    let area = ((bx - ax) * (cy - ay) - (by - ay) * (cx - ax)).abs() * 0.5;
    if area <= 1e-12 {
        return Some(());
    }
    if triangles.len().checked_div(6)? >= MAX_COMPLEX_TRIANGLES {
        return None;
    }
    triangles.extend_from_slice(&vertices);
    Some(())
}

fn ordered_or_snapped(left: f64, right: f64) -> Option<(f64, f64)> {
    if left <= right {
        return Some((left, right));
    }
    if approximately_equal(left, right) {
        let midpoint = left + (right - left) * 0.5;
        return Some((midpoint, midpoint));
    }
    None
}

fn cross_f64(a: F64Point, b: F64Point) -> f64 {
    a.x * b.y - a.y * b.x
}

fn polygon_area_f64(pts: &[Point]) -> f64 {
    let mut area = 0.0f64;
    for i in 0..pts.len() {
        let point = pts[i];
        let next = pts[(i + 1) % pts.len()];
        area += point.x as f64 * next.y as f64 - next.x as f64 * point.y as f64;
    }
    area * 0.5
}

fn ring_bounds(ring: &[Point]) -> (f32, f32, f32, f32) {
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

fn bounds_overlap(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    const EPSILON: f32 = 1e-5;
    a.0 <= b.2 + EPSILON && a.2 + EPSILON >= b.0 && a.1 <= b.3 + EPSILON && a.3 + EPSILON >= b.1
}

fn edges_are_adjacent(a: usize, b: usize, len: usize) -> bool {
    a == b || (a + 1) % len == b || (b + 1) % len == a
}

fn rings_intersect_or_touch(a: &[Point], b: &[Point]) -> bool {
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

fn segments_intersect_or_touch(a0: Point, a1: Point, b0: Point, b1: Point) -> bool {
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

fn point_on_segment(p: Point, a: Point, b: Point, epsilon: f32) -> bool {
    p.x >= a.x.min(b.x) - epsilon
        && p.x <= a.x.max(b.x) + epsilon
        && p.y >= a.y.min(b.y) - epsilon
        && p.y <= a.y.max(b.y) + epsilon
}

fn point_in_ring(point: Point, ring: &[Point]) -> bool {
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

pub(crate) fn cross(o: Point, a: Point, b: Point) -> f32 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

pub(crate) fn point_in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
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


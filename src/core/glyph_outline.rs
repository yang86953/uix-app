//! Pure glyph-outline helpers shared by drawing and native raster adapters.

const CORNER_ANGLE_THRESHOLD: f32 = 3.0;
const ENDPOINT_EPS: f32 = 1e-3;

pub(crate) const EDGE_RED: u8 = 1;
pub(crate) const EDGE_GREEN: u8 = 2;
pub(crate) const EDGE_BLUE: u8 = 4;
pub(crate) const EDGE_YELLOW: u8 = EDGE_RED | EDGE_GREEN;
pub(crate) const EDGE_MAGENTA: u8 = EDGE_RED | EDGE_BLUE;
pub(crate) const EDGE_CYAN: u8 = EDGE_GREEN | EDGE_BLUE;
pub(crate) const EDGE_WHITE: u8 = EDGE_RED | EDGE_GREEN | EDGE_BLUE;

/// Chlumsky `edgeColoringSimple`：按角点在闭合子路径上切换 CMY 双通道色。
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
        switch_color(color, seed, None);
        for local in 0..m {
            colors[range.start + local] = *color;
        }
        return;
    }

    if corners.len() == 1 {
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

#[inline]
pub(crate) fn is_outline_edges(data: &[f32]) -> bool {
    data.len() >= 4 && data.len().is_multiple_of(4)
}

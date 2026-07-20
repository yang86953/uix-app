// 字形轮廓边 → RGBA8 atlas 多通道 SDF（MSDF）。
// 边色为 Chlumsky CMY 双通道掩码（CPU colorize_edges）；采样 median → coverage。

struct CoverIn {
    @location(0) position: vec2<f32>,
    @location(1) atlas_origin: vec2<f32>,
    @location(2) @interpolate(flat) edge_start: u32,
    @location(3) @interpolate(flat) edge_count: u32,
}

struct CoverOut {
    @builtin(position) position: vec4<f32>,
    @location(0) atlas_origin: vec2<f32>,
    @location(1) @interpolate(flat) edge_start: u32,
    @location(2) @interpolate(flat) edge_count: u32,
}

@group(0) @binding(0) var<storage, read> edges: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> edge_colors: array<u32>;

const MSDF_RANGE: f32 = 4.0;
const EDGE_RED: u32 = 1u;
const EDGE_GREEN: u32 = 2u;
const EDGE_BLUE: u32 = 4u;

@vertex
fn cover_vs(input: CoverIn) -> CoverOut {
    var out: CoverOut;
    out.position = vec4<f32>(input.position, 0.0, 1.0);
    out.atlas_origin = input.atlas_origin;
    out.edge_start = input.edge_start;
    out.edge_count = input.edge_count;
    return out;
}

fn point_segment_dist(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let denom = max(dot(ba, ba), 1e-12);
    let t = clamp(dot(pa, ba) / denom, 0.0, 1.0);
    return length(pa - ba * t);
}

@fragment
fn cover_fs(input: CoverOut) -> @location(0) vec4<f32> {
    // @builtin(position) 为 atlas 像素坐标；用像素中心采样。
    let p = input.position.xy - input.atlas_origin - vec2<f32>(0.5, 0.5);
    var winding = 0;
    var min_dist = 1e10;
    var channel_dist = vec3<f32>(1e10, 1e10, 1e10);
    let start = input.edge_start;
    let end = start + input.edge_count;
    for (var i = start; i < end; i = i + 1u) {
        let e = edges[i];
        let a = e.xy;
        let b = e.zw;
        let d = point_segment_dist(p, a, b);
        min_dist = min(min_dist, d);
        let mask = edge_colors[i];
        if (mask & EDGE_RED) != 0u {
            channel_dist.x = min(channel_dist.x, d);
        }
        if (mask & EDGE_GREEN) != 0u {
            channel_dist.y = min(channel_dist.y, d);
        }
        if (mask & EDGE_BLUE) != 0u {
            channel_dist.z = min(channel_dist.z, d);
        }
        if (a.y > p.y) == (b.y > p.y) {
            continue;
        }
        let dy = b.y - a.y;
        if abs(dy) <= 1e-8 {
            continue;
        }
        let t = (p.y - a.y) / dy;
        let x_int = a.x + t * (b.x - a.x);
        if p.x < x_int {
            winding += select(-1, 1, dy > 0.0);
        }
    }
    let fallback = select(0.0, min_dist, min_dist < 1e9);
    var dist = channel_dist;
    if dist.x >= 1e9 {
        dist.x = fallback;
    }
    if dist.y >= 1e9 {
        dist.y = fallback;
    }
    if dist.z >= 1e9 {
        dist.z = fallback;
    }
    let sign = select(1.0, -1.0, winding != 0);
    let sd = dist * sign;
    let rgb = clamp(vec3<f32>(0.5) + sd / MSDF_RANGE, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb, 1.0);
}

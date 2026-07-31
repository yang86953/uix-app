struct ShapeIn {
    @location(0) position: vec2<f32>,
    @location(1) local: vec2<f32>,
    @location(2) color_a: vec4<f32>,
    @location(3) color_b: vec4<f32>,
    @location(4) params0: vec4<f32>,
    @location(5) params1: vec4<f32>,
    @location(6) mode: u32,
}

struct ShapeOut {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) color_a: vec4<f32>,
    @location(2) color_b: vec4<f32>,
    @location(3) params0: vec4<f32>,
    @location(4) params1: vec4<f32>,
    @location(5) @interpolate(flat) mode: u32,
}

@vertex
fn shape_vs(input: ShapeIn) -> ShapeOut {
    var out: ShapeOut;
    out.position = vec4<f32>(input.position, 0.0, 1.0);
    out.local = input.local;
    out.color_a = input.color_a;
    out.color_b = input.color_b;
    out.params0 = input.params0;
    out.params1 = input.params1;
    out.mode = input.mode;
    return out;
}

fn rounded_distance(local: vec2<f32>, size: vec2<f32>, radii: vec4<f32>) -> f32 {
    let right = local.x >= size.x * 0.5;
    let bottom = local.y >= size.y * 0.5;
    var radius = radii.x;
    if right && !bottom { radius = radii.y; }
    if right && bottom { radius = radii.z; }
    if !right && bottom { radius = radii.w; }
    radius = clamp(radius, 0.0, min(size.x, size.y) * 0.5);
    let q = abs(local - size * 0.5) - (size * 0.5 - vec2<f32>(radius));
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - radius;
}

fn premul(color: vec4<f32>, coverage: f32) -> vec4<f32> {
    let alpha = color.a * coverage;
    return vec4<f32>(color.rgb * alpha, alpha);
}

@fragment
fn shape_fs(input: ShapeOut) -> @location(0) vec4<f32> {
    if input.mode == 0u {
        return premul(input.color_a, 1.0);
    }
    if input.mode == 1u {
        let distance = rounded_distance(input.local, input.params0.xy, input.params1);
        return premul(input.color_a, smoothstep(0.75, -0.75, distance));
    }
    if input.mode == 2u {
        let outer = rounded_distance(input.local, input.params0.xy, input.params1);
        let inset = max(input.params0.z, 0.0);
        let inner_size = max(input.params0.xy - vec2<f32>(inset * 2.0), vec2<f32>(0.0));
        var coverage = smoothstep(0.75, -0.75, outer);
        if (inner_size.x > 0.0 && inner_size.y > 0.0) {
            // 描边宽度盖满矩形时跳过 inner（与 CPU 一致），否则双 SDF。
            let inner_radii = max(input.params1 - vec4<f32>(inset), vec4<f32>(0.0));
            let inner = rounded_distance(input.local - vec2<f32>(inset), inner_size, inner_radii);
            // Matches CPU `sdf_to_coverage(outer) * sdf_to_coverage(-inner)`.
            coverage *= smoothstep(-0.75, 0.75, inner);
        }
        return premul(input.color_a, coverage);
    }
    if input.mode == 3u {
        var t = input.local.x;
        if input.params0.x == 1.0 { t = input.local.y; }
        if input.params0.x == 2.0 { t = (input.local.x + input.local.y) * 0.5; }
        if input.params0.x == 3.0 { t = (input.local.x + 1.0 - input.local.y) * 0.5; }
        return premul(mix(input.color_a, input.color_b, clamp(t, 0.0, 1.0)), 1.0);
    }
    if input.mode == 4u {
        let distance = length(input.local);
        if distance > input.params0.y { discard; }
        let span = max(input.params0.y - input.params0.x, 0.0001);
        let t = clamp((distance - input.params0.x) / span, 0.0, 1.0);
        return premul(mix(input.color_a, input.color_b, t), 1.0);
    }
    if input.mode == 7u {
        let radius = input.params0.x;
        let start = input.params0.y;
        let sweep = input.params0.z;
        let radial_distance = length(input.local) - radius;
        var signed_distance = radial_distance;
        if sweep < 6.283184 {
            var delta = atan2(input.local.y, input.local.x) - start;
            if delta < 0.0 { delta = delta + 6.283185307; }
            let point_radius = length(input.local);
            var angular_distance = 0.0;
            if delta <= sweep {
                angular_distance = -min(delta, sweep - delta) * point_radius;
            } else {
                angular_distance = min(delta - sweep, 6.283185307 - delta) * point_radius;
            }
            if point_radius < 0.75 { angular_distance = -0.75; }
            signed_distance = max(radial_distance, angular_distance);
        }
        let aa = max(fwidth(signed_distance) * 0.5, 0.001);
        let coverage = smoothstep(aa, -aa, signed_distance);
        return premul(input.color_a, coverage);
    }
    // mode 5 = drop shadow, mode 6 = ambient；params0 = (exp_w, exp_h, blur_x, blur_y)
    // 将局部坐标缩放到单位 blur 空间，使各向异性模糊仍可用圆形 SDF。
    let blur_x = max(input.params0.z, 0.5);
    let blur_y = max(input.params0.w, 0.5);
    let scale = vec2<f32>(1.0 / blur_x, 1.0 / blur_y);
    let local = input.local * scale;
    let size = input.params0.xy * scale;
    let radii = input.params1 * vec4<f32>(scale.x, scale.x, scale.y, scale.y);
    let distance = rounded_distance(local, size, radii);
    var coverage = smoothstep(1.0, -1.0, distance);
    if input.mode == 6u { coverage = coverage * coverage; }
    return premul(input.color_a, coverage);
}

struct GlyphIn {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct GlyphOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn glyph_vs(input: GlyphIn) -> GlyphOut {
    var out: GlyphOut;
    out.position = vec4<f32>(input.position, 0.0, 1.0);
    out.uv = input.uv;
    out.color = input.color;
    return out;
}

@group(0) @binding(0) var glyph_texture: texture_2d<f32>;
@group(0) @binding(1) var glyph_sampler: sampler;

@fragment
fn glyph_fs(input: GlyphOut) -> @location(0) vec4<f32> {
    let coverage = textureSample(glyph_texture, glyph_sampler, input.uv).r;
    let alpha = input.color.a * coverage;
    return vec4<f32>(input.color.rgb * alpha, alpha);
}

fn median3(a: f32, b: f32, c: f32) -> f32 {
    return max(min(a, b), min(max(a, b), c));
}

const MSDF_RANGE: f32 = 4.0;

@fragment
fn glyph_msdf_fs(input: GlyphOut) -> @location(0) vec4<f32> {
    let sample = textureSample(glyph_texture, glyph_sampler, input.uv);
    let m = median3(sample.r, sample.g, sample.b);
    // Chlumsky：用 UV 的屏幕导数换算 atlas 中 MSDF_RANGE 对应多少屏幕像素。
    // 勿对 m 做 fwidth——平坦区域导数≈0 会导致 coverage 抖成碎边/糊团。
    let atlas_size = vec2<f32>(textureDimensions(glyph_texture, 0));
    let unit_range = vec2<f32>(MSDF_RANGE) / atlas_size;
    let screen_tex_size = vec2<f32>(1.0) / max(fwidth(input.uv), vec2<f32>(1e-5));
    let screen_px_range = max(0.5 * dot(unit_range, screen_tex_size), 1.0);
    // 编码约定：m<0.5 为字形内部（sd<0）。
    let coverage = clamp(0.5 - screen_px_range * (m - 0.5), 0.0, 1.0);
    let alpha = input.color.a * coverage;
    return vec4<f32>(input.color.rgb * alpha, alpha);
}

struct TextureIn {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) opacity: f32,
}

struct TextureOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) opacity: f32,
}

@vertex
fn texture_vs(input: TextureIn) -> TextureOut {
    var out: TextureOut;
    out.position = vec4<f32>(input.position, 0.0, 1.0);
    out.uv = input.uv;
    out.opacity = input.opacity;
    return out;
}

@group(0) @binding(0) var color_texture: texture_2d<f32>;
@group(0) @binding(1) var color_sampler: sampler;

@fragment
fn texture_fs(input: TextureOut) -> @location(0) vec4<f32> {
    // Source pixels are BGRA premultiplied; opacity scales the whole premul color.
    return textureSample(color_texture, color_sampler, input.uv) * input.opacity;
}

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
        let inner_radii = max(input.params1 - vec4<f32>(inset), vec4<f32>(0.0));
        let inner = rounded_distance(input.local - vec2<f32>(inset), inner_size, inner_radii);
        let coverage = smoothstep(0.75, -0.75, outer) * smoothstep(-0.75, 0.75, inner);
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
    let distance = rounded_distance(input.local, input.params0.xy, input.params1);
    let blur = max(input.params0.z, 0.5);
    var coverage = smoothstep(blur, -blur, distance);
    if input.params0.w > 0.5 { coverage = coverage * coverage; }
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

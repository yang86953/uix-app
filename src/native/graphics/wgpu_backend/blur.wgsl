struct BlurIn {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct BlurOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct BlurUniforms {
    texel_size: vec2<f32>,
    direction: vec2<f32>,
    // 与 CPU gaussian_blur 一致：sigma = radius / 3
    radius: f32,
    sigma: f32,
    _pad: vec2<f32>,
}

@group(0) @binding(0) var color_texture: texture_2d<f32>;
@group(0) @binding(1) var color_sampler: sampler;
@group(0) @binding(2) var<uniform> blur_params: BlurUniforms;

@vertex
fn blur_vs(input: BlurIn) -> BlurOut {
    var out: BlurOut;
    out.position = vec4<f32>(input.position, 0.0, 1.0);
    out.uv = input.uv;
    return out;
}

@fragment
fn blur_fs(input: BlurOut) -> @location(0) vec4<f32> {
    let kernel_radius = i32(ceil(max(blur_params.radius, 0.0)));
    let radius = min(kernel_radius, 32);
    let sigma = max(blur_params.sigma, 0.0001);
    let sigma2 = -1.0 / (2.0 * sigma * sigma);
    let step = blur_params.texel_size * blur_params.direction;
    var color = vec4<f32>(0.0);
    var total = 0.0;
    for (var i: i32 = -radius; i <= radius; i = i + 1) {
        let offset = f32(i);
        let weight = exp(offset * offset * sigma2);
        total = total + weight;
        let uv = input.uv + step * offset;
        color = color + textureSample(color_texture, color_sampler, uv) * weight;
    }
    return color / max(total, 0.0001);
}

@fragment
fn blit_fs(input: BlurOut) -> @location(0) vec4<f32> {
    return textureSample(color_texture, color_sampler, input.uv);
}

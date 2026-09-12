//! Windows D3D Adapter 共享的唯一 HLSL 源集合。

// RECT_HLSL 是 D3D11 与 D3D12 共用的唯一原生 shader 语义源码。
pub(super) const RECT_HLSL: &str = concat!(
    include_str!("d3d_common_sdf.hlsl"),
    r#"
cbuffer RectCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    float4 u_rect;
    float4 u_color;
    float4 u_radius;
    // x 保存半描边宽度，y 保存 fringe，z/w 保存共享 outer/inner 原点偏移。
    float4 u_stroke;
    // 共享 GPU Raster Module 已经计算完成的实际绘制边界。
    float4 u_draw_rect;
};

struct VSIn {
    float2 pos : POSITION;
};

struct VSOut {
    float4 pos : SV_POSITION;
    float2 local : TEXCOORD0;
    float2 rect_size : TEXCOORD1;
};

VSOut VSMain(VSIn input)
{
    VSOut o;
    // 平台 shader 只消费共享层冻结的绘制边界，不再自行推导描边外扩。
    float2 pos = u_draw_rect.xy + input.pos * u_draw_rect.zw;
    float2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    o.pos = float4(ndc, 0.0, 1.0);
    // 局部坐标覆盖共享 draw rect，供片元阶段恢复同心双 SDF。
    o.local = input.pos * u_draw_rect.zw;
    o.rect_size = u_rect.zw;
    return o;
}

// Port of CPU `rounded_rect_sdf` (center-relative, per-corner radius)
// moved to d3d_common_sdf.hlsl, included once at the top of this source.

float4 PSMain(VSOut input) : SV_Target
{
    float mask;
    if (u_stroke.x > 0.0)
    {
        // 使用共享 outer 偏移把 draw rect 局部坐标转换到 outer SDF 原点。
        float2 outer_local = input.local - float2(u_stroke.z, u_stroke.z);
        // 使用共享 inner 偏移保持 inner 与 outer SDF 严格同心。
        float2 inner_local = input.local - float2(u_stroke.w, u_stroke.w);
        // 双 SDF（与 CPU 一致）：外扩/内缩 half 使弧线端点对齐像素
        // 中心，消除整数坐标下顶/底圆角起点偏差；中心行 coverage 与 CPU 相同。
        float h = u_stroke.x;
        float2 outer_size = input.rect_size + 2.0 * h;
        float4 outer_rad = u_radius + h;
        float2 inner_size = max(input.rect_size - 2.0 * h, 0.0);
        float4 inner_rad = max(u_radius - h, 0.0);
        // 计算外边界有符号距离。
        float outer_sd = rounded_rect_sdf(outer_local, outer_size, outer_rad);
        if (inner_size.x > 0.0 && inner_size.y > 0.0)
        {
            // 以内缩后的独立局部原点计算同心内边界距离。
            float inner_sd = rounded_rect_sdf(inner_local, inner_size, inner_rad);
            // Matches CPU `sdf_to_coverage(outer) * sdf_to_coverage(-inner)`.
            mask = saturate(0.5 - outer_sd) * saturate(0.5 + inner_sd);
        }
        else
        {
            // 描边宽度盖满矩形：直接填充外扩圆角矩形。
            mask = saturate(0.5 - outer_sd);
        }
    }
    else
    {
        // Same 1px linear AA coverage as CPU `sdf_to_coverage`.
        mask = any(u_radius > 0.0)
            ? saturate(0.5 - rounded_rect_sdf(input.local, input.rect_size, u_radius))
            : 1.0;
    }
    if (mask <= 0.0)
        discard;
    // Match CPU fill/stroke: quantize the 8-bit straight color, premultiply
    // once, then apply analytic coverage before premultiplied SrcOver blend.
    float4 color = floor(saturate(u_color) * 255.0 + 0.5);
    float3 premul = floor(color.rgb * color.a / 255.0);
    return float4(premul * mask, color.a * mask) / 255.0;
}
"#,
);

// GLYPH_HLSL 是 D3D11 与 D3D12 共用的唯一原生 shader 语义源码。
pub(super) const GLYPH_HLSL: &str = r#"
cbuffer GlyphCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    float u_corner_radius;
    float3 _pad1;
};

Texture2D<float> u_atlas : register(t0);
SamplerState u_samp : register(s0);

struct VSIn {
    float2 pos : POSITION;
    float2 uv : TEXCOORD0;
    float4 color : COLOR0;
};

struct VSOut {
    float4 pos : SV_POSITION;
    float2 uv : TEXCOORD0;
    float4 color : COLOR0;
};

VSOut VSMain(VSIn input)
{
    VSOut o;
    float2 ndc = (input.pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    o.pos = float4(ndc, 0.0, 1.0);
    o.uv = input.uv;
    o.color = input.color;
    return o;
}

float4 PSMain(VSOut input) : SV_Target
{
    // Match the CPU glyph path's two integer truncation steps before the
    // premultiplied SrcOver blend. Keeping this quantization in the shader
    // avoids the 2-channel drift caused by a straight-alpha hardware multiply.
    // 共享 R8Unorm 与 nearest 契约已保证 coverage 位于单位域。
    float coverage = floor(u_atlas.Sample(u_samp, input.uv) * 255.0 + 0.5);
    // 共享 FramePlan 顶点契约已保证 tint 位于单位域。
    float4 color = floor(input.color * 255.0 + 0.5);
    float alpha = floor(color.a * coverage / 255.0);
    float3 premul = floor(color.rgb * color.a / 255.0);
    float3 rgb = floor(premul * coverage / 255.0);
    return float4(rgb, alpha) / 255.0;
}
"#;

// RHI_TEXTURED_PS_HLSL 是 D3D11 与 D3D12 共用的唯一原生 shader 语义源码。
pub(super) const RHI_TEXTURED_PS_HLSL: &str = r#"
cbuffer SampledCB : register(b0)
{
    // xy = viewport，zw = 左上/右上缺口补画峰值。
    float4 u_line0;
    // x = 圆角半径，y = 缺口补画衰减距离，zw = 左下/右下补画峰值。
    float4 u_line1;
};

Texture2D u_tex : register(t0);
SamplerState u_samp : register(s0);

struct VSOut {
    float4 pos : SV_POSITION;
    float2 uv : TEXCOORD0;
    float4 color : COLOR0;
};

float4 PSMain(VSOut input) : SV_Target
{
    float4 sample = u_tex.Sample(u_samp, input.uv);
    // FramePlan 已验证顶点颜色属于单位域，D3D11 不再私自饱和输入。
    float4 tint = input.color;
    float coverage = 1.0;
    float fill_alpha = 0.0;
    if (u_line1.x > 0.0)
    {
        float2 half_size = u_line0.xy * 0.5;
        float2 centered = abs(input.pos.xy - half_size);
        float2 distance = centered - half_size + u_line1.x;
        float signed_distance = length(max(distance, 0.0))
            + min(max(distance.x, distance.y), 0.0) - u_line1.x;
        coverage = saturate(0.5 - signed_distance / max(fwidth(signed_distance), 0.0001));
        if (u_line1.y > 0.0)
        {
            // 圆角缺口用与客户端阴影环一致的二次衰减延伸外圈阴影；
            // uv 保持内容左上原点约定，按所在象限取逐角补画峰值。
            float horizontal = input.uv.y < 0.5
                ? lerp(u_line0.z, u_line0.w, step(0.5, input.uv.x))
                : lerp(u_line1.z, u_line1.w, step(0.5, input.uv.x));
            float falloff = saturate(1.0 - signed_distance / u_line1.y);
            fill_alpha = horizontal * falloff * falloff;
        }
    }
    float4 color = float4(sample.rgb * tint.rgb, sample.a * tint.a);
    // 缺口阴影是预乘黑色，只向输出 alpha 贡献补画事实。
    return float4(color.rgb * coverage, color.a * coverage + fill_alpha * (1.0 - coverage));
}
"#;

// GRADIENT_HLSL 是 D3D11 与 D3D12 共用的唯一原生 shader 语义源码。
pub(super) const GRADIENT_HLSL: &str = concat!(
    include_str!("d3d_common_sdf.hlsl"),
    r#"
cbuffer GradCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    float4 u_origin_edge_x; // xy = origin, zw = physical edge X
    float4 u_edge_y;        // xy = physical edge Y, zw = padding
    float4 u_color_a;
    float4 u_color_b;
    // x = mode (0=linear, 1=radial)
    // y = linear dir (0..3) OR radial inner radius in local space
    // z = radial outer radius in local space OR linear local rect width
    // w = linear local rect height
    float4 u_params;
    // S4 rounded-corner mask: per-corner radii, quad size + unit origin, unit size.
    float4 u_mask_radius;
    float4 u_quad_mask;
    float4 u_mask_size;
};

struct VSIn {
    float2 pos : POSITION;
};

struct VSOut {
    float4 pos : SV_POSITION;
    float2 local : TEXCOORD0;
};

VSOut VSMain(VSIn input)
{
    VSOut o;
    float2 origin = u_origin_edge_x.xy;
    float2 edge_x = u_origin_edge_x.zw;
    float2 edge_y = u_edge_y.xy;
    float2 pos = origin + input.pos.x * edge_x + input.pos.y * edge_y;
    float2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    o.pos = float4(ndc, 0.0, 1.0);
    o.local = input.pos;
    return o;
}

float4 PSMain(VSOut input) : SV_Target
{
    // S4 rounded-corner mask evaluated in unit-quad pixel space.
    float coverage = 1.0;
    if (any(u_mask_radius > float4(0.0, 0.0, 0.0, 0.0)))
    {
        float2 quad_size = u_quad_mask.xy;
        float2 mask_origin = u_quad_mask.zw;
        float2 mask_px = (input.local - mask_origin) * quad_size;
        float2 mask_wh = u_mask_size.xy * quad_size;
        float sdf = rounded_rect_sdf(mask_px, mask_wh, u_mask_radius);
        coverage = saturate(0.5 - sdf);
        if (coverage <= 0.0)
            discard;
    }
    float mode = u_params.x;
    if (mode < 0.5)
    {
        // Linear — matches CPU fill_linear_gradient t calculation.
        float dir = u_params.y;
        float t;
        float2 local = input.local;
        float2 size = float2(u_params.z, u_params.w);
        if (dir < 0.5)
            t = local.x;
        else if (dir < 1.5)
            t = local.y;
        else if (dir < 2.5)
            t = (local.x * size.x + local.y * size.y) / max(size.x + size.y, 1e-6);
        else
            t = (local.x * size.x - local.y * size.y + size.y) / max(size.x + size.y, 1e-6);
        t = saturate(t);
        return lerp(u_color_a, u_color_b, t) * coverage;
    }
    else
    {
        // Radial — local space is the original circle's normalized square.
        float2 center = float2(0.5, 0.5);
        float dist = length(input.local - center);
        float outer_r = u_params.z;
        float inner_r = u_params.y;
        if (dist > outer_r)
            discard;
        float range = max(outer_r - inner_r, 1e-6);
        float t = saturate((dist - inner_r) / range);
        return lerp(u_color_a, u_color_b, t) * coverage;
    }
}
"#,
);

// MESH_HLSL 是 D3D11 与 D3D12 共用的唯一原生 shader 语义源码。
pub(super) const MESH_HLSL: &str = r#"
cbuffer MeshCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    float4 u_color;
};

struct VSIn {
    float2 pos : POSITION;
    float coverage : TEXCOORD0;
};

struct VSOut {
    float4 pos : SV_POSITION;
    float coverage : TEXCOORD0;
};

VSOut VSMain(VSIn input)
{
    VSOut o;
    float2 ndc = (input.pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    o.pos = float4(ndc, 0.0, 1.0);
    o.coverage = input.coverage;
    return o;
}

float4 PSMain(VSOut input) : SV_Target
{
    return float4(u_color.rgb, u_color.a * saturate(input.coverage));
}
"#;

// BLUR_HLSL 是 D3D11 与 D3D12 共用的唯一原生 shader 语义源码。
pub(super) const BLUR_HLSL: &str = r#"
Texture2D u_tex : register(t0);
SamplerState u_samp : register(s0);

cbuffer BlurCB : register(b0)
{
    // xy/zw = source sampling domain min/max pixel-center UV
    float4 u_uv_bounds;
    // xy = normalized one-texel step; z = tap radius (taps = 2r+1)
    float4 u_step_taps;
    // Gaussian weights ordered [-r..+r], zero-terminated, max 64 taps
    float4 u_weights[16];
};

struct VSIn { float2 pos : POSITION; float2 uv : TEXCOORD0; };
struct VSOut { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; };

VSOut VSMain(VSIn input)
{
    VSOut o;
    o.pos = float4(input.pos, 0.0, 1.0);
    o.uv = input.uv;
    return o;
}

// Separable Gaussian: one pass samples along the shared normalized texel step.
float4 PSMain(VSOut input) : SV_TARGET
{
    float2 step = u_step_taps.xy;
    int radius = (int)u_step_taps.z;
    float4 color = 0;
    for (int i = 0; i < 64; ++i)
    {
        float w = u_weights[i / 4][i % 4];
        if (w <= 0.0) break;
        float2 off = step * (float)(i - radius);
        float2 sample_uv = clamp(input.uv + off, u_uv_bounds.xy, u_uv_bounds.zw);
        color += w * u_tex.Sample(u_samp, sample_uv);
    }
    return color;
}
"#;

// SHADOW_HLSL 是 D3D11 与 D3D12 共用的唯一原生 shader 语义源码。
pub(super) const SHADOW_HLSL: &str = concat!(
    include_str!("d3d_common_sdf.hlsl"),
    r#"
cbuffer ShadowCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    // Shadow expanded quad origin and x edge in device coordinates.
    float4 u_rect;
    float4 u_color;
    float4 u_radius;
    // xy = y edge, zw = x/y blur in device coordinates.
    float4 u_params;
    // xy = shadow body size, z = ambient flag, w unused.
    float4 u_size;
};

struct VSIn {
    float2 pos : POSITION;
};

struct VSOut {
    float4 pos : SV_POSITION;
    float2 local : TEXCOORD0;
    float2 rect_size : TEXCOORD1;
};

VSOut VSMain(VSIn input)
{
    VSOut o;
    float2 blur = max(u_params.zw, 0.0);
    float2 body_size = max(u_size.xy, float2(0.0001, 0.0001));
    float2 expanded_size = body_size + 2.0 * blur;
    float2 axis_x = u_rect.zw / max(expanded_size.x, 0.0001);
    float2 axis_y = u_params.xy / max(expanded_size.y, 0.0001);
    float2 draw_xy = u_rect.xy - axis_x - axis_y;
    float2 draw_edge_x = u_rect.zw + axis_x * 2.0;
    float2 draw_edge_y = u_params.xy + axis_y * 2.0;
    float2 pos = draw_xy + input.pos.x * draw_edge_x + input.pos.y * draw_edge_y;
    float2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    o.pos = float4(ndc, 0.0, 1.0);
    o.local = input.pos * (expanded_size + 2.0);
    o.rect_size = body_size;
    return o;
}

// rounded_rect_sdf is shared via d3d_common_sdf.hlsl, included once above.

float shadow_coverage(float sd, float blur)
{
    float t = saturate((blur - sd) / (2.0 * blur));
    return t * t * (3.0 - 2.0 * t);
}

float shadow_coverage_ambient(float sd, float blur)
{
    float halfb = blur * 0.5;
    float t = saturate((halfb - sd) / (blur + halfb));
    float t2 = t * t;
    return t2 * t2 * (5.0 - 4.0 * t);
}

float4 PSMain(VSOut input) : SV_Target
{
    float2 blur = max(u_params.zw, 0.0);
    float blur_radius = max(blur.x, blur.y);
    float2 shape_local = input.local - blur - 1.0;
    float sd = rounded_rect_sdf(shape_local, input.rect_size, u_radius);
    float coverage;
    if (blur_radius > 0.5)
    {
        if (u_size.z > 0.5)
            coverage = shadow_coverage_ambient(sd, blur_radius);
        else
            coverage = shadow_coverage(sd, blur_radius);
    }
    else
    {
        // Matches CPU sdf_to_coverage when blur is negligible.
        coverage = saturate(0.5 - sd);
    }
    if (coverage <= 0.0)
        discard;
    // Straight-alpha for SRC_ALPHA blend (matches glyph / rounded-rect path).
    return float4(u_color.rgb, u_color.a * coverage);
}
"#,
);

// MSDF_GLYPH_HLSL 是 D3D11 与 D3D12 共用的唯一原生 shader 语义源码。
pub(super) const MSDF_GLYPH_HLSL: &str = r#"
cbuffer MsdfCB : register(b0)
{
    float2 u_viewport;
    float2 u_tex_size;
    float u_range;
    float3 _pad0;
};

Texture2D<float4> u_atlas : register(t0);
SamplerState u_samp : register(s0);

struct VSIn {
    float2 pos : POSITION;
    float2 uv : TEXCOORD0;
    float4 color : COLOR0;
};

struct VSOut {
    float4 pos : SV_POSITION;
    float2 uv : TEXCOORD0;
    float4 color : COLOR0;
};

VSOut VSMain(VSIn input)
{
    VSOut o;
    float2 ndc = (input.pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    o.pos = float4(ndc, 0.0, 1.0);
    o.uv = input.uv;
    o.color = input.color;
    return o;
}

float median3(float a, float b, float c)
{
    return max(min(a, b), min(max(a, b), c));
}

float4 PSMain(VSOut input) : SV_Target
{
    // 共享 Rgba8Unorm 与 linear 契约已保证 encoded 位于单位域。
    float3 encoded = u_atlas.Sample(u_samp, input.uv).rgb;
    float signed_distance = median3(encoded.r, encoded.g, encoded.b) - 0.5;
    float2 uv_derivative = abs(ddx(input.uv)) + abs(ddy(input.uv));
    float2 texture_size = max(u_tex_size, float2(1.0, 1.0));
    float2 unit_range = float2(u_range, u_range) / texture_size;
    float2 screen_texture_size = 1.0 / max(uv_derivative, float2(0.000001, 0.000001));
    float screen_pixel_range = max(0.5 * dot(unit_range, screen_texture_size), 1.0);
    float coverage = saturate(0.5 - signed_distance * screen_pixel_range);
    float coverage_byte = floor(coverage * 255.0 + 0.5);
    // 共享 FramePlan 顶点契约已保证 MSDF tint 位于单位域。
    float4 color = floor(input.color * 255.0 + 0.5);
    float alpha = floor(color.a * coverage_byte / 255.0);
    float3 premul = floor(color.rgb * color.a / 255.0);
    float3 rgb = floor(premul * coverage_byte / 255.0);
    return float4(rgb, alpha) / 255.0;
}
"#;

// SECTOR_HLSL 是 D3D11 与 D3D12 共用的唯一原生 shader 语义源码。
pub(super) const SECTOR_HLSL: &str = r#"
cbuffer SectorCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    float4 u_rect;
    float4 u_color;
    float4 u_angles;
};

struct VSIn {
    float2 pos : POSITION;
};

struct VSOut {
    float4 pos : SV_POSITION;
    float2 local : TEXCOORD0;
    float2 rect_size : TEXCOORD1;
};

VSOut VSMain(VSIn input)
{
    VSOut output;
    float2 pos = u_rect.xy + input.pos * u_rect.zw;
    float2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    output.pos = float4(ndc, 0.0, 1.0);
    output.local = input.pos * u_rect.zw;
    output.rect_size = u_rect.zw;
    return output;
}

float4 PSMain(VSOut input) : SV_Target
{
    const float TAU = 6.283185307179586;
    float2 unit = (input.local / max(input.rect_size, float2(0.0001, 0.0001)) - 0.5) * 2.0;
    float radius = length(unit);
    float radial_width = max(fwidth(radius), 0.0001);
    float radial_mask = saturate((1.0 - radius) / radial_width + 0.5);
    float angular_mask = 1.0;
    if (u_angles.y < TAU - 0.0001 && radius > 0.0001)
    {
        float angle = atan2(unit.y, unit.x);
        if (angle < 0.0)
            angle += TAU;
        float delta = fmod(angle - u_angles.x + TAU, TAU);
        float edge = min(delta, u_angles.y - delta);
        float angular_width = max(fwidth(angle), 0.0001);
        angular_mask = saturate(edge / angular_width + 0.5);
        if (delta > u_angles.y)
            angular_mask = 0.0;
    }
    float mask = radial_mask * angular_mask;
    if (mask <= 0.0)
        discard;
    float4 color = floor(saturate(u_color) * 255.0 + 0.5);
    float3 premul = floor(color.rgb * color.a / 255.0);
    return float4(premul * mask, color.a * mask) / 255.0;
}
"#;

// LINE_HLSL 是 D3D11 与 D3D12 共用的唯一解析抗锯齿线段语义源码。
pub(super) const LINE_HLSL: &str = r#"
cbuffer LineCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    float4 u_points;
    float4 u_color;
    float4 u_params;
};

struct VSIn {
    float2 pos : POSITION;
};

struct VSOut {
    float4 pos : SV_POSITION;
    float2 position : TEXCOORD0;
};

VSOut VSMain(VSIn input)
{
    VSOut output;
    float fringe = max(u_params.x * 0.5, 0.0) + 1.5;
    float2 lower = min(u_points.xy, u_points.zw) - fringe;
    float2 upper = max(u_points.xy, u_points.zw) + fringe;
    float2 position = lerp(lower, upper, input.pos);
    float2 ndc = (position / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    output.pos = float4(ndc, 0.0, 1.0);
    output.position = position;
    return output;
}

float4 PSMain(VSOut input) : SV_Target
{
    float2 start = u_points.xy;
    float2 segment = u_points.zw - start;
    float length_squared = max(dot(segment, segment), 0.000001);
    float along = saturate(dot(input.position - start, segment) / length_squared);
    float distance_to_line = length(input.position - (start + along * segment)) - u_params.x * 0.5;
    float derivative = max(fwidth(distance_to_line), 0.0001);
    float coverage = saturate(0.5 - distance_to_line / derivative);
    if (coverage <= 0.0)
        discard;
    float4 color = floor(saturate(u_color) * 255.0 + 0.5);
    float3 premul = floor(color.rgb * color.a / 255.0);
    return float4(premul * coverage, color.a * coverage) / 255.0;
}
"#;

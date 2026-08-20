//! D3D11 薄 RHI 的 RGBA8 MSDF 字形 shader 源。

// 提供 MSDF median、屏幕导数 AA 与固定 premultiplied color ABI。
pub(crate) const MSDF_GLYPH_HLSL: &str = r#"
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

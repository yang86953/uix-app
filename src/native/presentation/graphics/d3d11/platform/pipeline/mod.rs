//! D3D11 native geometry pipeline — solid/stroke rects + glyph atlas +
//! path meshes + box shadow + soft blit.
//!
//! Hot Canvas2D paths: `fill_rect` / `fill_circle` / `stroke_rect` /
//! `stroke_circle` (+ axis-aligned `draw_line` via solid fill), identity
//! solid `blit_glyph` via coverage atlas, identity linear/radial gradient
//! fills, identity simple `fill_path` / `stroke_path` (CPU tessellate →
//! solid triangles), and identity box/ambient shadow (SDF outer glow).
//! Soft blit for the rest (#169).

#![allow(nonstandard_style)]

use std::{collections::HashMap, ffi::CStr, mem::size_of, sync::Arc};

use crate::core::{Errc, Error, Result};
use crate::native::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuImageBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSector,
    GpuSolidMesh, GpuSolidRect, GpuStrokeRect, SoftFallbackTile,
};
use ::windows::core::PCSTR;
use ::windows::Win32::Foundation::{FALSE, RECT, TRUE};
use ::windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use ::windows::Win32::Graphics::Direct3D::{
    ID3DBlob, D3D11_SRV_DIMENSION_TEXTURE2D, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
};
use ::windows::Win32::Graphics::Direct3D11::{
    ID3D11BlendState, ID3D11Buffer, ID3D11Device, ID3D11DeviceContext, ID3D11InputLayout,
    ID3D11PixelShader, ID3D11RasterizerState, ID3D11RenderTargetView, ID3D11SamplerState,
    ID3D11ShaderResourceView, ID3D11Texture2D, ID3D11VertexShader, D3D11_BIND_CONSTANT_BUFFER,
    D3D11_BIND_SHADER_RESOURCE, D3D11_BIND_VERTEX_BUFFER, D3D11_BLEND_DESC,
    D3D11_BLEND_INV_SRC_ALPHA, D3D11_BLEND_ONE, D3D11_BLEND_OP_ADD, D3D11_BLEND_SRC_ALPHA,
    D3D11_BLEND_ZERO, D3D11_BOX, D3D11_BUFFER_DESC, D3D11_COLOR_WRITE_ENABLE_ALL,
    D3D11_COMPARISON_NEVER, D3D11_CPU_ACCESS_WRITE, D3D11_CULL_NONE, D3D11_FILL_SOLID,
    D3D11_FILTER_MIN_MAG_MIP_POINT, D3D11_INPUT_ELEMENT_DESC, D3D11_INPUT_PER_VERTEX_DATA,
    D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_WRITE_DISCARD, D3D11_RASTERIZER_DESC,
    D3D11_RENDER_TARGET_BLEND_DESC, D3D11_SAMPLER_DESC, D3D11_SHADER_RESOURCE_VIEW_DESC,
    D3D11_SHADER_RESOURCE_VIEW_DESC_0, D3D11_SUBRESOURCE_DATA, D3D11_TEX2D_SRV,
    D3D11_TEXTURE2D_DESC, D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_USAGE_DEFAULT, D3D11_USAGE_DYNAMIC,
    D3D11_VIEWPORT,
};
use ::windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R32G32B32A32_FLOAT, DXGI_FORMAT_R32G32_FLOAT,
    DXGI_FORMAT_R32_UINT, DXGI_FORMAT_R8_UNORM, DXGI_SAMPLE_DESC,
};

const ATLAS_MIN: u32 = 256;
const ATLAS_MAX: u32 = 2048;
const GLYPH_VB_INITIAL_GLYPHS: usize = 256;

const RECT_HLSL: &str = r#"
cbuffer RectCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    float4 u_rect;
    float4 u_color;
    float4 u_radius;
    // x = half stroke width (0 = fill); yzw unused
    float4 u_stroke;
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
    // Stroke expands the draw quad by half_lw + 1px AA fringe (matches CPU SDF).
    float expand = u_stroke.x > 0.0 ? (u_stroke.x + 1.0) : 0.0;
    float2 draw_xy = u_rect.xy - expand;
    float2 draw_wh = u_rect.zw + expand * 2.0;
    float2 pos = draw_xy + input.pos * draw_wh;
    float2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    o.pos = float4(ndc, 0.0, 1.0);
    o.local = input.pos * draw_wh;
    o.rect_size = u_rect.zw;
    return o;
}

// Port of CPU `rounded_rect_sdf` (center-relative, per-corner radius).
float rounded_rect_sdf(float2 local, float2 size, float4 radius)
{
    float2 half_size = size * 0.5;
    float2 q = local - half_size;
    float cr;
    if (q.x < 0.0)
        cr = (q.y < 0.0) ? radius.x : radius.w;
    else
        cr = (q.y < 0.0) ? radius.y : radius.z;
    float2 d = abs(q) - half_size + cr;
    float outside = length(max(d, 0.0));
    float inside = min(max(d.x, d.y), 0.0);
    return outside + inside - cr;
}

float4 PSMain(VSOut input) : SV_Target
{
    float mask;
    if (u_stroke.x > 0.0)
    {
        float expand = u_stroke.x + 1.0;
        // Quad 由 VS 外扩 expand，原点在 rect.xy - (h+1)；减去 1px 后原点
        // 落在 rect.xy - h，即 outer 矩形左上角（outer/inner 中心与 rect
        // 中心重合），避免双 SDF 中心错位。
        float2 shape_local = input.local - float2(1.0, 1.0);
        // 双 SDF（与 CPU 一致）：外扩/内缩 half 使弧线端点对齐像素
        // 中心，消除整数坐标下顶/底圆角起点偏差；中心行 coverage 与 CPU 相同。
        float h = u_stroke.x;
        float2 outer_size = input.rect_size + 2.0 * h;
        float4 outer_rad = u_radius + h;
        float2 inner_size = max(input.rect_size - 2.0 * h, 0.0);
        float4 inner_rad = max(u_radius - h, 0.0);
        float outer_sd = rounded_rect_sdf(shape_local, outer_size, outer_rad);
        float mask;
        if (inner_size.x > 0.0 && inner_size.y > 0.0)
        {
            float inner_sd = rounded_rect_sdf(shape_local, inner_size, inner_rad);
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
"#;

const BLIT_HLSL: &str = r#"
Texture2D u_tex : register(t0);
SamplerState u_samp : register(s0);

cbuffer BlitCB : register(b0)
{
    // xy = source top-left; zw = source size, both normalized to the SRV.
    float4 u_uv_rect;
    // 组 opacity 需要同步缩放 premultiplied RGB 和 alpha。
    float4 u_tint;
};

struct VSIn {
    float2 pos : POSITION;
};

struct VSOut {
    float4 pos : SV_POSITION;
    float2 uv : TEXCOORD0;
};

VSOut VSMain(VSIn input)
{
    VSOut o;
    o.pos = float4(input.pos, 0.0, 1.0);
    // D3D texture (0,0) is top-left; CPU soft buffer is top-left row-major.
    float2 unit_uv = float2(input.pos.x * 0.5 + 0.5, 0.5 - input.pos.y * 0.5);
    o.uv = u_uv_rect.xy + unit_uv * u_uv_rect.zw;
    return o;
}

float4 PSMain(VSOut input) : SV_Target
{
    // 离屏目标已经是 premultiplied 颜色，tint 不得只缩放 alpha。
    return u_tex.Sample(u_samp, input.uv) * u_tint;
}
"#;

const GLYPH_HLSL: &str = r#"
cbuffer GlyphCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
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
    float coverage = floor(saturate(u_atlas.Sample(u_samp, input.uv)) * 255.0 + 0.5);
    float4 color = floor(saturate(input.color) * 255.0 + 0.5);
    float alpha = floor(color.a * coverage / 255.0);
    float3 premul = floor(color.rgb * color.a / 255.0);
    float3 rgb = floor(premul * coverage / 255.0);
    return float4(rgb, alpha) / 255.0;
}
"#;

// 薄 RHI 的通用 sampled quad 像素着色器，顶点阶段复用 glyph 的 position/uv/color ABI。
const RHI_TEXTURED_PS_HLSL: &str = r#"
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
    float4 tint = saturate(input.color);
    return float4(sample.rgb * tint.rgb, sample.a * tint.a);
}
"#;

const GRADIENT_HLSL: &str = r#"
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
    // z = radial outer radius in local space OR linear edge X length
    // w = linear edge Y length
    float4 u_params;
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
        return lerp(u_color_a, u_color_b, t);
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
        return lerp(u_color_a, u_color_b, t);
    }
}
"#;

const MESH_HLSL: &str = r#"
cbuffer MeshCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    float4 u_color;
};

struct VSIn {
    float2 pos : POSITION;
};

struct VSOut {
    float4 pos : SV_POSITION;
};

VSOut VSMain(VSIn input)
{
    VSOut o;
    float2 ndc = (input.pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    o.pos = float4(ndc, 0.0, 1.0);
    return o;
}

float4 PSMain(VSOut input) : SV_Target
{
    return u_color;
}
"#;

// Box / ambient shadow — ports CPU `shadow_coverage` / `shadow_coverage_ambient`.
const BLUR_HLSL: &str = r#"
Texture2D u_tex : register(t0);
SamplerState u_samp : register(s0);

cbuffer BlurCB : register(b0)
{
    // xy = target viewport size; zw = source texture size (physical px)
    float4 u_sizes;
    // xy = target region origin; zw = region size (physical px)
    float4 u_region;
    // xy = pixel sampling direction; z = tap radius (taps = 2r+1)
    float4 u_dir_taps;
    // Gaussian weights ordered [-r..+r], zero-terminated, max 64 taps
    float4 u_weights[16];
};

struct VSIn { float2 pos : POSITION; };
struct VSOut { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; };

VSOut VSMain(VSIn input)
{
    VSOut o;
    o.pos = float4(input.pos, 0.0, 1.0);
    // D3D texture (0,0) is top-left; map the region to the source UV space.
    float2 unit = float2(input.pos.x * 0.5 + 0.5, 0.5 - input.pos.y * 0.5);
    o.uv = (u_region.xy + unit * u_region.zw) / u_sizes.zw;
    return o;
}

// Separable Gaussian: one pass samples along u_dir_taps.xy.
float4 PSMain(VSOut input) : SV_TARGET
{
    float2 step = u_dir_taps.xy / u_sizes.zw;
    int radius = (int)u_dir_taps.z;
    float4 color = 0;
    for (int i = 0; i < 64; ++i)
    {
        float w = u_weights[i / 4][i % 4];
        if (w <= 0.0) break;
        float2 off = step * (float)(i - radius);
        color += w * u_tex.Sample(u_samp, input.uv + off);
    }
    return color;
}
"#;

const SHADOW_HLSL: &str = r#"
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

float rounded_rect_sdf(float2 local, float2 size, float4 radius)
{
    float2 half_size = size * 0.5;
    float2 q = local - half_size;
    float cr;
    if (q.x < 0.0)
        cr = (q.y < 0.0) ? radius.x : radius.w;
    else
        cr = (q.y < 0.0) ? radius.y : radius.z;
    float2 d = abs(q) - half_size + cr;
    float outside = length(max(d, 0.0));
    float inside = min(max(d.x, d.y), 0.0);
    return outside + inside - cr;
}

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
"#;

#[repr(C)]
#[derive(Clone, Copy)]
struct RectConstants {
    viewport: [f32; 2],
    _pad0: [f32; 2],
    rect: [f32; 4],
    color: [f32; 4],
    radius: [f32; 4],
    /// x = half stroke width (0 = fill).
    stroke: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct BlitConstants {
    uv_rect: [f32; 4],
    // 对 premultiplied 离屏采样结果执行组 opacity 缩放。
    tint: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct BlurConstants {
    /// xy = viewport 尺寸；zw = 源纹理尺寸（物理像素）。
    sizes: [f32; 4],
    /// xy = 目标 region 原点；zw = region 尺寸（物理像素）。
    region: [f32; 4],
    /// xy = 采样方向（像素）；z = tap 半径；w 保留。
    dir_taps: [f32; 4],
    /// 高斯权重，按 [-r..+r] 顺序、0 结尾，最多 64 taps。
    weights: [f32; 64],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GlyphConstants {
    viewport: [f32; 2],
    _pad0: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GradientConstants {
    viewport: [f32; 2],
    _pad0: [f32; 2],
    origin_edge_x: [f32; 4],
    edge_y: [f32; 4],
    color_a: [f32; 4],
    color_b: [f32; 4],
    /// x=mode (0 linear / 1 radial), y=dir|inner_r, z=outer_r, w unused.
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MeshConstants {
    viewport: [f32; 2],
    _pad0: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ShadowConstants {
    viewport: [f32; 2],
    _pad0: [f32; 2],
    rect: [f32; 4],
    color: [f32; 4],
    radius: [f32; 4],
    /// x = blur, y = ambient (0/1).
    params: [f32; 4],
}

const MESH_VB_INITIAL_FLOATS: usize = 1024;

#[repr(C)]
#[derive(Clone, Copy)]
struct GlyphVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

struct AtlasCursor {
    x: u32,
    y: u32,
    row_h: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphAtlasKey {
    allocation: usize,
    len: usize,
    width: u32,
    height: u32,
}

impl GlyphAtlasKey {
    fn new(coverage: &Arc<[u8]>, width: u32, height: u32) -> Self {
        Self {
            allocation: Arc::as_ptr(coverage) as *const u8 as usize,
            len: coverage.len(),
            width,
            height,
        }
    }
}

struct GlyphAtlasEntry {
    /// Retaining the allocation prevents an evicted font-cache buffer from
    /// being freed and reusing the pointer while this atlas entry is live.
    _coverage: Arc<[u8]>,
    uv: (f32, f32, f32, f32),
}

struct RasterState {
    viewports: Vec<D3D11_VIEWPORT>,
    scissors: Vec<RECT>,
}

impl RasterState {
    fn capture(context: &ID3D11DeviceContext) -> Self {
        unsafe {
            let mut viewport_count = 0;
            context.RSGetViewports(&mut viewport_count, None);
            let mut viewports = vec![D3D11_VIEWPORT::default(); viewport_count as usize];
            if !viewports.is_empty() {
                context.RSGetViewports(&mut viewport_count, Some(viewports.as_mut_ptr()));
                viewports.truncate(viewport_count as usize);
            }

            let mut scissor_count = 0;
            context.RSGetScissorRects(&mut scissor_count, None);
            let mut scissors = vec![RECT::default(); scissor_count as usize];
            if !scissors.is_empty() {
                context.RSGetScissorRects(&mut scissor_count, Some(scissors.as_mut_ptr()));
                scissors.truncate(scissor_count as usize);
            }

            Self {
                viewports,
                scissors,
            }
        }
    }

    fn restore(&self, context: &ID3D11DeviceContext) {
        unsafe {
            context.RSSetViewports((!self.viewports.is_empty()).then_some(&self.viewports));
            context.RSSetScissorRects((!self.scissors.is_empty()).then_some(&self.scissors));
        }
    }
}

fn d3d_error(operation: &str, err: ::windows::core::Error) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("D3d11Pipeline: {operation} failed: {err}"),
    )
}

fn compile_shader(source: &str, entry: &CStr, target: &CStr) -> Result<ID3DBlob> {
    let mut code = None;
    let mut errors = None;
    let hr = unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            PCSTR::null(),
            None,
            None,
            PCSTR::from_raw(entry.as_ptr().cast()),
            PCSTR::from_raw(target.as_ptr().cast()),
            0,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    if hr.is_err() {
        let detail = errors
            .as_ref()
            .map(|blob| {
                let ptr = unsafe { blob.GetBufferPointer() } as *const u8;
                let len = unsafe { blob.GetBufferSize() };
                if ptr.is_null() || len == 0 {
                    return String::new();
                }
                let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
                String::from_utf8_lossy(bytes).into_owned()
            })
            .unwrap_or_default();
        return Err(Error::new(
            Errc::PlatformError,
            format!("D3d11Pipeline: D3DCompile {entry:?}/{target:?} failed: {detail}"),
        ));
    }
    code.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            format!("D3d11Pipeline: D3DCompile {entry:?}/{target:?} returned no blob"),
        )
    })
}

fn create_static_vb(device: &ID3D11Device, vertices: &[f32]) -> Result<ID3D11Buffer> {
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: std::mem::size_of_val(vertices) as u32,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
        StructureByteStride: 0,
    };
    let data = D3D11_SUBRESOURCE_DATA {
        pSysMem: vertices.as_ptr().cast(),
        SysMemPitch: 0,
        SysMemSlicePitch: 0,
    };
    let mut vb = None;
    unsafe {
        device
            .CreateBuffer(&desc, Some(&data), Some(&mut vb))
            .map_err(|e| d3d_error("CreateBuffer(vb)", e))?;
    }
    vb.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            "D3d11Pipeline: CreateBuffer returned no VB",
        )
    })
}

fn create_dynamic_vb(device: &ID3D11Device, byte_width: usize) -> Result<ID3D11Buffer> {
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: byte_width.max(size_of::<GlyphVertex>() * 6) as u32,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        MiscFlags: 0,
        StructureByteStride: 0,
    };
    let mut vb = None;
    unsafe {
        device
            .CreateBuffer(&desc, None, Some(&mut vb))
            .map_err(|e| d3d_error("CreateBuffer(dynamic vb)", e))?;
    }
    vb.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            "D3d11Pipeline: CreateBuffer(dynamic) returned no VB",
        )
    })
}

fn next_pow2_u32(v: u32) -> u32 {
    v.next_power_of_two().max(1)
}

/// Tight top-left pixel bounds of visible straight-alpha CPU fallback data.
///
/// The native soft texture persists between ordered segments, so callers must
/// upload and sample exactly this box; sampling a fullscreen quad would draw
/// stale texture contents outside the current CPU segment.
pub(crate) fn visible_pixel_bounds(
    pixels: &[u32],
    width: i32,
    height: i32,
) -> Option<(i32, i32, i32, i32)> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let width = width as usize;
    let count = pixels.len().min(width.saturating_mul(height as usize));
    let mut min_x = width;
    let mut min_y = height as usize;
    let mut max_x = 0usize;
    let mut max_y = 0usize;
    let mut any = false;

    for (index, pixel) in pixels.iter().take(count).enumerate() {
        if (pixel >> 24) == 0 {
            continue;
        }
        let x = index % width;
        let y = index / width;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
        any = true;
    }

    any.then(|| {
        (
            min_x as i32,
            min_y as i32,
            (max_x - min_x + 1) as i32,
            (max_y - min_y + 1) as i32,
        )
    })
}

pub struct D3d11Pipeline {
    vs_rect: ID3D11VertexShader,
    ps_rect: ID3D11PixelShader,
    layout: ID3D11InputLayout,
    vs_blit: ID3D11VertexShader,
    ps_blit: ID3D11PixelShader,
    /// 可分离高斯模糊像素着色器。
    ps_blur: ID3D11PixelShader,
    vs_glyph: ID3D11VertexShader,
    ps_glyph: ID3D11PixelShader,
    // 薄 RHI 的 RGBA8 MSDF 字形像素着色器。
    ps_msdf: ID3D11PixelShader,
    // 薄 RHI 通用纹理 quad 的像素着色器。
    ps_rhi_textured: ID3D11PixelShader,
    layout_glyph: ID3D11InputLayout,
    vs_grad: ID3D11VertexShader,
    ps_grad: ID3D11PixelShader,
    vs_mesh: ID3D11VertexShader,
    ps_mesh: ID3D11PixelShader,
    vs_shadow: ID3D11VertexShader,
    ps_shadow: ID3D11PixelShader,
    // 薄 RHI 的原生扇形 VS/PS。
    vs_sector: ID3D11VertexShader,
    ps_sector: ID3D11PixelShader,
    // 兼容层动态 BGRA 图片纹理与 SRV 的所有权。
    image: rhi_image::D3d11ImageOwner,
    vb_unit: ID3D11Buffer,
    vb_fullscreen: ID3D11Buffer,
    vb_glyph: ID3D11Buffer,
    vb_glyph_capacity: usize,
    vb_mesh: ID3D11Buffer,
    vb_mesh_capacity_floats: usize,
    cb: ID3D11Buffer,
    cb_blit: ID3D11Buffer,
    /// 模糊常量缓冲（方向、region、高斯权重）。
    cb_blur: ID3D11Buffer,
    cb_glyph: ID3D11Buffer,
    cb_grad: ID3D11Buffer,
    cb_mesh: ID3D11Buffer,
    cb_shadow: ID3D11Buffer,
    blend_alpha: ID3D11BlendState,
    blend_premultiplied: ID3D11BlendState,
    // sampled Additive quad 使用源与目标都为 ONE 的 blend 状态。
    blend_additive: ID3D11BlendState,
    blend_replace: ID3D11BlendState,
    rasterizer: ID3D11RasterizerState,
    sampler: ID3D11SamplerState,
    soft_tex: Option<ID3D11Texture2D>,
    soft_srv: Option<ID3D11ShaderResourceView>,
    soft_w: i32,
    soft_h: i32,
    atlas_tex: Option<ID3D11Texture2D>,
    atlas_srv: Option<ID3D11ShaderResourceView>,
    atlas_w: u32,
    atlas_h: u32,
    atlas_cursor: AtlasCursor,
    /// Cross-call/cross-frame entries for the live atlas texture. Coverage is
    /// shared, not copied, and total retained exact glyph bytes are bounded by
    /// the atlas packing capacity.
    atlas_cache: HashMap<GlyphAtlasKey, GlyphAtlasEntry>,
    /// Atlas 上传次数统计（诊断）。
    // 该计数仅由 crate 内测试诊断读取，生产 pipeline 不保留额外字段。
    #[cfg(test)]
    atlas_upload_count: usize,
    /// Scratch for packing coverage into atlas rows (R8).
    atlas_upload: Vec<u8>,
    /// Scratch glyph vertices for Map/Draw.
    glyph_verts: Vec<GlyphVertex>,
}

mod pipeline;
mod pipeline2;
mod pipeline3;
// 将 MSDF shader 源拆出，保持 pipeline 主模块不超过文件行数边界。
mod msdf_shader;
mod rhi_blur;
mod rhi_gradient;
mod rhi_shadow;
mod rhi_shape;
// 将扇形 shader 与 draw ABI 拆到独立文件，保持 pipeline 主模块边界清晰。
mod rhi_sector;
// 将兼容层图片上传与 affine textured draw 拆到独立文件。
mod rhi_image;
mod rhi_textured;

// 让 pipeline 构造模块复用 MSDF shader 源而不暴露原生 shader 对象。
pub(super) use msdf_shader::MSDF_GLYPH_HLSL;

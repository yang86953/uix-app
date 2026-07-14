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

use std::mem::size_of;

use crate::core::{Errc, Error, Result};
use crate::native::traits::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSolidMesh,
    GpuSolidRect, GpuStrokeRect, SoftFallbackTile,
};
use ::windows::core::PCSTR;
use ::windows::Win32::Foundation::{FALSE, RECT, TRUE};
use ::windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use ::windows::Win32::Graphics::Direct3D::{
    ID3DBlob, D3D11_SRV_DIMENSION_TEXTURE2D, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
};
use ::windows::Win32::Graphics::Direct3D11::{
    ID3D11BlendState, ID3D11Buffer, ID3D11Device, ID3D11DeviceContext, ID3D11InputLayout,
    ID3D11PixelShader, ID3D11RasterizerState, ID3D11SamplerState, ID3D11ShaderResourceView,
    ID3D11Texture2D, ID3D11VertexShader, D3D11_BIND_CONSTANT_BUFFER, D3D11_BIND_SHADER_RESOURCE,
    D3D11_BIND_VERTEX_BUFFER, D3D11_BLEND_DESC, D3D11_BLEND_INV_SRC_ALPHA, D3D11_BLEND_ONE,
    D3D11_BLEND_OP_ADD, D3D11_BLEND_SRC_ALPHA, D3D11_BLEND_ZERO, D3D11_BOX, D3D11_BUFFER_DESC,
    D3D11_COLOR_WRITE_ENABLE_ALL, D3D11_COMPARISON_NEVER, D3D11_CPU_ACCESS_WRITE, D3D11_CULL_NONE,
    D3D11_FILL_SOLID, D3D11_FILTER_MIN_MAG_MIP_POINT, D3D11_INPUT_ELEMENT_DESC,
    D3D11_INPUT_PER_VERTEX_DATA, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_WRITE_DISCARD,
    D3D11_RASTERIZER_DESC, D3D11_RENDER_TARGET_BLEND_DESC, D3D11_SAMPLER_DESC,
    D3D11_SHADER_RESOURCE_VIEW_DESC, D3D11_SHADER_RESOURCE_VIEW_DESC_0, D3D11_SUBRESOURCE_DATA,
    D3D11_TEX2D_SRV, D3D11_TEXTURE2D_DESC, D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_USAGE_DEFAULT,
    D3D11_USAGE_DYNAMIC, D3D11_VIEWPORT,
};
use ::windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R32G32B32A32_FLOAT, DXGI_FORMAT_R32G32_FLOAT,
    DXGI_FORMAT_R8_UNORM, DXGI_SAMPLE_DESC,
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

float corner_mask(float2 p, float r)
{
    return 1.0 - smoothstep(r - 1.0, r + 1.0, length(p));
}

float rounded_rect_mask(float2 local, float2 size, float4 radius)
{
    float m = 1.0;
    if (radius.x > 0.0 && local.x < radius.x && local.y < radius.x)
        m *= corner_mask(local - float2(radius.x, radius.x), radius.x);
    if (radius.y > 0.0 && local.x > size.x - radius.y && local.y < radius.y)
        m *= corner_mask(local - float2(size.x - radius.y, radius.y), radius.y);
    if (radius.z > 0.0 && local.x > size.x - radius.z && local.y > size.y - radius.z)
        m *= corner_mask(local - float2(size.x - radius.z, size.y - radius.z), radius.z);
    if (radius.w > 0.0 && local.x < radius.w && local.y > size.y - radius.w)
        m *= corner_mask(local - float2(radius.w, size.y - radius.w), radius.w);
    return m;
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
        float2 shape_local = input.local - float2(expand, expand);
        float sd = rounded_rect_sdf(shape_local, input.rect_size, u_radius);
        float stroke_sd = abs(sd) - u_stroke.x;
        // Matches CPU `sdf_to_coverage`: saturate(0.5 - sd).
        mask = saturate(0.5 - stroke_sd);
    }
    else
    {
        mask = rounded_rect_mask(input.local, input.rect_size, u_radius);
    }
    if (mask <= 0.0)
        discard;
    // Straight-alpha output for SRC_ALPHA blend (do not premul RGB by mask).
    return float4(u_color.rgb, u_color.a * mask);
}
"#;

const BLIT_HLSL: &str = r#"
Texture2D u_tex : register(t0);
SamplerState u_samp : register(s0);

cbuffer BlitCB : register(b0)
{
    // xy = source top-left; zw = source size, both normalized to the SRV.
    float4 u_uv_rect;
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
    return u_tex.Sample(u_samp, input.uv);
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
    float a = u_atlas.Sample(u_samp, input.uv);
    // Coverage modulates alpha only — RGB stays straight for SRC_ALPHA blend.
    // Premul (`color * a`) washed glyphs out to near-invisible gray.
    return float4(input.color.rgb, input.color.a * a);
}
"#;

const GRADIENT_HLSL: &str = r#"
cbuffer GradCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    float4 u_rect;      // xy = top-left, zw = size (AABB)
    float4 u_color_a;
    float4 u_color_b;
    // x = mode (0=linear, 1=radial)
    // y = linear dir (0..3) OR radial inner_r
    // z = radial outer_r (unused for linear)
    // w unused
    float4 u_params;
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
    float2 draw_xy = u_rect.xy;
    float2 draw_wh = u_rect.zw;
    float2 pos = draw_xy + input.pos * draw_wh;
    float2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    o.pos = float4(ndc, 0.0, 1.0);
    o.local = input.pos * draw_wh;
    o.rect_size = draw_wh;
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
        float2 size = input.rect_size;
        if (dir < 0.5)
            t = local.x / max(size.x, 1.0);
        else if (dir < 1.5)
            t = local.y / max(size.y, 1.0);
        else if (dir < 2.5)
            t = (local.x + local.y) / max(size.x + size.y, 1.0);
        else
            t = (local.x - local.y + size.y) / max(size.x + size.y, 1.0);
        t = saturate(t);
        return lerp(u_color_a, u_color_b, t);
    }
    else
    {
        // Radial — AABB is (cx-outer, cy-outer, 2*outer, 2*outer).
        float2 center = u_rect.xy + u_rect.zw * 0.5;
        float2 world = u_rect.xy + input.local;
        float dist = length(world - center);
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
const SHADOW_HLSL: &str = r#"
cbuffer ShadowCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    // Shadow shape rect (after offset), not the expanded draw quad.
    float4 u_rect;
    float4 u_color;
    float4 u_radius;
    // x = blur, y = ambient (0/1), zw unused
    float4 u_params;
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
    float blur = max(u_params.x, 0.0);
    float expand = blur + 1.0;
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
    float blur = max(u_params.x, 0.0);
    float expand = blur + 1.0;
    float2 shape_local = input.local - float2(expand, expand);
    float sd = rounded_rect_sdf(shape_local, input.rect_size, u_radius);
    float coverage;
    if (blur > 0.5)
    {
        if (u_params.y > 0.5)
            coverage = shadow_coverage_ambient(sd, blur);
        else
            coverage = shadow_coverage(sd, blur);
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
    rect: [f32; 4],
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

fn compile_shader(source: &str, entry: &str, target: &str) -> Result<ID3DBlob> {
    let mut code = None;
    let mut errors = None;
    let hr = unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            PCSTR::null(),
            None,
            None,
            PCSTR::from_raw(entry.as_ptr()),
            PCSTR::from_raw(target.as_ptr()),
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
            format!("D3d11Pipeline: D3DCompile {entry}/{target} failed: {detail}"),
        ));
    }
    code.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            format!("D3d11Pipeline: D3DCompile {entry}/{target} returned no blob"),
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
    vs_glyph: ID3D11VertexShader,
    ps_glyph: ID3D11PixelShader,
    layout_glyph: ID3D11InputLayout,
    vs_grad: ID3D11VertexShader,
    ps_grad: ID3D11PixelShader,
    vs_mesh: ID3D11VertexShader,
    ps_mesh: ID3D11PixelShader,
    vs_shadow: ID3D11VertexShader,
    ps_shadow: ID3D11PixelShader,
    vb_unit: ID3D11Buffer,
    vb_fullscreen: ID3D11Buffer,
    vb_glyph: ID3D11Buffer,
    vb_glyph_capacity: usize,
    vb_mesh: ID3D11Buffer,
    vb_mesh_capacity_floats: usize,
    cb: ID3D11Buffer,
    cb_blit: ID3D11Buffer,
    cb_glyph: ID3D11Buffer,
    cb_grad: ID3D11Buffer,
    cb_mesh: ID3D11Buffer,
    cb_shadow: ID3D11Buffer,
    blend_alpha: ID3D11BlendState,
    blend_premultiplied: ID3D11BlendState,
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
    /// Scratch for packing coverage into atlas rows (R8).
    atlas_upload: Vec<u8>,
    /// Scratch glyph vertices for Map/Draw.
    glyph_verts: Vec<GlyphVertex>,
}

impl D3d11Pipeline {
    pub(crate) fn new(device: &ID3D11Device) -> Result<Self> {
        let vs_blob = compile_shader(RECT_HLSL, "VSMain\0", "vs_4_0\0")?;
        let ps_blob = compile_shader(RECT_HLSL, "PSMain\0", "ps_4_0\0")?;
        let blit_vs_blob = compile_shader(BLIT_HLSL, "VSMain\0", "vs_4_0\0")?;
        let blit_ps_blob = compile_shader(BLIT_HLSL, "PSMain\0", "ps_4_0\0")?;
        let glyph_vs_blob = compile_shader(GLYPH_HLSL, "VSMain\0", "vs_4_0\0")?;
        let glyph_ps_blob = compile_shader(GLYPH_HLSL, "PSMain\0", "ps_4_0\0")?;
        let grad_vs_blob = compile_shader(GRADIENT_HLSL, "VSMain\0", "vs_4_0\0")?;
        let grad_ps_blob = compile_shader(GRADIENT_HLSL, "PSMain\0", "ps_4_0\0")?;
        let mesh_vs_blob = compile_shader(MESH_HLSL, "VSMain\0", "vs_4_0\0")?;
        let mesh_ps_blob = compile_shader(MESH_HLSL, "PSMain\0", "ps_4_0\0")?;
        let shadow_vs_blob = compile_shader(SHADOW_HLSL, "VSMain\0", "vs_4_0\0")?;
        let shadow_ps_blob = compile_shader(SHADOW_HLSL, "PSMain\0", "ps_4_0\0")?;

        let mut vs_rect = None;
        unsafe {
            device
                .CreateVertexShader(
                    std::slice::from_raw_parts(
                        vs_blob.GetBufferPointer() as *const u8,
                        vs_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut vs_rect),
                )
                .map_err(|e| d3d_error("CreateVertexShader(rect)", e))?;
        }
        let vs_rect =
            vs_rect.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no rect VS"))?;

        let mut ps_rect = None;
        unsafe {
            device
                .CreatePixelShader(
                    std::slice::from_raw_parts(
                        ps_blob.GetBufferPointer() as *const u8,
                        ps_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut ps_rect),
                )
                .map_err(|e| d3d_error("CreatePixelShader(rect)", e))?;
        }
        let ps_rect =
            ps_rect.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no rect PS"))?;

        let mut vs_blit = None;
        unsafe {
            device
                .CreateVertexShader(
                    std::slice::from_raw_parts(
                        blit_vs_blob.GetBufferPointer() as *const u8,
                        blit_vs_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut vs_blit),
                )
                .map_err(|e| d3d_error("CreateVertexShader(blit)", e))?;
        }
        let vs_blit =
            vs_blit.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no blit VS"))?;

        let mut ps_blit = None;
        unsafe {
            device
                .CreatePixelShader(
                    std::slice::from_raw_parts(
                        blit_ps_blob.GetBufferPointer() as *const u8,
                        blit_ps_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut ps_blit),
                )
                .map_err(|e| d3d_error("CreatePixelShader(blit)", e))?;
        }
        let ps_blit =
            ps_blit.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no blit PS"))?;

        let mut vs_glyph = None;
        unsafe {
            device
                .CreateVertexShader(
                    std::slice::from_raw_parts(
                        glyph_vs_blob.GetBufferPointer() as *const u8,
                        glyph_vs_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut vs_glyph),
                )
                .map_err(|e| d3d_error("CreateVertexShader(glyph)", e))?;
        }
        let vs_glyph = vs_glyph
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no glyph VS"))?;

        let mut ps_glyph = None;
        unsafe {
            device
                .CreatePixelShader(
                    std::slice::from_raw_parts(
                        glyph_ps_blob.GetBufferPointer() as *const u8,
                        glyph_ps_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut ps_glyph),
                )
                .map_err(|e| d3d_error("CreatePixelShader(glyph)", e))?;
        }
        let ps_glyph = ps_glyph
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no glyph PS"))?;

        let mut vs_grad = None;
        unsafe {
            device
                .CreateVertexShader(
                    std::slice::from_raw_parts(
                        grad_vs_blob.GetBufferPointer() as *const u8,
                        grad_vs_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut vs_grad),
                )
                .map_err(|e| d3d_error("CreateVertexShader(grad)", e))?;
        }
        let vs_grad =
            vs_grad.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no grad VS"))?;

        let mut ps_grad = None;
        unsafe {
            device
                .CreatePixelShader(
                    std::slice::from_raw_parts(
                        grad_ps_blob.GetBufferPointer() as *const u8,
                        grad_ps_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut ps_grad),
                )
                .map_err(|e| d3d_error("CreatePixelShader(grad)", e))?;
        }
        let ps_grad =
            ps_grad.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no grad PS"))?;

        let mut vs_mesh = None;
        unsafe {
            device
                .CreateVertexShader(
                    std::slice::from_raw_parts(
                        mesh_vs_blob.GetBufferPointer() as *const u8,
                        mesh_vs_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut vs_mesh),
                )
                .map_err(|e| d3d_error("CreateVertexShader(mesh)", e))?;
        }
        let vs_mesh =
            vs_mesh.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no mesh VS"))?;

        let mut ps_mesh = None;
        unsafe {
            device
                .CreatePixelShader(
                    std::slice::from_raw_parts(
                        mesh_ps_blob.GetBufferPointer() as *const u8,
                        mesh_ps_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut ps_mesh),
                )
                .map_err(|e| d3d_error("CreatePixelShader(mesh)", e))?;
        }
        let ps_mesh =
            ps_mesh.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no mesh PS"))?;

        let mut vs_shadow = None;
        unsafe {
            device
                .CreateVertexShader(
                    std::slice::from_raw_parts(
                        shadow_vs_blob.GetBufferPointer() as *const u8,
                        shadow_vs_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut vs_shadow),
                )
                .map_err(|e| d3d_error("CreateVertexShader(shadow)", e))?;
        }
        let vs_shadow = vs_shadow
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no shadow VS"))?;

        let mut ps_shadow = None;
        unsafe {
            device
                .CreatePixelShader(
                    std::slice::from_raw_parts(
                        shadow_ps_blob.GetBufferPointer() as *const u8,
                        shadow_ps_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut ps_shadow),
                )
                .map_err(|e| d3d_error("CreatePixelShader(shadow)", e))?;
        }
        let ps_shadow = ps_shadow
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no shadow PS"))?;

        let input_elems = [D3D11_INPUT_ELEMENT_DESC {
            SemanticName: PCSTR::from_raw(b"POSITION\0".as_ptr()),
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 0,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        }];
        let mut layout = None;
        unsafe {
            device
                .CreateInputLayout(
                    &input_elems,
                    std::slice::from_raw_parts(
                        vs_blob.GetBufferPointer() as *const u8,
                        vs_blob.GetBufferSize(),
                    ),
                    Some(&mut layout),
                )
                .map_err(|e| d3d_error("CreateInputLayout", e))?;
        }
        let layout = layout
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no input layout"))?;

        let glyph_elems = [
            D3D11_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR::from_raw(b"POSITION\0".as_ptr()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 0,
                InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
            D3D11_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR::from_raw(b"TEXCOORD\0".as_ptr()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 8,
                InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
            D3D11_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR::from_raw(b"COLOR\0".as_ptr()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 16,
                InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
        ];
        let mut layout_glyph = None;
        unsafe {
            device
                .CreateInputLayout(
                    &glyph_elems,
                    std::slice::from_raw_parts(
                        glyph_vs_blob.GetBufferPointer() as *const u8,
                        glyph_vs_blob.GetBufferSize(),
                    ),
                    Some(&mut layout_glyph),
                )
                .map_err(|e| d3d_error("CreateInputLayout(glyph)", e))?;
        }
        let layout_glyph = layout_glyph.ok_or_else(|| {
            Error::new(Errc::PlatformError, "D3d11Pipeline: no glyph input layout")
        })?;

        let unit: [f32; 12] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0];
        let fullscreen: [f32; 12] = [
            -1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0,
        ];
        let vb_unit = create_static_vb(device, &unit)?;
        let vb_fullscreen = create_static_vb(device, &fullscreen)?;
        let vb_glyph_capacity = GLYPH_VB_INITIAL_GLYPHS;
        let vb_glyph = create_dynamic_vb(device, vb_glyph_capacity * 6 * size_of::<GlyphVertex>())?;
        let vb_mesh_capacity_floats = MESH_VB_INITIAL_FLOATS;
        let vb_mesh = create_dynamic_vb(device, vb_mesh_capacity_floats * size_of::<f32>())?;

        let cb_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of::<RectConstants>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut cb = None;
        unsafe {
            device
                .CreateBuffer(&cb_desc, None, Some(&mut cb))
                .map_err(|e| d3d_error("CreateBuffer(cb)", e))?;
        }
        let cb = cb.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no CB"))?;

        let cb_blit_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of::<BlitConstants>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut cb_blit = None;
        unsafe {
            device
                .CreateBuffer(&cb_blit_desc, None, Some(&mut cb_blit))
                .map_err(|e| d3d_error("CreateBuffer(cb_blit)", e))?;
        }
        let cb_blit =
            cb_blit.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no blit CB"))?;

        let cb_glyph_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of::<GlyphConstants>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut cb_glyph = None;
        unsafe {
            device
                .CreateBuffer(&cb_glyph_desc, None, Some(&mut cb_glyph))
                .map_err(|e| d3d_error("CreateBuffer(cb_glyph)", e))?;
        }
        let cb_glyph = cb_glyph
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no glyph CB"))?;

        let cb_grad_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of::<GradientConstants>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut cb_grad = None;
        unsafe {
            device
                .CreateBuffer(&cb_grad_desc, None, Some(&mut cb_grad))
                .map_err(|e| d3d_error("CreateBuffer(cb_grad)", e))?;
        }
        let cb_grad =
            cb_grad.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no grad CB"))?;

        let cb_mesh_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of::<MeshConstants>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut cb_mesh = None;
        unsafe {
            device
                .CreateBuffer(&cb_mesh_desc, None, Some(&mut cb_mesh))
                .map_err(|e| d3d_error("CreateBuffer(cb_mesh)", e))?;
        }
        let cb_mesh =
            cb_mesh.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no mesh CB"))?;

        let cb_shadow_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of::<ShadowConstants>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut cb_shadow = None;
        unsafe {
            device
                .CreateBuffer(&cb_shadow_desc, None, Some(&mut cb_shadow))
                .map_err(|e| d3d_error("CreateBuffer(cb_shadow)", e))?;
        }
        let cb_shadow = cb_shadow
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no shadow CB"))?;

        let mut blend_alpha = None;
        let alpha_desc = D3D11_BLEND_DESC {
            AlphaToCoverageEnable: FALSE,
            IndependentBlendEnable: FALSE,
            RenderTarget: [D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: TRUE,
                SrcBlend: D3D11_BLEND_SRC_ALPHA,
                DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            }; 8],
        };
        unsafe {
            device
                .CreateBlendState(&alpha_desc, Some(&mut blend_alpha))
                .map_err(|e| d3d_error("CreateBlendState", e))?;
        }
        let blend_alpha = blend_alpha
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no blend state"))?;

        let mut blend_premultiplied = None;
        let premultiplied_desc = D3D11_BLEND_DESC {
            AlphaToCoverageEnable: FALSE,
            IndependentBlendEnable: FALSE,
            RenderTarget: [D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: TRUE,
                SrcBlend: D3D11_BLEND_ONE,
                DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            }; 8],
        };
        unsafe {
            device
                .CreateBlendState(&premultiplied_desc, Some(&mut blend_premultiplied))
                .map_err(|e| d3d_error("CreateBlendState(premultiplied)", e))?;
        }
        let blend_premultiplied = blend_premultiplied.ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "D3d11Pipeline: no premultiplied blend state",
            )
        })?;

        let mut blend_replace = None;
        let replace_desc = D3D11_BLEND_DESC {
            AlphaToCoverageEnable: FALSE,
            IndependentBlendEnable: FALSE,
            RenderTarget: [D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: FALSE,
                SrcBlend: D3D11_BLEND_ONE,
                DestBlend: D3D11_BLEND_ZERO,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_ZERO,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            }; 8],
        };
        unsafe {
            device
                .CreateBlendState(&replace_desc, Some(&mut blend_replace))
                .map_err(|e| d3d_error("CreateBlendState(replace)", e))?;
        }
        let blend_replace = blend_replace
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no replace blend"))?;

        let mut rasterizer = None;
        let rs_desc = D3D11_RASTERIZER_DESC {
            FillMode: D3D11_FILL_SOLID,
            CullMode: D3D11_CULL_NONE,
            FrontCounterClockwise: FALSE,
            DepthBias: 0,
            DepthBiasClamp: 0.0,
            SlopeScaledDepthBias: 0.0,
            DepthClipEnable: FALSE,
            ScissorEnable: TRUE,
            MultisampleEnable: FALSE,
            AntialiasedLineEnable: FALSE,
        };
        unsafe {
            device
                .CreateRasterizerState(&rs_desc, Some(&mut rasterizer))
                .map_err(|e| d3d_error("CreateRasterizerState", e))?;
        }
        let rasterizer = rasterizer
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no rasterizer"))?;

        let mut sampler = None;
        let samp_desc = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_POINT,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            MipLODBias: 0.0,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            BorderColor: [0.0; 4],
            MinLOD: 0.0,
            MaxLOD: 0.0,
        };
        unsafe {
            device
                .CreateSamplerState(&samp_desc, Some(&mut sampler))
                .map_err(|e| d3d_error("CreateSamplerState", e))?;
        }
        let sampler =
            sampler.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no sampler"))?;

        Ok(Self {
            vs_rect,
            ps_rect,
            layout,
            vs_blit,
            ps_blit,
            vs_glyph,
            ps_glyph,
            layout_glyph,
            vs_grad,
            ps_grad,
            vs_mesh,
            ps_mesh,
            vs_shadow,
            ps_shadow,
            vb_unit,
            vb_fullscreen,
            vb_glyph,
            vb_glyph_capacity,
            vb_mesh,
            vb_mesh_capacity_floats,
            cb,
            cb_blit,
            cb_glyph,
            cb_grad,
            cb_mesh,
            cb_shadow,
            blend_alpha,
            blend_premultiplied,
            blend_replace,
            rasterizer,
            sampler,
            soft_tex: None,
            soft_srv: None,
            soft_w: 0,
            soft_h: 0,
            atlas_tex: None,
            atlas_srv: None,
            atlas_w: 0,
            atlas_h: 0,
            atlas_cursor: AtlasCursor {
                x: 0,
                y: 0,
                row_h: 0,
            },
            atlas_upload: Vec::new(),
            glyph_verts: Vec::new(),
        })
    }

    fn ensure_soft_texture(
        &mut self,
        device: &ID3D11Device,
        width: i32,
        height: i32,
    ) -> Result<()> {
        let w = width.max(1);
        let h = height.max(1);
        if self.soft_tex.is_some() && self.soft_w == w && self.soft_h == h {
            return Ok(());
        }
        self.soft_srv = None;
        self.soft_tex = None;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w as u32,
            Height: h as u32,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut tex = None;
        unsafe {
            device
                .CreateTexture2D(&desc, None, Some(&mut tex))
                .map_err(|e| d3d_error("CreateTexture2D(soft)", e))?;
        }
        let tex =
            tex.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no soft texture"))?;
        let srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                },
            },
        };
        let mut srv = None;
        unsafe {
            device
                .CreateShaderResourceView(&tex, Some(&srv_desc), Some(&mut srv))
                .map_err(|e| d3d_error("CreateShaderResourceView", e))?;
        }
        let srv =
            srv.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no soft SRV"))?;
        self.soft_tex = Some(tex);
        self.soft_srv = Some(srv);
        self.soft_w = w;
        self.soft_h = h;
        Ok(())
    }

    fn ensure_atlas(&mut self, device: &ID3D11Device, need_w: u32, need_h: u32) -> Result<()> {
        let need_w = need_w.max(1);
        let need_h = need_h.max(1);
        let want_w = next_pow2_u32(need_w).clamp(ATLAS_MIN, ATLAS_MAX);
        let want_h = next_pow2_u32(need_h).clamp(ATLAS_MIN, ATLAS_MAX);
        if self.atlas_tex.is_some() && self.atlas_w >= want_w && self.atlas_h >= want_h {
            return Ok(());
        }
        // Grow to at least current size so we don't thrash on mixed glyph sizes.
        let w = next_pow2_u32(self.atlas_w.max(want_w)).clamp(ATLAS_MIN, ATLAS_MAX);
        let h = next_pow2_u32(self.atlas_h.max(want_h)).clamp(ATLAS_MIN, ATLAS_MAX);
        self.atlas_srv = None;
        self.atlas_tex = None;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut tex = None;
        unsafe {
            device
                .CreateTexture2D(&desc, None, Some(&mut tex))
                .map_err(|e| d3d_error("CreateTexture2D(atlas)", e))?;
        }
        let tex =
            tex.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no atlas texture"))?;
        let srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_R8_UNORM,
            ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                },
            },
        };
        let mut srv = None;
        unsafe {
            device
                .CreateShaderResourceView(&tex, Some(&srv_desc), Some(&mut srv))
                .map_err(|e| d3d_error("CreateShaderResourceView(atlas)", e))?;
        }
        let srv =
            srv.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no atlas SRV"))?;
        self.atlas_tex = Some(tex);
        self.atlas_srv = Some(srv);
        self.atlas_w = w;
        self.atlas_h = h;
        self.atlas_cursor = AtlasCursor {
            x: 0,
            y: 0,
            row_h: 0,
        };
        Ok(())
    }

    fn reset_atlas_cursor(&mut self) {
        self.atlas_cursor = AtlasCursor {
            x: 0,
            y: 0,
            row_h: 0,
        };
    }

    fn ensure_glyph_vb(&mut self, device: &ID3D11Device, glyph_count: usize) -> Result<()> {
        if glyph_count <= self.vb_glyph_capacity {
            return Ok(());
        }
        let mut cap = self.vb_glyph_capacity.max(GLYPH_VB_INITIAL_GLYPHS);
        while cap < glyph_count {
            cap = cap.saturating_mul(2);
        }
        self.vb_glyph = create_dynamic_vb(device, cap * 6 * size_of::<GlyphVertex>())?;
        self.vb_glyph_capacity = cap;
        Ok(())
    }

    /// Returns `true` if `(gw, gh)` fits at the current shelf cursor without
    /// wrapping the atlas (may still advance to a new row).
    fn atlas_can_fit(&self, gw: u32, gh: u32) -> bool {
        if self.atlas_tex.is_none() || self.atlas_w == 0 || self.atlas_h == 0 {
            return false;
        }
        let mut x = self.atlas_cursor.x;
        let mut y = self.atlas_cursor.y;
        let mut row_h = self.atlas_cursor.row_h;
        if x + gw > self.atlas_w {
            x = 0;
            y += row_h;
            row_h = 0;
        }
        y + gh <= self.atlas_h && x + gw <= self.atlas_w && row_h.max(gh) <= self.atlas_h
    }

    /// Shelf-pack one glyph; caller must ensure space (flush/grow first).
    fn pack_glyph_unchecked(
        &mut self,
        context: &ID3D11DeviceContext,
        coverage: &[u8],
        cov_w: u32,
        cov_h: u32,
    ) -> Result<(f32, f32, f32, f32)> {
        let gw = cov_w.max(1);
        let gh = cov_h.max(1);
        if self.atlas_cursor.x + gw > self.atlas_w {
            self.atlas_cursor.x = 0;
            self.atlas_cursor.y += self.atlas_cursor.row_h;
            self.atlas_cursor.row_h = 0;
        }
        if self.atlas_cursor.y + gh > self.atlas_h || self.atlas_cursor.x + gw > self.atlas_w {
            return Err(Error::new(
                Errc::PlatformError,
                "D3d11Pipeline: pack_glyph_unchecked called without space",
            ));
        }

        let ax = self.atlas_cursor.x;
        let ay = self.atlas_cursor.y;
        let expected = (gw as usize).saturating_mul(gh as usize);
        if coverage.len() < expected {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "D3d11Pipeline: glyph coverage too small, got {}, need {expected}",
                    coverage.len()
                ),
            ));
        }

        let Some(tex) = self.atlas_tex.as_ref() else {
            return Err(Error::new(
                Errc::PlatformError,
                "D3d11Pipeline: atlas texture missing",
            ));
        };

        self.atlas_upload.clear();
        self.atlas_upload.extend_from_slice(&coverage[..expected]);
        let box_ = D3D11_BOX {
            left: ax,
            top: ay,
            front: 0,
            right: ax + gw,
            bottom: ay + gh,
            back: 1,
        };
        unsafe {
            context.UpdateSubresource(
                tex,
                0,
                Some(&box_),
                self.atlas_upload.as_ptr().cast(),
                gw,
                0,
            );
        }

        self.atlas_cursor.x = ax + gw + 1;
        self.atlas_cursor.row_h = self.atlas_cursor.row_h.max(gh + 1);

        let inv_w = 1.0 / self.atlas_w as f32;
        let inv_h = 1.0 / self.atlas_h as f32;
        Ok((
            ax as f32 * inv_w,
            ay as f32 * inv_h,
            (ax + gw) as f32 * inv_w,
            (ay + gh) as f32 * inv_h,
        ))
    }

    fn push_glyph_quad(
        verts: &mut Vec<GlyphVertex>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        rgba: [f32; 4],
    ) {
        let x1 = x + w;
        let y1 = y + h;
        verts.push(GlyphVertex {
            pos: [x, y],
            uv: [u0, v0],
            color: rgba,
        });
        verts.push(GlyphVertex {
            pos: [x1, y],
            uv: [u1, v0],
            color: rgba,
        });
        verts.push(GlyphVertex {
            pos: [x, y1],
            uv: [u0, v1],
            color: rgba,
        });
        verts.push(GlyphVertex {
            pos: [x, y1],
            uv: [u0, v1],
            color: rgba,
        });
        verts.push(GlyphVertex {
            pos: [x1, y],
            uv: [u1, v0],
            color: rgba,
        });
        verts.push(GlyphVertex {
            pos: [x1, y1],
            uv: [u1, v1],
            color: rgba,
        });
    }

    fn flush_glyph_batch(
        &mut self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
    ) -> Result<()> {
        if self.glyph_verts.is_empty() {
            return Ok(());
        }
        let glyph_count = self.glyph_verts.len() / 6;
        self.ensure_glyph_vb(device, glyph_count)?;
        let Some(srv) = self.atlas_srv.as_ref() else {
            return Err(Error::new(
                Errc::PlatformError,
                "D3d11Pipeline: atlas SRV missing",
            ));
        };

        let constants = GlyphConstants {
            viewport: [viewport_w, viewport_h],
            _pad0: [0.0, 0.0],
        };
        let mut mapped_cb = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            context
                .Map(
                    &self.cb_glyph,
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut mapped_cb),
                )
                .map_err(|e| d3d_error("Map(cb_glyph)", e))?;
            std::ptr::copy_nonoverlapping(
                (&constants as *const GlyphConstants).cast::<u8>(),
                mapped_cb.pData.cast(),
                size_of::<GlyphConstants>(),
            );
            context.Unmap(&self.cb_glyph, 0);
        }

        let bytes = self.glyph_verts.len() * size_of::<GlyphVertex>();
        let mut mapped_vb = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            context
                .Map(
                    &self.vb_glyph,
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut mapped_vb),
                )
                .map_err(|e| d3d_error("Map(vb_glyph)", e))?;
            std::ptr::copy_nonoverlapping(
                self.glyph_verts.as_ptr().cast::<u8>(),
                mapped_vb.pData.cast(),
                bytes,
            );
            context.Unmap(&self.vb_glyph, 0);
        }

        let stride = size_of::<GlyphVertex>() as u32;
        let offset = 0u32;
        let (sx, sy, sw, sh) =
            scissor.unwrap_or((0, 0, viewport_w.ceil() as i32, viewport_h.ceil() as i32));
        let rect = ::windows::Win32::Foundation::RECT {
            left: sx,
            top: sy,
            right: sx + sw.max(0),
            bottom: sy + sh.max(0),
        };
        unsafe {
            context.IASetInputLayout(&self.layout_glyph);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(self.vb_glyph.clone())),
                Some(&stride),
                Some(&offset),
            );
            context.VSSetShader(&self.vs_glyph, None);
            context.PSSetShader(&self.ps_glyph, None);
            context.VSSetConstantBuffers(0, Some(&[Some(self.cb_glyph.clone())]));
            context.PSSetShaderResources(0, Some(&[Some(srv.clone())]));
            context.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            context.RSSetState(&self.rasterizer);
            context.OMSetBlendState(&self.blend_alpha, None, 0xffff_ffff);
            context.RSSetScissorRects(Some(&[rect]));
            context.Draw(self.glyph_verts.len() as u32, 0);
            context.PSSetShaderResources(0, Some(&[None]));
        }
        self.glyph_verts.clear();
        Ok(())
    }

    pub fn draw_glyphs(
        &mut self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<()> {
        if glyphs.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        self.glyph_verts.clear();
        self.reset_atlas_cursor();

        for g in glyphs {
            if g.w <= 0.0 || g.h <= 0.0 || g.cov_w == 0 || g.cov_h == 0 {
                continue;
            }
            let gw = g.cov_w.max(1);
            let gh = g.cov_h.max(1);
            if gw > ATLAS_MAX || gh > ATLAS_MAX {
                return Err(Error::new(
                    Errc::InvalidArgument,
                    format!("D3d11Pipeline: glyph {gw}x{gh} exceeds atlas max {ATLAS_MAX}"),
                ));
            }

            if self.atlas_tex.is_none() {
                self.ensure_atlas(device, gw, gh)?;
            }

            // Growing recreates the texture — flush any verts that still sample it.
            let want_w = next_pow2_u32(gw).clamp(ATLAS_MIN, ATLAS_MAX);
            let want_h = next_pow2_u32(gh).clamp(ATLAS_MIN, ATLAS_MAX);
            if want_w > self.atlas_w || want_h > self.atlas_h {
                self.flush_glyph_batch(device, context, viewport_w, viewport_h, scissor)?;
                self.ensure_atlas(device, gw, gh)?;
            }

            if !self.atlas_can_fit(gw, gh) {
                self.flush_glyph_batch(device, context, viewport_w, viewport_h, scissor)?;
                let grow_h =
                    next_pow2_u32(self.atlas_h.saturating_add(gh)).clamp(ATLAS_MIN, ATLAS_MAX);
                let grow_w = next_pow2_u32(self.atlas_w.max(gw)).clamp(ATLAS_MIN, ATLAS_MAX);
                if grow_h > self.atlas_h || grow_w > self.atlas_w {
                    self.ensure_atlas(device, grow_w, grow_h)?;
                }
                self.reset_atlas_cursor();
                if !self.atlas_can_fit(gw, gh) {
                    return Err(Error::new(
                        Errc::PlatformError,
                        "D3d11Pipeline: glyph does not fit in atlas after grow",
                    ));
                }
            }

            let uv = self.pack_glyph_unchecked(context, &g.coverage, g.cov_w, g.cov_h)?;
            Self::push_glyph_quad(
                &mut self.glyph_verts,
                g.x,
                g.y,
                g.w,
                g.h,
                uv.0,
                uv.1,
                uv.2,
                uv.3,
                g.rgba,
            );
        }
        self.flush_glyph_batch(device, context, viewport_w, viewport_h, scissor)
    }

    fn bind_rect_pipeline(
        &self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        replace_blend: bool,
    ) {
        let stride = (2 * size_of::<f32>()) as u32;
        let offset = 0u32;
        unsafe {
            context.IASetInputLayout(&self.layout);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(self.vb_unit.clone())),
                Some(&stride),
                Some(&offset),
            );
            context.VSSetShader(&self.vs_rect, None);
            context.PSSetShader(&self.ps_rect, None);
            context.VSSetConstantBuffers(0, Some(&[Some(self.cb.clone())]));
            context.PSSetConstantBuffers(0, Some(&[Some(self.cb.clone())]));
            context.RSSetState(&self.rasterizer);
            if replace_blend {
                context.OMSetBlendState(&self.blend_replace, None, 0xffff_ffff);
            } else {
                context.OMSetBlendState(&self.blend_alpha, None, 0xffff_ffff);
            }
            let (sx, sy, sw, sh) =
                scissor.unwrap_or((0, 0, viewport_w.ceil() as i32, viewport_h.ceil() as i32));
            let rect = ::windows::Win32::Foundation::RECT {
                left: sx,
                top: sy,
                right: sx + sw.max(0),
                bottom: sy + sh.max(0),
            };
            context.RSSetScissorRects(Some(&[rect]));
        }
    }

    fn draw_rect_constants(
        &self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        rgba: [f32; 4],
        radius: [f32; 4],
        half_stroke: f32,
    ) -> Result<()> {
        if w <= 0.0 || h <= 0.0 {
            return Ok(());
        }
        let constants = RectConstants {
            viewport: [viewport_w, viewport_h],
            _pad0: [0.0, 0.0],
            rect: [x, y, w, h],
            color: rgba,
            radius,
            stroke: [half_stroke.max(0.0), 0.0, 0.0, 0.0],
        };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            context
                .Map(&self.cb, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))
                .map_err(|e| d3d_error("Map(cb)", e))?;
            std::ptr::copy_nonoverlapping(
                (&constants as *const RectConstants).cast::<u8>(),
                mapped.pData.cast(),
                size_of::<RectConstants>(),
            );
            context.Unmap(&self.cb, 0);
            context.Draw(6, 0);
        }
        Ok(())
    }

    pub fn draw_solid_rects(
        &mut self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuSolidRect],
    ) -> Result<()> {
        if rects.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        self.bind_rect_pipeline(context, viewport_w, viewport_h, scissor, false);
        for rect in rects {
            self.draw_rect_constants(
                context,
                viewport_w,
                viewport_h,
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                rect.rgba,
                rect.radius,
                0.0,
            )?;
        }
        Ok(())
    }

    pub fn draw_stroke_rects(
        &mut self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuStrokeRect],
    ) -> Result<()> {
        if rects.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        self.bind_rect_pipeline(context, viewport_w, viewport_h, scissor, false);
        for rect in rects {
            let half = rect.line_width.max(0.0) * 0.5;
            if half <= 0.0 {
                continue;
            }
            self.draw_rect_constants(
                context,
                viewport_w,
                viewport_h,
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                rect.rgba,
                rect.radius,
                half,
            )?;
        }
        Ok(())
    }

    /// Replace-blend clear quads (partial dirty clear; D3D ClearRTV is full-surface).
    pub fn clear_rects(
        &mut self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        rects: &[GpuSolidRect],
    ) -> Result<()> {
        if rects.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        self.bind_rect_pipeline(context, viewport_w, viewport_h, None, true);
        for rect in rects {
            self.draw_rect_constants(
                context,
                viewport_w,
                viewport_h,
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                rect.rgba,
                rect.radius,
                0.0,
            )?;
        }
        Ok(())
    }

    fn bind_grad_pipeline(
        &self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
    ) {
        let stride = (2 * size_of::<f32>()) as u32;
        let offset = 0u32;
        unsafe {
            context.IASetInputLayout(&self.layout);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(self.vb_unit.clone())),
                Some(&stride),
                Some(&offset),
            );
            context.VSSetShader(&self.vs_grad, None);
            context.PSSetShader(&self.ps_grad, None);
            context.VSSetConstantBuffers(0, Some(&[Some(self.cb_grad.clone())]));
            context.PSSetConstantBuffers(0, Some(&[Some(self.cb_grad.clone())]));
            context.RSSetState(&self.rasterizer);
            context.OMSetBlendState(&self.blend_alpha, None, 0xffff_ffff);
            let (sx, sy, sw, sh) =
                scissor.unwrap_or((0, 0, viewport_w.ceil() as i32, viewport_h.ceil() as i32));
            let rect = ::windows::Win32::Foundation::RECT {
                left: sx,
                top: sy,
                right: sx + sw.max(0),
                bottom: sy + sh.max(0),
            };
            context.RSSetScissorRects(Some(&[rect]));
        }
    }

    fn draw_grad_constants(
        &self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color_a: [f32; 4],
        color_b: [f32; 4],
        params: [f32; 4],
    ) -> Result<()> {
        if w <= 0.0 || h <= 0.0 {
            return Ok(());
        }
        let constants = GradientConstants {
            viewport: [viewport_w, viewport_h],
            _pad0: [0.0, 0.0],
            rect: [x, y, w, h],
            color_a,
            color_b,
            params,
        };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            context
                .Map(
                    &self.cb_grad,
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut mapped),
                )
                .map_err(|e| d3d_error("Map(cb_grad)", e))?;
            std::ptr::copy_nonoverlapping(
                (&constants as *const GradientConstants).cast::<u8>(),
                mapped.pData.cast(),
                size_of::<GradientConstants>(),
            );
            context.Unmap(&self.cb_grad, 0);
            context.Draw(6, 0);
        }
        Ok(())
    }

    pub fn draw_linear_gradients(
        &mut self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuLinearGradientRect],
    ) -> Result<()> {
        if rects.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        self.bind_grad_pipeline(context, viewport_w, viewport_h, scissor);
        for rect in rects {
            if rect.w <= 0.0 || rect.h <= 0.0 {
                continue;
            }
            self.draw_grad_constants(
                context,
                viewport_w,
                viewport_h,
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                rect.color_a,
                rect.color_b,
                [0.0, rect.dir as f32, 0.0, 0.0],
            )?;
        }
        Ok(())
    }

    pub fn draw_radial_gradients(
        &mut self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        grads: &[GpuRadialGradient],
    ) -> Result<()> {
        if grads.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        self.bind_grad_pipeline(context, viewport_w, viewport_h, scissor);
        for g in grads {
            let outer = g.outer_r.max(0.0);
            if outer <= 0.0 {
                continue;
            }
            let x = g.cx - outer;
            let y = g.cy - outer;
            let size = outer * 2.0;
            self.draw_grad_constants(
                context,
                viewport_w,
                viewport_h,
                x,
                y,
                size,
                size,
                g.color_inner,
                g.color_outer,
                [1.0, g.inner_r.max(0.0), outer, 0.0],
            )?;
        }
        Ok(())
    }

    fn ensure_mesh_vb(&mut self, device: &ID3D11Device, float_count: usize) -> Result<()> {
        if float_count <= self.vb_mesh_capacity_floats {
            return Ok(());
        }
        let mut cap = self.vb_mesh_capacity_floats.max(MESH_VB_INITIAL_FLOATS);
        while cap < float_count {
            cap = cap.saturating_mul(2);
        }
        self.vb_mesh = create_dynamic_vb(device, cap * size_of::<f32>())?;
        self.vb_mesh_capacity_floats = cap;
        Ok(())
    }

    /// Draw CPU-tessellated solid triangle meshes (path fill/stroke).
    pub fn draw_solid_meshes(
        &mut self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        meshes: &[GpuSolidMesh],
    ) -> Result<()> {
        if meshes.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        let (sx, sy, sw, sh) =
            scissor.unwrap_or((0, 0, viewport_w.ceil() as i32, viewport_h.ceil() as i32));
        let scissor_rect = ::windows::Win32::Foundation::RECT {
            left: sx,
            top: sy,
            right: sx + sw.max(0),
            bottom: sy + sh.max(0),
        };
        for mesh in meshes {
            let verts = mesh.vertices.as_ref();
            if verts.len() < 6 || verts.len() % 2 != 0 {
                continue;
            }
            let vert_count = (verts.len() / 2) as u32;
            if !vert_count.is_multiple_of(3) {
                continue;
            }
            self.ensure_mesh_vb(device, verts.len())?;
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                context
                    .Map(
                        &self.vb_mesh,
                        0,
                        D3D11_MAP_WRITE_DISCARD,
                        0,
                        Some(&mut mapped),
                    )
                    .map_err(|e| d3d_error("Map(vb_mesh)", e))?;
                std::ptr::copy_nonoverlapping(
                    verts.as_ptr().cast::<u8>(),
                    mapped.pData.cast(),
                    std::mem::size_of_val(verts),
                );
                context.Unmap(&self.vb_mesh, 0);
            }
            let constants = MeshConstants {
                viewport: [viewport_w, viewport_h],
                _pad0: [0.0, 0.0],
                color: mesh.rgba,
            };
            let mut mapped_cb = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                context
                    .Map(
                        &self.cb_mesh,
                        0,
                        D3D11_MAP_WRITE_DISCARD,
                        0,
                        Some(&mut mapped_cb),
                    )
                    .map_err(|e| d3d_error("Map(cb_mesh)", e))?;
                std::ptr::copy_nonoverlapping(
                    (&constants as *const MeshConstants).cast::<u8>(),
                    mapped_cb.pData.cast(),
                    size_of::<MeshConstants>(),
                );
                context.Unmap(&self.cb_mesh, 0);

                let stride = (2 * size_of::<f32>()) as u32;
                let offset = 0u32;
                context.IASetInputLayout(&self.layout);
                context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                context.IASetVertexBuffers(
                    0,
                    1,
                    Some(&Some(self.vb_mesh.clone())),
                    Some(&stride),
                    Some(&offset),
                );
                context.VSSetShader(&self.vs_mesh, None);
                context.PSSetShader(&self.ps_mesh, None);
                context.VSSetConstantBuffers(0, Some(&[Some(self.cb_mesh.clone())]));
                context.PSSetConstantBuffers(0, Some(&[Some(self.cb_mesh.clone())]));
                context.RSSetState(&self.rasterizer);
                context.OMSetBlendState(&self.blend_alpha, None, 0xffff_ffff);
                context.RSSetScissorRects(Some(&[scissor_rect]));
                context.Draw(vert_count, 0);
            }
        }
        Ok(())
    }

    /// Draw axis-aligned box / ambient shadows (SDF coverage, matches CPU).
    pub fn draw_box_shadows(
        &mut self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        shadows: &[GpuBoxShadow],
    ) -> Result<()> {
        if shadows.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        let (sx, sy, sw, sh) =
            scissor.unwrap_or((0, 0, viewport_w.ceil() as i32, viewport_h.ceil() as i32));
        let scissor_rect = ::windows::Win32::Foundation::RECT {
            left: sx,
            top: sy,
            right: sx + sw.max(0),
            bottom: sy + sh.max(0),
        };
        let stride = (2 * size_of::<f32>()) as u32;
        let offset = 0u32;
        unsafe {
            context.IASetInputLayout(&self.layout);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(self.vb_unit.clone())),
                Some(&stride),
                Some(&offset),
            );
            context.VSSetShader(&self.vs_shadow, None);
            context.PSSetShader(&self.ps_shadow, None);
            context.VSSetConstantBuffers(0, Some(&[Some(self.cb_shadow.clone())]));
            context.PSSetConstantBuffers(0, Some(&[Some(self.cb_shadow.clone())]));
            context.RSSetState(&self.rasterizer);
            context.OMSetBlendState(&self.blend_alpha, None, 0xffff_ffff);
            context.RSSetScissorRects(Some(&[scissor_rect]));
        }
        for shadow in shadows {
            if shadow.w <= 0.0 || shadow.h <= 0.0 || shadow.rgba[3] <= 0.0 {
                continue;
            }
            let blur = shadow.blur.max(0.0);
            let constants = ShadowConstants {
                viewport: [viewport_w, viewport_h],
                _pad0: [0.0, 0.0],
                rect: [
                    shadow.x + shadow.offset_x,
                    shadow.y + shadow.offset_y,
                    shadow.w,
                    shadow.h,
                ],
                color: shadow.rgba,
                radius: shadow.radius,
                params: [blur, if shadow.ambient { 1.0 } else { 0.0 }, 0.0, 0.0],
            };
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                context
                    .Map(
                        &self.cb_shadow,
                        0,
                        D3D11_MAP_WRITE_DISCARD,
                        0,
                        Some(&mut mapped),
                    )
                    .map_err(|e| d3d_error("Map(cb_shadow)", e))?;
                std::ptr::copy_nonoverlapping(
                    (&constants as *const ShadowConstants).cast::<u8>(),
                    mapped.pData.cast(),
                    size_of::<ShadowConstants>(),
                );
                context.Unmap(&self.cb_shadow, 0);
                context.Draw(6, 0);
            }
        }
        Ok(())
    }

    pub fn blit_soft_fallback(
        &mut self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        pixels: &[u32],
        width: i32,
        height: i32,
    ) -> Result<()> {
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let expected = (width as usize).saturating_mul(height as usize);
        if pixels.len() < expected {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "D3d11Pipeline: soft blit buffer too small, got {}, need {expected}",
                    pixels.len()
                ),
            ));
        }
        let Some((x, y, upload_w, upload_h)) = visible_pixel_bounds(pixels, width, height) else {
            return Ok(());
        };
        let mut packed = Vec::with_capacity((upload_w as usize).saturating_mul(upload_h as usize));
        for row in y..y + upload_h {
            let start = row as usize * width as usize + x as usize;
            packed.extend_from_slice(&pixels[start..start + upload_w as usize]);
        }
        self.blit_soft_fallback_tile(
            device,
            context,
            &packed,
            width,
            height,
            SoftFallbackTile::at_destination(x, y, upload_w, upload_h),
        )
    }

    pub fn blit_soft_fallback_tile(
        &mut self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        pixels: &[u32],
        target_width: i32,
        target_height: i32,
        tile: SoftFallbackTile,
    ) -> Result<()> {
        if target_width <= 0 || target_height <= 0 {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("D3d11Pipeline: invalid soft target {target_width}x{target_height}"),
            ));
        }
        tile.validate_payload(pixels)?;
        if tile.dst_x.saturating_add(tile.width) > target_width
            || tile.dst_y.saturating_add(tile.height) > target_height
        {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "D3d11Pipeline: soft tile {}x{} at {},{} exceeds {target_width}x{target_height}",
                    tile.width, tile.height, tile.dst_x, tile.dst_y
                ),
            ));
        }
        let upload_w = tile.width;
        let upload_h = tile.height;
        self.ensure_soft_texture(device, target_width, target_height)?;
        let Some(tex) = self.soft_tex.as_ref() else {
            return Err(Error::new(
                Errc::PlatformError,
                "D3d11Pipeline: soft texture missing",
            ));
        };
        let Some(srv) = self.soft_srv.as_ref() else {
            return Err(Error::new(
                Errc::PlatformError,
                "D3d11Pipeline: soft SRV missing",
            ));
        };
        let constants = BlitConstants {
            uv_rect: [
                tile.dst_x as f32 / target_width as f32,
                tile.dst_y as f32 / target_height as f32,
                upload_w as f32 / target_width as f32,
                upload_h as f32 / target_height as f32,
            ],
        };
        let upload_box = D3D11_BOX {
            left: tile.dst_x as u32,
            top: tile.dst_y as u32,
            front: 0,
            right: (tile.dst_x + upload_w) as u32,
            bottom: (tile.dst_y + upload_h) as u32,
            back: 1,
        };

        unsafe {
            context.UpdateSubresource(
                tex,
                0,
                Some(&upload_box),
                pixels.as_ptr().cast(),
                (upload_w as u32) * 4,
                0,
            );

            // Native draws before this segment may have narrowed the D3D
            // scissor to a ScrollView. Set the exact soft bounds explicitly,
            // both to avoid inheriting that state and to avoid sampling stale
            // pixels from prior ordered CPU segments.
            let viewport = D3D11_VIEWPORT {
                TopLeftX: tile.dst_x as f32,
                TopLeftY: tile.dst_y as f32,
                Width: upload_w as f32,
                Height: upload_h as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            };
            let scissor = ::windows::Win32::Foundation::RECT {
                left: tile.dst_x,
                top: tile.dst_y,
                right: tile.dst_x + upload_w,
                bottom: tile.dst_y + upload_h,
            };
            context.RSSetViewports(Some(&[viewport]));
            context.RSSetScissorRects(Some(&[scissor]));

            let stride = (2 * size_of::<f32>()) as u32;
            let offset = 0u32;
            context.IASetInputLayout(&self.layout);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(self.vb_fullscreen.clone())),
                Some(&stride),
                Some(&offset),
            );
            context.VSSetShader(&self.vs_blit, None);
            context.PSSetShader(&self.ps_blit, None);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            context
                .Map(
                    &self.cb_blit,
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut mapped),
                )
                .map_err(|e| d3d_error("Map(cb_blit soft fallback)", e))?;
            std::ptr::copy_nonoverlapping(
                (&constants as *const BlitConstants).cast::<u8>(),
                mapped.pData.cast(),
                size_of::<BlitConstants>(),
            );
            context.Unmap(&self.cb_blit, 0);
            context.VSSetConstantBuffers(0, Some(&[Some(self.cb_blit.clone())]));
            context.PSSetShaderResources(0, Some(&[Some(srv.clone())]));
            context.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            context.RSSetState(&self.rasterizer);
            // CPU fallback pixels use AARRGGBB premultiplied-alpha storage.
            // Applying SRC_ALPHA here would multiply their RGB a second time.
            context.OMSetBlendState(&self.blend_premultiplied, None, 0xffff_ffff);
            context.Draw(6, 0);
            // Unbind SRV so the texture can be updated next frame.
            context.PSSetShaderResources(0, Some(&[None]));
        }
        Ok(())
    }

    /// Stretch-sample `srv` into `dst` on the current RT (full texture → dst rect).
    ///
    /// Sets a temporary viewport to `dst` and draws the fullscreen blit quad so the
    /// entire SRV covers that rect. Caller must not have `srv`'s texture bound as RTV.
    pub fn blit_srv_to_rect(
        &self,
        context: &ID3D11DeviceContext,
        srv: &ID3D11ShaderResourceView,
        source_w: f32,
        source_h: f32,
        target_w: f32,
        target_h: f32,
        src: crate::core::Rect,
        dst: crate::core::Rect,
    ) -> Result<()> {
        if dst.w <= 0.0
            || dst.h <= 0.0
            || source_w <= 0.0
            || source_h <= 0.0
            || target_w <= 0.0
            || target_h <= 0.0
        {
            return Ok(());
        }
        let src_right = src.x + src.w;
        let src_bottom = src.y + src.h;
        if !src.x.is_finite()
            || !src.y.is_finite()
            || !src.w.is_finite()
            || !src.h.is_finite()
            || src.w <= 0.0
            || src.h <= 0.0
            || src.x < 0.0
            || src.y < 0.0
            || src_right > source_w
            || src_bottom > source_h
        {
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3d11Pipeline: source crop lies outside the offscreen target",
            ));
        }
        let vp = D3D11_VIEWPORT {
            TopLeftX: dst.x,
            TopLeftY: dst.y,
            Width: dst.w.max(0.0),
            Height: dst.h.max(0.0),
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        let full_scissor = RECT {
            left: 0,
            top: 0,
            right: target_w.ceil() as i32,
            bottom: target_h.ceil() as i32,
        };
        let constants = BlitConstants {
            uv_rect: [
                src.x / source_w,
                src.y / source_h,
                src.w / source_w,
                src.h / source_h,
            ],
        };
        let previous_raster_state = RasterState::capture(context);
        let result = (|| -> Result<()> {
            unsafe {
                context.RSSetViewports(Some(&[vp]));
                context.RSSetScissorRects(Some(&[full_scissor]));
                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                context
                    .Map(
                        &self.cb_blit,
                        0,
                        D3D11_MAP_WRITE_DISCARD,
                        0,
                        Some(&mut mapped),
                    )
                    .map_err(|e| d3d_error("Map(cb_blit)", e))?;
                std::ptr::copy_nonoverlapping(
                    (&constants as *const BlitConstants).cast::<u8>(),
                    mapped.pData.cast(),
                    size_of::<BlitConstants>(),
                );
                context.Unmap(&self.cb_blit, 0);
                let stride = (2 * size_of::<f32>()) as u32;
                let offset = 0u32;
                context.IASetInputLayout(&self.layout);
                context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                context.IASetVertexBuffers(
                    0,
                    1,
                    Some(&Some(self.vb_fullscreen.clone())),
                    Some(&stride),
                    Some(&offset),
                );
                context.VSSetShader(&self.vs_blit, None);
                context.PSSetShader(&self.ps_blit, None);
                context.VSSetConstantBuffers(0, Some(&[Some(self.cb_blit.clone())]));
                context.PSSetShaderResources(0, Some(&[Some(srv.clone())]));
                context.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
                context.RSSetState(&self.rasterizer);
                context.OMSetBlendState(&self.blend_alpha, None, 0xffff_ffff);
                context.Draw(6, 0);
                context.PSSetShaderResources(0, Some(&[None]));
            }
            Ok(())
        })();
        previous_raster_state.restore(context);
        result
    }
}

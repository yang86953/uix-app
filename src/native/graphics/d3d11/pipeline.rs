//! D3D11 native geometry pipeline — solid/stroke rects + glyph atlas + soft blit.
//!
//! Hot Canvas2D paths: `fill_rect` / `fill_circle` / `stroke_rect` /
//! `stroke_circle` (+ axis-aligned `draw_line` via solid fill) and identity
//! solid `blit_glyph` via coverage atlas. Soft blit for the rest (#169).

#![cfg(windows)]
#![allow(nonstandard_style)]

use std::mem::size_of;

use crate::core::{Errc, Error, Result};
use crate::native::traits::present::{GpuGlyphBlit, GpuSolidRect, GpuStrokeRect};
use ::windows::core::PCSTR;
use ::windows::Win32::Foundation::{FALSE, TRUE};
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
    D3D11_COLOR_WRITE_ENABLE_ALL, D3D11_COMPARISON_NEVER, D3D11_CPU_ACCESS_WRITE,
    D3D11_CULL_NONE, D3D11_FILL_SOLID, D3D11_FILTER_MIN_MAG_MIP_POINT, D3D11_INPUT_ELEMENT_DESC,
    D3D11_INPUT_PER_VERTEX_DATA, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_WRITE_DISCARD,
    D3D11_RASTERIZER_DESC, D3D11_RENDER_TARGET_BLEND_DESC, D3D11_SAMPLER_DESC,
    D3D11_SHADER_RESOURCE_VIEW_DESC, D3D11_SHADER_RESOURCE_VIEW_DESC_0, D3D11_SUBRESOURCE_DATA,
    D3D11_TEX2D_SRV, D3D11_TEXTURE2D_DESC, D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_USAGE_DEFAULT,
    D3D11_USAGE_DYNAMIC,
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
    return u_color * mask;
}
"#;

const BLIT_HLSL: &str = r#"
Texture2D u_tex : register(t0);
SamplerState u_samp : register(s0);

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
    o.uv = float2(input.pos.x * 0.5 + 0.5, 0.5 - input.pos.y * 0.5);
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
    return input.color * a;
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
struct GlyphConstants {
    viewport: [f32; 2],
    _pad0: [f32; 2],
}

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
        ByteWidth: (vertices.len() * size_of::<f32>()) as u32,
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
    vb.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: CreateBuffer returned no VB"))
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

pub struct D3d11Pipeline {
    vs_rect: ID3D11VertexShader,
    ps_rect: ID3D11PixelShader,
    layout: ID3D11InputLayout,
    vs_blit: ID3D11VertexShader,
    ps_blit: ID3D11PixelShader,
    vs_glyph: ID3D11VertexShader,
    ps_glyph: ID3D11PixelShader,
    layout_glyph: ID3D11InputLayout,
    vb_unit: ID3D11Buffer,
    vb_fullscreen: ID3D11Buffer,
    vb_glyph: ID3D11Buffer,
    vb_glyph_capacity: usize,
    cb: ID3D11Buffer,
    cb_glyph: ID3D11Buffer,
    blend_alpha: ID3D11BlendState,
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
    pub fn new(device: &ID3D11Device) -> Result<Self> {
        let vs_blob = compile_shader(RECT_HLSL, "VSMain\0", "vs_4_0\0")?;
        let ps_blob = compile_shader(RECT_HLSL, "PSMain\0", "ps_4_0\0")?;
        let blit_vs_blob = compile_shader(BLIT_HLSL, "VSMain\0", "vs_4_0\0")?;
        let blit_ps_blob = compile_shader(BLIT_HLSL, "PSMain\0", "ps_4_0\0")?;
        let glyph_vs_blob = compile_shader(GLYPH_HLSL, "VSMain\0", "vs_4_0\0")?;
        let glyph_ps_blob = compile_shader(GLYPH_HLSL, "PSMain\0", "ps_4_0\0")?;

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
        let vs_rect = vs_rect
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no rect VS"))?;

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
        let ps_rect = ps_rect
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no rect PS"))?;

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
        let vs_blit = vs_blit
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no blit VS"))?;

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
        let ps_blit = ps_blit
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no blit PS"))?;

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
        let vb_glyph = create_dynamic_vb(
            device,
            vb_glyph_capacity * 6 * size_of::<GlyphVertex>(),
        )?;

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
        let sampler = sampler
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no sampler"))?;

        Ok(Self {
            vs_rect,
            ps_rect,
            layout,
            vs_blit,
            ps_blit,
            vs_glyph,
            ps_glyph,
            layout_glyph,
            vb_unit,
            vb_fullscreen,
            vb_glyph,
            vb_glyph_capacity,
            cb,
            cb_glyph,
            blend_alpha,
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

    fn ensure_soft_texture(&mut self, device: &ID3D11Device, width: i32, height: i32) -> Result<()> {
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
        let tex = tex.ok_or_else(|| {
            Error::new(Errc::PlatformError, "D3d11Pipeline: no soft texture")
        })?;
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
        let srv = srv.ok_or_else(|| {
            Error::new(Errc::PlatformError, "D3d11Pipeline: no soft SRV")
        })?;
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
        let tex = tex.ok_or_else(|| {
            Error::new(Errc::PlatformError, "D3d11Pipeline: no atlas texture")
        })?;
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
        let srv = srv.ok_or_else(|| {
            Error::new(Errc::PlatformError, "D3d11Pipeline: no atlas SRV")
        })?;
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
        let (sx, sy, sw, sh) = scissor.unwrap_or((
            0,
            0,
            viewport_w.ceil() as i32,
            viewport_h.ceil() as i32,
        ));
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
            let (sx, sy, sw, sh) = scissor.unwrap_or((
                0,
                0,
                viewport_w.ceil() as i32,
                viewport_h.ceil() as i32,
            ));
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
                .Map(
                    &self.cb,
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut mapped),
                )
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
        self.ensure_soft_texture(device, width, height)?;
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

        unsafe {
            context.UpdateSubresource(
                tex,
                0,
                None,
                pixels.as_ptr().cast(),
                (width as u32) * 4,
                0,
            );

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
            context.PSSetShaderResources(0, Some(&[Some(srv.clone())]));
            context.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            context.RSSetState(&self.rasterizer);
            context.OMSetBlendState(&self.blend_alpha, None, 0xffff_ffff);
            context.Draw(6, 0);
            // Unbind SRV so the texture can be updated next frame.
            context.PSSetShaderResources(0, Some(&[None]));
        }
        Ok(())
    }
}

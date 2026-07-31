//! Minimal D3D12 native raster pipeline: rounded solid rectangles + soft fallback.

#![allow(nonstandard_style)]

use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::{size_of, ManuallyDrop};
use std::sync::Arc;

use crate::core::{Errc, Error, Result};
use crate::native::present::{GpuGlyphBlit, GpuSolidRect, SoftFallbackTile};
use ::windows::core::PCSTR;
use ::windows::Win32::Foundation::{FALSE, RECT, TRUE};
use ::windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use ::windows::Win32::Graphics::Direct3D::{ID3DBlob, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST};
use ::windows::Win32::Graphics::Direct3D12::*;
use ::windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R32G32B32A32_FLOAT, DXGI_FORMAT_R32G32_FLOAT,
    DXGI_FORMAT_R8_UNORM, DXGI_SAMPLE_DESC,
};

use super::transfer::{
    record_transition, release_copy_location, texture_copy_location_footprint,
    texture_copy_location_subresource,
};

const RECT_ROOT_DWORDS: u32 = 20;
const GLYPH_ATLAS_SIZE: u32 = 2048;
const GLYPH_RETAINED_UPLOAD_LIMIT: usize = 8 * 1024 * 1024;
const SOFT_SRV_SLOT: usize = 0;
const GLYPH_SRV_SLOT: usize = 1;
const SRV_DESCRIPTOR_COUNT: u32 = 2;

const RECT_HLSL: &str = r#"
cbuffer RectCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    float4 u_rect;
    float4 u_color;
    float4 u_radius;
    float4 u_stroke;
};

static const float2 UNIT_POS[6] = {
    float2(0.0, 0.0), float2(1.0, 0.0), float2(0.0, 1.0),
    float2(0.0, 1.0), float2(1.0, 0.0), float2(1.0, 1.0)
};

struct VSOut {
    float4 pos : SV_POSITION;
    float2 local : TEXCOORD0;
    float2 rect_size : TEXCOORD1;
};

VSOut VSMain(uint vertex_id : SV_VertexID)
{
    VSOut o;
    float2 unit = UNIT_POS[vertex_id];
    float2 pos = u_rect.xy + unit * u_rect.zw;
    float2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    o.pos = float4(ndc, 0.0, 1.0);
    o.local = unit * u_rect.zw;
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
    float mask = any(u_radius > 0.0)
        ? saturate(0.5 - rounded_rect_sdf(input.local, input.rect_size, u_radius))
        : 1.0;
    if (mask <= 0.0)
        discard;
    // Match CPU solid fill: 8-bit premultiply first, then apply analytic
    // coverage and use the premultiplied SrcOver PSO.
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
    float4 u_uv_rect;
};

static const float2 CLIP_POS[6] = {
    float2(-1.0, -1.0), float2(1.0, -1.0), float2(-1.0, 1.0),
    float2(-1.0, 1.0), float2(1.0, -1.0), float2(1.0, 1.0)
};

struct VSOut {
    float4 pos : SV_POSITION;
    float2 uv : TEXCOORD0;
};

VSOut VSMain(uint vertex_id : SV_VertexID)
{
    VSOut o;
    float2 pos = CLIP_POS[vertex_id];
    o.pos = float4(pos, 0.0, 1.0);
    float2 unit = float2(pos.x * 0.5 + 0.5, 0.5 - pos.y * 0.5);
    o.uv = u_uv_rect.xy + unit * u_uv_rect.zw;
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
    // Match the CPU glyph path's two integer truncation steps before the
    // premultiplied SrcOver blend.
    float coverage = floor(saturate(u_atlas.Sample(u_samp, input.uv)) * 255.0 + 0.5);
    float4 color = floor(saturate(input.color) * 255.0 + 0.5);
    float alpha = floor(color.a * coverage / 255.0);
    float3 premul = floor(color.rgb * color.a / 255.0);
    float3 rgb = floor(premul * coverage / 255.0);
    return float4(rgb, alpha) / 255.0;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GlyphAtlasPlacement {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl GlyphAtlasPlacement {
    fn uv(self) -> (f32, f32, f32, f32) {
        let inverse = 1.0 / GLYPH_ATLAS_SIZE as f32;
        (
            self.x as f32 * inverse,
            self.y as f32 * inverse,
            (self.x + self.width) as f32 * inverse,
            (self.y + self.height) as f32 * inverse,
        )
    }
}

struct GlyphAtlasEntry {
    // Retaining the allocation prevents pointer reuse while this cache entry is live.
    _coverage: Arc<[u8]>,
    placement: GlyphAtlasPlacement,
}

#[derive(Default)]
pub(crate) struct GlyphAtlasState {
    x: u32,
    y: u32,
    row_height: u32,
    cache: HashMap<GlyphAtlasKey, GlyphAtlasEntry>,
}

impl GlyphAtlasState {
    pub(crate) fn reset(&mut self) {
        self.x = 0;
        self.y = 0;
        self.row_height = 0;
        self.cache.clear();
    }

    pub(crate) fn can_fit(&self, width: u32, height: u32) -> bool {
        let mut x = self.x;
        let mut y = self.y;
        let mut row_height = self.row_height;
        if x.saturating_add(width) > GLYPH_ATLAS_SIZE {
            x = 0;
            y = y.saturating_add(row_height);
            row_height = 0;
        }
        x.saturating_add(width) <= GLYPH_ATLAS_SIZE
            && y.saturating_add(height) <= GLYPH_ATLAS_SIZE
            && row_height.max(height) <= GLYPH_ATLAS_SIZE
    }

    pub(crate) fn pack(&mut self, width: u32, height: u32) -> Option<GlyphAtlasPlacement> {
        if self.x.saturating_add(width) > GLYPH_ATLAS_SIZE {
            self.x = 0;
            self.y = self.y.saturating_add(self.row_height);
            self.row_height = 0;
        }
        if !self.can_fit(width, height) {
            return None;
        }
        let placement = GlyphAtlasPlacement {
            x: self.x,
            y: self.y,
            width,
            height,
        };
        self.x = self.x.saturating_add(width).saturating_add(1);
        self.row_height = self.row_height.max(height.saturating_add(1));
        Some(placement)
    }
}

struct PendingGlyphUpload {
    placement: GlyphAtlasPlacement,
    coverage: Arc<[u8]>,
}

#[derive(Default)]
struct GlyphSegment {
    uploads: Vec<PendingGlyphUpload>,
    vertices: Vec<GlyphVertex>,
    texture_upload_offset: usize,
    texture_upload_height: u32,
    vertex_offset: usize,
    vertex_bytes: usize,
}

struct UploadBuffer {
    resource: ID3D12Resource,
    capacity: usize,
}

#[derive(Default)]
struct FrameUploads {
    buffers: Vec<UploadBuffer>,
    transient: Vec<ID3D12Resource>,
    used: usize,
}

pub(super) struct D3d12Pipeline {
    root_signature: ID3D12RootSignature,
    solid_pso: ID3D12PipelineState,
    soft_pso: ID3D12PipelineState,
    glyph_pso: ID3D12PipelineState,
    srv_heap: ID3D12DescriptorHeap,
    srv_stride: u32,
    soft_texture: Option<ID3D12Resource>,
    soft_texture_state: D3D12_RESOURCE_STATES,
    soft_width: i32,
    soft_height: i32,
    glyph_atlas: Option<ID3D12Resource>,
    glyph_atlas_state: D3D12_RESOURCE_STATES,
    glyph_state: GlyphAtlasState,
    #[cfg(test)]
    glyph_atlas_upload_count: usize,
    frame_uploads: Vec<FrameUploads>,
}

fn pipeline_error(operation: &str, error: ::windows::core::Error) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("D3d12Pipeline: {operation} failed: {error}"),
    )
}

fn invalid_input(message: impl Into<String>) -> Error {
    Error::new(Errc::InvalidArgument, message)
}

fn blob_message(blob: Option<&ID3DBlob>) -> String {
    let Some(blob) = blob else {
        return String::new();
    };
    let ptr = unsafe { blob.GetBufferPointer() }.cast::<u8>();
    let len = unsafe { blob.GetBufferSize() };
    if ptr.is_null() || len == 0 {
        return String::new();
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf8_lossy(bytes)
        .trim_end_matches('\0')
        .to_string()
}

fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
    let ptr = unsafe { blob.GetBufferPointer() }.cast::<u8>();
    let len = unsafe { blob.GetBufferSize() };
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

fn compile_shader(source: &str, entry: &[u8], target: &[u8]) -> Result<ID3DBlob> {
    let mut code = None;
    let mut errors = None;
    let compile = unsafe {
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
    if let Err(error) = compile {
        return Err(Error::new(
            Errc::PlatformError,
            format!(
                "D3d12Pipeline: D3DCompile failed: {error}; {}",
                blob_message(errors.as_ref())
            ),
        ));
    }
    code.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            "D3d12Pipeline: compiler returned no blob",
        )
    })
}

fn create_root_signature(device: &ID3D12Device) -> Result<ID3D12RootSignature> {
    let range = D3D12_DESCRIPTOR_RANGE {
        RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
        NumDescriptors: 1,
        BaseShaderRegister: 0,
        RegisterSpace: 0,
        OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
    };
    let parameters = [
        D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                Constants: D3D12_ROOT_CONSTANTS {
                    ShaderRegister: 0,
                    RegisterSpace: 0,
                    Num32BitValues: RECT_ROOT_DWORDS,
                },
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
        },
        D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: 1,
                    pDescriptorRanges: &range,
                },
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
        },
    ];
    let sampler = D3D12_STATIC_SAMPLER_DESC {
        Filter: D3D12_FILTER_MIN_MAG_MIP_POINT,
        AddressU: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        AddressV: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        AddressW: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        MipLODBias: 0.0,
        MaxAnisotropy: 1,
        ComparisonFunc: D3D12_COMPARISON_FUNC_NEVER,
        BorderColor: D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK,
        MinLOD: 0.0,
        MaxLOD: f32::MAX,
        ShaderRegister: 0,
        RegisterSpace: 0,
        ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
    };
    let desc = D3D12_ROOT_SIGNATURE_DESC {
        NumParameters: parameters.len() as u32,
        pParameters: parameters.as_ptr(),
        NumStaticSamplers: 1,
        pStaticSamplers: &sampler,
        Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
    };
    let mut serialized = None;
    let mut errors = None;
    let serialize = unsafe {
        D3D12SerializeRootSignature(
            &desc,
            D3D_ROOT_SIGNATURE_VERSION_1,
            &mut serialized,
            Some(&mut errors),
        )
    };
    if let Err(error) = serialize {
        return Err(Error::new(
            Errc::PlatformError,
            format!(
                "D3d12Pipeline: D3D12SerializeRootSignature failed: {error}; {}",
                blob_message(errors.as_ref())
            ),
        ));
    }
    let serialized = serialized.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            "D3d12Pipeline: root signature serializer returned no blob",
        )
    })?;
    unsafe { device.CreateRootSignature(0, blob_bytes(&serialized)) }
        .map_err(|error| pipeline_error("ID3D12Device::CreateRootSignature", error))
}

fn alpha_blend_desc(premultiplied_source: bool) -> D3D12_BLEND_DESC {
    let target = D3D12_RENDER_TARGET_BLEND_DESC {
        BlendEnable: TRUE,
        LogicOpEnable: FALSE,
        SrcBlend: if premultiplied_source {
            D3D12_BLEND_ONE
        } else {
            D3D12_BLEND_SRC_ALPHA
        },
        DestBlend: D3D12_BLEND_INV_SRC_ALPHA,
        BlendOp: D3D12_BLEND_OP_ADD,
        SrcBlendAlpha: D3D12_BLEND_ONE,
        DestBlendAlpha: D3D12_BLEND_INV_SRC_ALPHA,
        BlendOpAlpha: D3D12_BLEND_OP_ADD,
        LogicOp: D3D12_LOGIC_OP_NOOP,
        RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
    };
    D3D12_BLEND_DESC {
        AlphaToCoverageEnable: FALSE,
        IndependentBlendEnable: FALSE,
        RenderTarget: [target; 8],
    }
}

fn create_pso(
    device: &ID3D12Device,
    root_signature: &ID3D12RootSignature,
    vs: &ID3DBlob,
    ps: &ID3DBlob,
    input_layout: &[D3D12_INPUT_ELEMENT_DESC],
    premultiplied_source: bool,
    label: &str,
) -> Result<ID3D12PipelineState> {
    let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: ManuallyDrop::new(Some(root_signature.clone())),
        VS: D3D12_SHADER_BYTECODE {
            pShaderBytecode: unsafe { vs.GetBufferPointer() },
            BytecodeLength: unsafe { vs.GetBufferSize() },
        },
        PS: D3D12_SHADER_BYTECODE {
            pShaderBytecode: unsafe { ps.GetBufferPointer() },
            BytecodeLength: unsafe { ps.GetBufferSize() },
        },
        InputLayout: D3D12_INPUT_LAYOUT_DESC {
            pInputElementDescs: if input_layout.is_empty() {
                std::ptr::null()
            } else {
                input_layout.as_ptr()
            },
            NumElements: input_layout.len() as u32,
        },
        BlendState: alpha_blend_desc(premultiplied_source),
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC {
            FillMode: D3D12_FILL_MODE_SOLID,
            CullMode: D3D12_CULL_MODE_NONE,
            FrontCounterClockwise: FALSE,
            DepthBias: 0,
            DepthBiasClamp: 0.0,
            SlopeScaledDepthBias: 0.0,
            DepthClipEnable: TRUE,
            MultisampleEnable: FALSE,
            AntialiasedLineEnable: FALSE,
            ForcedSampleCount: 0,
            ConservativeRaster: D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
        },
        DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
            DepthEnable: FALSE,
            DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO,
            DepthFunc: D3D12_COMPARISON_FUNC_ALWAYS,
            StencilEnable: FALSE,
            ..D3D12_DEPTH_STENCIL_DESC::default()
        },
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: 1,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        IBStripCutValue: D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED,
        Flags: D3D12_PIPELINE_STATE_FLAG_NONE,
        ..D3D12_GRAPHICS_PIPELINE_STATE_DESC::default()
    };
    desc.RTVFormats[0] = DXGI_FORMAT_B8G8R8A8_UNORM;
    let created: ::windows::core::Result<ID3D12PipelineState> =
        unsafe { device.CreateGraphicsPipelineState(&desc) };
    unsafe {
        ManuallyDrop::drop(&mut desc.pRootSignature);
    }
    created.map_err(|error| pipeline_error(label, error))
}

fn upload_heap_properties() -> D3D12_HEAP_PROPERTIES {
    D3D12_HEAP_PROPERTIES {
        Type: D3D12_HEAP_TYPE_UPLOAD,
        CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
        MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
        CreationNodeMask: 0,
        VisibleNodeMask: 0,
    }
}

fn default_heap_properties() -> D3D12_HEAP_PROPERTIES {
    D3D12_HEAP_PROPERTIES {
        Type: D3D12_HEAP_TYPE_DEFAULT,
        CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
        MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
        CreationNodeMask: 0,
        VisibleNodeMask: 0,
    }
}

fn buffer_desc(size: usize) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Alignment: 0,
        Width: size.max(1) as u64,
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: Default::default(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        Flags: D3D12_RESOURCE_FLAG_NONE,
    }
}

fn create_upload_buffer(device: &ID3D12Device, size: usize) -> Result<ID3D12Resource> {
    let heap = upload_heap_properties();
    let desc = buffer_desc(size);
    let mut resource = None;
    unsafe {
        device.CreateCommittedResource(
            &heap,
            D3D12_HEAP_FLAG_NONE,
            &desc,
            D3D12_RESOURCE_STATE_GENERIC_READ,
            None,
            &mut resource,
        )
    }
    .map_err(|error| pipeline_error("CreateCommittedResource(upload)", error))?;
    resource.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            "D3d12Pipeline: upload buffer was not created",
        )
    })
}

fn create_soft_texture(device: &ID3D12Device, width: i32, height: i32) -> Result<ID3D12Resource> {
    let heap = default_heap_properties();
    let desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        Alignment: 0,
        Width: width as u64,
        Height: height as u32,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
        Flags: D3D12_RESOURCE_FLAG_NONE,
    };
    let mut resource = None;
    unsafe {
        device.CreateCommittedResource(
            &heap,
            D3D12_HEAP_FLAG_NONE,
            &desc,
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
            &mut resource,
        )
    }
    .map_err(|error| pipeline_error("CreateCommittedResource(soft texture)", error))?;
    resource.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            "D3d12Pipeline: soft texture was not created",
        )
    })
}

fn create_glyph_atlas(device: &ID3D12Device) -> Result<ID3D12Resource> {
    let heap = default_heap_properties();
    let desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        Alignment: 0,
        Width: GLYPH_ATLAS_SIZE as u64,
        Height: GLYPH_ATLAS_SIZE,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: DXGI_FORMAT_R8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
        Flags: D3D12_RESOURCE_FLAG_NONE,
    };
    let mut resource = None;
    unsafe {
        device.CreateCommittedResource(
            &heap,
            D3D12_HEAP_FLAG_NONE,
            &desc,
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
            &mut resource,
        )
    }
    .map_err(|error| pipeline_error("CreateCommittedResource(glyph atlas)", error))?;
    resource.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            "D3d12Pipeline: glyph atlas was not created",
        )
    })
}

fn checked_align_up(value: usize, alignment: usize, label: &str) -> Result<usize> {
    debug_assert!(alignment.is_power_of_two());
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
        .ok_or_else(|| invalid_input(format!("D3d12Pipeline: {label} alignment overflow")))
}

pub(crate) fn validated_glyph_layout(
    width: u32,
    height: u32,
    coverage_len: usize,
) -> Result<(usize, usize)> {
    if width == 0 || height == 0 || width > GLYPH_ATLAS_SIZE || height > GLYPH_ATLAS_SIZE {
        return Err(invalid_input(format!(
            "D3d12Pipeline: glyph dimensions must be within 1..={GLYPH_ATLAS_SIZE}, got {width}x{height}"
        )));
    }
    let expected = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| invalid_input("D3d12Pipeline: glyph coverage size overflow"))?;
    if coverage_len < expected {
        return Err(invalid_input(format!(
            "D3d12Pipeline: glyph coverage buffer too small, got {coverage_len}, need {expected}"
        )));
    }
    let row_pitch = checked_align_up(
        width as usize,
        D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as usize,
        "glyph row pitch",
    )?;
    let total_bytes = row_pitch
        .checked_mul(height as usize)
        .ok_or_else(|| invalid_input("D3d12Pipeline: glyph upload size overflow"))?;
    Ok((row_pitch, total_bytes))
}

pub(crate) fn glyph_segment_staging_bytes(upload_height: u32) -> Result<usize> {
    if upload_height > GLYPH_ATLAS_SIZE {
        return Err(invalid_input(format!(
            "D3d12Pipeline: glyph staging height {upload_height} exceeds {GLYPH_ATLAS_SIZE}"
        )));
    }
    (GLYPH_ATLAS_SIZE as usize)
        .checked_mul(upload_height as usize)
        .ok_or_else(|| invalid_input("D3d12Pipeline: glyph staging size overflow"))
}

pub(crate) fn glyph_upload_fits_retained_budget(
    retained_capacity: Option<usize>,
    current_slot_capacity: usize,
    total_bytes: usize,
) -> bool {
    current_slot_capacity >= total_bytes
        || retained_capacity
            .and_then(|total| total.checked_add(total_bytes.saturating_sub(current_slot_capacity)))
            .is_some_and(|total| total <= GLYPH_RETAINED_UPLOAD_LIMIT)
}

pub(crate) fn validated_soft_layout(
    width: i32,
    height: i32,
    pixels_len: usize,
) -> Result<(usize, usize)> {
    if width <= 0 || height <= 0 {
        return Err(invalid_input(format!(
            "D3d12Pipeline: soft dimensions must be positive, got {width}x{height}"
        )));
    }
    let width = width as usize;
    let height = height as usize;
    let expected = width
        .checked_mul(height)
        .ok_or_else(|| invalid_input("D3d12Pipeline: soft pixel count overflow"))?;
    if pixels_len < expected {
        return Err(invalid_input(format!(
            "D3d12Pipeline: soft buffer too small, got {pixels_len}, need {expected}"
        )));
    }
    let row_bytes = width
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| invalid_input("D3d12Pipeline: soft row byte count overflow"))?;
    let row_pitch = row_bytes
        .checked_add(D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as usize - 1)
        .ok_or_else(|| invalid_input("D3d12Pipeline: soft row pitch overflow"))?
        & !(D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as usize - 1);
    if u32::try_from(row_pitch).is_err() {
        return Err(invalid_input(
            "D3d12Pipeline: soft row pitch exceeds D3D12 footprint range",
        ));
    }
    let total_bytes = row_pitch
        .checked_mul(height)
        .ok_or_else(|| invalid_input("D3d12Pipeline: soft upload byte count overflow"))?;
    Ok((row_pitch, total_bytes))
}

fn scissor_rect(viewport_w: f32, viewport_h: f32, scissor: Option<(i32, i32, i32, i32)>) -> RECT {
    let max_x = viewport_w.ceil().clamp(0.0, i32::MAX as f32) as i32;
    let max_y = viewport_h.ceil().clamp(0.0, i32::MAX as f32) as i32;
    let (x, y, width, height) = scissor.unwrap_or((0, 0, max_x, max_y));
    let left = x.clamp(0, max_x);
    let top = y.clamp(0, max_y);
    RECT {
        left,
        top,
        right: x.saturating_add(width.max(0)).clamp(left, max_x),
        bottom: y.saturating_add(height.max(0)).clamp(top, max_y),
    }
}

impl D3d12Pipeline {
    pub(super) fn new(device: &ID3D12Device, frame_count: usize) -> Result<Self> {
        let root_signature = create_root_signature(device)?;
        let rect_vs = compile_shader(RECT_HLSL, b"VSMain\0", b"vs_5_0\0")?;
        let rect_ps = compile_shader(RECT_HLSL, b"PSMain\0", b"ps_5_0\0")?;
        let blit_vs = compile_shader(BLIT_HLSL, b"VSMain\0", b"vs_5_0\0")?;
        let blit_ps = compile_shader(BLIT_HLSL, b"PSMain\0", b"ps_5_0\0")?;
        let glyph_vs = compile_shader(GLYPH_HLSL, b"VSMain\0", b"vs_5_0\0")?;
        let glyph_ps = compile_shader(GLYPH_HLSL, b"PSMain\0", b"ps_5_0\0")?;
        let glyph_input_layout = [
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR(c"POSITION".as_ptr().cast()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 0,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR(c"TEXCOORD".as_ptr().cast()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 8,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR(c"COLOR".as_ptr().cast()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 16,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
        ];
        let solid_pso = create_pso(
            device,
            &root_signature,
            &rect_vs,
            &rect_ps,
            &[],
            true,
            "CreateGraphicsPipelineState(solid)",
        )?;
        let soft_pso = create_pso(
            device,
            &root_signature,
            &blit_vs,
            &blit_ps,
            &[],
            true,
            "CreateGraphicsPipelineState(soft blit)",
        )?;
        let glyph_pso = create_pso(
            device,
            &root_signature,
            &glyph_vs,
            &glyph_ps,
            &glyph_input_layout,
            true,
            "CreateGraphicsPipelineState(glyph)",
        )?;
        let heap_desc = D3D12_DESCRIPTOR_HEAP_DESC {
            Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            NumDescriptors: SRV_DESCRIPTOR_COUNT,
            Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
            NodeMask: 0,
        };
        let srv_heap = unsafe { device.CreateDescriptorHeap(&heap_desc) }
            .map_err(|error| pipeline_error("CreateDescriptorHeap(SRV)", error))?;
        let srv_stride = unsafe {
            device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV)
        };
        Ok(Self {
            root_signature,
            solid_pso,
            soft_pso,
            glyph_pso,
            srv_heap,
            srv_stride,
            soft_texture: None,
            soft_texture_state: D3D12_RESOURCE_STATE_COPY_DEST,
            soft_width: 0,
            soft_height: 0,
            glyph_atlas: None,
            glyph_atlas_state: D3D12_RESOURCE_STATE_COPY_DEST,
            glyph_state: GlyphAtlasState::default(),
            #[cfg(test)]
            glyph_atlas_upload_count: 0,
            frame_uploads: (0..frame_count).map(|_| FrameUploads::default()).collect(),
        })
    }

    pub(super) fn begin_frame(&mut self, frame_index: usize) {
        if let Some(frame) = self.frame_uploads.get_mut(frame_index) {
            frame.transient.clear();
            frame.used = 0;
        }
    }

    fn srv_cpu_handle(&self, slot: usize) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        let mut handle = unsafe { self.srv_heap.GetCPUDescriptorHandleForHeapStart() };
        handle.ptr += slot * self.srv_stride as usize;
        handle
    }

    fn srv_gpu_handle(&self, slot: usize) -> D3D12_GPU_DESCRIPTOR_HANDLE {
        let mut handle = unsafe { self.srv_heap.GetGPUDescriptorHandleForHeapStart() };
        handle.ptr += (slot * self.srv_stride as usize) as u64;
        handle
    }

    pub(super) fn draw_solid_rects(
        &self,
        list: &ID3D12GraphicsCommandList,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuSolidRect],
    ) -> Result<()> {
        if rects.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        if !viewport_w.is_finite() || !viewport_h.is_finite() {
            return Err(invalid_input("D3d12Pipeline: viewport must be finite"));
        }
        let viewport = D3D12_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: viewport_w,
            Height: viewport_h,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        let scissor = scissor_rect(viewport_w, viewport_h, scissor);
        if scissor.right <= scissor.left || scissor.bottom <= scissor.top {
            return Ok(());
        }
        unsafe {
            list.SetGraphicsRootSignature(&self.root_signature);
            list.SetPipelineState(&self.solid_pso);
            list.RSSetViewports(&[viewport]);
            list.RSSetScissorRects(&[scissor]);
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        }
        for rect in rects {
            if rect.w <= 0.0 || rect.h <= 0.0 {
                continue;
            }
            let constants = RectConstants {
                viewport: [viewport_w, viewport_h],
                _pad0: [0.0; 2],
                rect: [rect.x, rect.y, rect.w, rect.h],
                color: rect.rgba,
                radius: rect.radius,
                stroke: [0.0; 4],
            };
            unsafe {
                list.SetGraphicsRoot32BitConstants(
                    0,
                    RECT_ROOT_DWORDS,
                    (&constants as *const RectConstants).cast::<c_void>(),
                    0,
                );
                list.DrawInstanced(6, 1, 0, 0);
            }
        }
        Ok(())
    }

    fn ensure_soft_texture(
        &mut self,
        device: &ID3D12Device,
        width: i32,
        height: i32,
    ) -> Result<()> {
        if self.soft_texture.is_some() && self.soft_width == width && self.soft_height == height {
            return Ok(());
        }
        let texture = create_soft_texture(device, width, height)?;
        let srv_desc = D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
            Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
            Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D12_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                    PlaneSlice: 0,
                    ResourceMinLODClamp: 0.0,
                },
            },
        };
        unsafe {
            device.CreateShaderResourceView(
                &texture,
                Some(&srv_desc),
                self.srv_cpu_handle(SOFT_SRV_SLOT),
            );
        }
        self.soft_texture = Some(texture);
        self.soft_texture_state = D3D12_RESOURCE_STATE_COPY_DEST;
        self.soft_width = width;
        self.soft_height = height;
        Ok(())
    }

    fn ensure_glyph_atlas(&mut self, device: &ID3D12Device) -> Result<()> {
        if self.glyph_atlas.is_some() {
            return Ok(());
        }
        let texture = create_glyph_atlas(device)?;
        let srv_desc = D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_R8_UNORM,
            ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
            Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
            Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D12_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                    PlaneSlice: 0,
                    ResourceMinLODClamp: 0.0,
                },
            },
        };
        unsafe {
            device.CreateShaderResourceView(
                &texture,
                Some(&srv_desc),
                self.srv_cpu_handle(GLYPH_SRV_SLOT),
            );
        }
        self.glyph_atlas = Some(texture);
        self.glyph_atlas_state = D3D12_RESOURCE_STATE_COPY_DEST;
        self.glyph_state.reset();
        Ok(())
    }

    fn push_glyph_quad(
        vertices: &mut Vec<GlyphVertex>,
        glyph: &GpuGlyphBlit,
        placement: GlyphAtlasPlacement,
    ) {
        let (u0, v0, u1, v1) = placement.uv();
        let x0 = glyph.x;
        let y0 = glyph.y;
        let x1 = glyph.x + glyph.w;
        let y1 = glyph.y + glyph.h;
        let make = |pos, uv| GlyphVertex {
            pos,
            uv,
            color: glyph.rgba,
        };
        vertices.extend_from_slice(&[
            make([x0, y0], [u0, v0]),
            make([x1, y0], [u1, v0]),
            make([x0, y1], [u0, v1]),
            make([x0, y1], [u0, v1]),
            make([x1, y0], [u1, v0]),
            make([x1, y1], [u1, v1]),
        ]);
    }

    fn plan_glyph_segments(&mut self, glyphs: &[GpuGlyphBlit]) -> Result<Vec<GlyphSegment>> {
        let mut segments = Vec::new();
        let mut current = GlyphSegment::default();
        for glyph in glyphs {
            if glyph.w <= 0.0 || glyph.h <= 0.0 || glyph.cov_w == 0 || glyph.cov_h == 0 {
                continue;
            }
            let expected = (glyph.cov_w as usize)
                .checked_mul(glyph.cov_h as usize)
                .ok_or_else(|| invalid_input("D3d12Pipeline: glyph coverage size overflow"))?;
            validated_glyph_layout(glyph.cov_w, glyph.cov_h, glyph.coverage.len())?;
            let cache_key = (glyph.coverage.len() == expected)
                .then(|| GlyphAtlasKey::new(&glyph.coverage, glyph.cov_w, glyph.cov_h));
            if let Some(placement) = cache_key
                .as_ref()
                .and_then(|key| self.glyph_state.cache.get(key))
                .map(|entry| entry.placement)
            {
                Self::push_glyph_quad(&mut current.vertices, glyph, placement);
                continue;
            }

            if !self.glyph_state.can_fit(glyph.cov_w, glyph.cov_h) {
                if !current.vertices.is_empty() {
                    segments.push(std::mem::take(&mut current));
                }
                self.glyph_state.reset();
            }
            let placement = self
                .glyph_state
                .pack(glyph.cov_w, glyph.cov_h)
                .ok_or_else(|| {
                    Error::new(
                        Errc::PlatformError,
                        "D3d12Pipeline: glyph does not fit after atlas reset",
                    )
                })?;
            current.uploads.push(PendingGlyphUpload {
                placement,
                coverage: Arc::clone(&glyph.coverage),
            });
            if let Some(cache_key) = cache_key {
                self.glyph_state.cache.insert(
                    cache_key,
                    GlyphAtlasEntry {
                        _coverage: Arc::clone(&glyph.coverage),
                        placement,
                    },
                );
            }
            Self::push_glyph_quad(&mut current.vertices, glyph, placement);
        }
        if !current.vertices.is_empty() {
            segments.push(current);
        }
        Ok(segments)
    }

    fn layout_glyph_upload(segments: &mut [GlyphSegment]) -> Result<usize> {
        let mut cursor = 0usize;
        for segment in segments {
            if !segment.uploads.is_empty() {
                cursor = checked_align_up(
                    cursor,
                    D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT as usize,
                    "glyph footprint",
                )?;
                segment.texture_upload_offset = cursor;
                segment.texture_upload_height = segment
                    .uploads
                    .iter()
                    .map(|upload| upload.placement.y.saturating_add(upload.placement.height))
                    .max()
                    .unwrap_or(0);
                let byte_count = glyph_segment_staging_bytes(segment.texture_upload_height)?;
                cursor = cursor
                    .checked_add(byte_count)
                    .ok_or_else(|| invalid_input("D3d12Pipeline: glyph upload offset overflow"))?;
            }
            cursor = checked_align_up(cursor, 16, "glyph vertex buffer")?;
            segment.vertex_offset = cursor;
            segment.vertex_bytes = segment
                .vertices
                .len()
                .checked_mul(size_of::<GlyphVertex>())
                .ok_or_else(|| invalid_input("D3d12Pipeline: glyph vertex size overflow"))?;
            cursor = cursor
                .checked_add(segment.vertex_bytes)
                .ok_or_else(|| invalid_input("D3d12Pipeline: glyph vertex offset overflow"))?;
        }
        Ok(cursor)
    }

    fn acquire_glyph_upload(
        &mut self,
        device: &ID3D12Device,
        frame_index: usize,
        total_bytes: usize,
    ) -> Result<ID3D12Resource> {
        let frame = self.frame_uploads.get(frame_index).ok_or_else(|| {
            invalid_input(format!(
                "D3d12Pipeline: invalid glyph upload frame index {frame_index}"
            ))
        })?;
        let slot = frame.used;
        let retained_capacity = frame
            .buffers
            .iter()
            .try_fold(0usize, |total, buffer| total.checked_add(buffer.capacity));
        let current_slot_capacity = frame.buffers.get(slot).map_or(0, |buffer| buffer.capacity);
        if glyph_upload_fits_retained_budget(retained_capacity, current_slot_capacity, total_bytes)
        {
            return self.acquire_upload(device, frame_index, total_bytes);
        }
        let resource = create_upload_buffer(device, total_bytes)?;
        self.frame_uploads[frame_index]
            .transient
            .push(resource.clone());
        Ok(resource)
    }

    fn acquire_upload(
        &mut self,
        device: &ID3D12Device,
        frame_index: usize,
        total_bytes: usize,
    ) -> Result<ID3D12Resource> {
        let frame = self.frame_uploads.get_mut(frame_index).ok_or_else(|| {
            invalid_input(format!(
                "D3d12Pipeline: invalid upload frame index {frame_index}"
            ))
        })?;
        let slot = frame.used;
        let needs_buffer = frame
            .buffers
            .get(slot)
            .is_none_or(|buffer| buffer.capacity < total_bytes);
        if needs_buffer {
            let resource = create_upload_buffer(device, total_bytes)?;
            let buffer = UploadBuffer {
                resource,
                capacity: total_bytes,
            };
            if slot < frame.buffers.len() {
                frame.buffers[slot] = buffer;
            } else {
                frame.buffers.push(buffer);
            }
        }
        frame.used += 1;
        Ok(frame.buffers[slot].resource.clone())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the explicit frame and viewport inputs match the D3D12 recording boundary"
    )]
    pub(super) fn draw_glyphs(
        &mut self,
        device: &ID3D12Device,
        list: &ID3D12GraphicsCommandList,
        frame_index: usize,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<()> {
        let result = self.draw_glyphs_inner(
            device,
            list,
            frame_index,
            viewport_w,
            viewport_h,
            scissor,
            glyphs,
        );
        if result.is_err() {
            // Planning updates the persistent cache before command recording.
            // On any failure, discard it so a later frame never samples an
            // entry whose upload may not have reached the queue.
            self.glyph_state.reset();
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_glyphs_inner(
        &mut self,
        device: &ID3D12Device,
        list: &ID3D12GraphicsCommandList,
        frame_index: usize,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<()> {
        if glyphs.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        if !viewport_w.is_finite() || !viewport_h.is_finite() {
            return Err(invalid_input("D3d12Pipeline: viewport must be finite"));
        }
        let viewport = D3D12_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: viewport_w,
            Height: viewport_h,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        let scissor = scissor_rect(viewport_w, viewport_h, scissor);
        if scissor.right <= scissor.left || scissor.bottom <= scissor.top {
            return Ok(());
        }

        self.ensure_glyph_atlas(device)?;
        let mut segments = self.plan_glyph_segments(glyphs)?;
        if segments.is_empty() {
            return Ok(());
        }
        let total_bytes = Self::layout_glyph_upload(&mut segments)?;
        for segment in &segments {
            u32::try_from(segment.vertices.len()).map_err(|_| {
                invalid_input("D3d12Pipeline: glyph vertex count exceeds D3D12 range")
            })?;
            u32::try_from(segment.vertex_bytes).map_err(|_| {
                invalid_input("D3d12Pipeline: glyph vertex bytes exceed D3D12 range")
            })?;
        }
        let upload = self.acquire_glyph_upload(device, frame_index, total_bytes)?;

        let empty_read = D3D12_RANGE { Begin: 0, End: 0 };
        let mut mapped = std::ptr::null_mut();
        unsafe { upload.Map(0, Some(&empty_read), Some(&mut mapped)) }
            .map_err(|error| pipeline_error("ID3D12Resource::Map(glyph upload)", error))?;
        if mapped.is_null() {
            unsafe { upload.Unmap(0, None) };
            return Err(Error::new(
                Errc::PlatformError,
                "D3d12Pipeline: glyph upload Map returned null",
            ));
        }
        for segment in &segments {
            for pending in &segment.uploads {
                let row_bytes = pending.placement.width as usize;
                for row in 0..pending.placement.height as usize {
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            pending.coverage.as_ptr().add(row * row_bytes),
                            mapped.cast::<u8>().add(
                                segment.texture_upload_offset
                                    + (pending.placement.y as usize + row)
                                        * GLYPH_ATLAS_SIZE as usize
                                    + pending.placement.x as usize,
                            ),
                            row_bytes,
                        );
                    }
                }
            }
            unsafe {
                std::ptr::copy_nonoverlapping(
                    segment.vertices.as_ptr().cast::<u8>(),
                    mapped.cast::<u8>().add(segment.vertex_offset),
                    segment.vertex_bytes,
                );
            }
        }
        let written = D3D12_RANGE {
            Begin: 0,
            End: total_bytes,
        };
        unsafe { upload.Unmap(0, Some(&written)) };

        let atlas = self.glyph_atlas.as_ref().cloned().ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "D3d12Pipeline: glyph atlas missing after creation",
            )
        })?;
        let constants = GlyphConstants {
            viewport: [viewport_w, viewport_h],
            _pad0: [0.0; 2],
        };
        let gpu_handle = self.srv_gpu_handle(GLYPH_SRV_SLOT);
        unsafe {
            list.SetDescriptorHeaps(&[Some(self.srv_heap.clone())]);
            list.SetGraphicsRootSignature(&self.root_signature);
            list.SetGraphicsRootDescriptorTable(1, gpu_handle);
            list.SetGraphicsRoot32BitConstants(
                0,
                (size_of::<GlyphConstants>() / size_of::<u32>()) as u32,
                (&constants as *const GlyphConstants).cast::<c_void>(),
                0,
            );
            list.SetPipelineState(&self.glyph_pso);
            list.RSSetViewports(&[viewport]);
            list.RSSetScissorRects(&[scissor]);
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        }

        for segment in &segments {
            if !segment.uploads.is_empty() {
                record_transition(
                    list,
                    &atlas,
                    self.glyph_atlas_state,
                    D3D12_RESOURCE_STATE_COPY_DEST,
                );
                self.glyph_atlas_state = D3D12_RESOURCE_STATE_COPY_DEST;
                for pending in &segment.uploads {
                    let footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                        Offset: segment.texture_upload_offset as u64,
                        Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                            Format: DXGI_FORMAT_R8_UNORM,
                            Width: GLYPH_ATLAS_SIZE,
                            Height: segment.texture_upload_height,
                            Depth: 1,
                            RowPitch: GLYPH_ATLAS_SIZE,
                        },
                    };
                    let mut destination = texture_copy_location_subresource(&atlas);
                    let mut source = texture_copy_location_footprint(&upload, footprint);
                    let source_box = D3D12_BOX {
                        left: pending.placement.x,
                        top: pending.placement.y,
                        front: 0,
                        right: pending.placement.x + pending.placement.width,
                        bottom: pending.placement.y + pending.placement.height,
                        back: 1,
                    };
                    unsafe {
                        list.CopyTextureRegion(
                            &destination,
                            pending.placement.x,
                            pending.placement.y,
                            0,
                            &source,
                            Some(&source_box),
                        );
                    }
                    release_copy_location(&mut destination);
                    release_copy_location(&mut source);
                    #[cfg(test)]
                    {
                        self.glyph_atlas_upload_count += 1;
                    }
                }
                record_transition(
                    list,
                    &atlas,
                    D3D12_RESOURCE_STATE_COPY_DEST,
                    D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                );
                self.glyph_atlas_state = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
            }
            let view = D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: unsafe { upload.GetGPUVirtualAddress() }
                    + segment.vertex_offset as u64,
                SizeInBytes: segment.vertex_bytes as u32,
                StrideInBytes: size_of::<GlyphVertex>() as u32,
            };
            unsafe {
                list.IASetVertexBuffers(0, Some(std::slice::from_ref(&view)));
                list.DrawInstanced(segment.vertices.len() as u32, 1, 0, 0);
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn glyph_atlas_upload_count(&self) -> usize {
        self.glyph_atlas_upload_count
    }

    #[cfg(test)]
    pub(crate) fn glyph_atlas_extent(&self) -> Option<(u32, u32)> {
        self.glyph_atlas
            .as_ref()
            .map(|_| (GLYPH_ATLAS_SIZE, GLYPH_ATLAS_SIZE))
    }

    pub(super) fn blit_soft_fallback(
        &mut self,
        device: &ID3D12Device,
        list: &ID3D12GraphicsCommandList,
        frame_index: usize,
        pixels: &[u32],
        width: i32,
        height: i32,
    ) -> Result<()> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| invalid_input("soft fallback pixel count overflow"))?;
        if pixels.len() < expected {
            return Err(invalid_input(format!(
                "soft fallback buffer too small, got {}, need {expected}",
                pixels.len()
            )));
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
            list,
            frame_index,
            &packed,
            width,
            height,
            SoftFallbackTile::at_destination(x, y, upload_w, upload_h),
        )
    }

    #[allow(clippy::too_many_arguments)] // D3D12 command recording needs the device/list/frame plus API-neutral tile payload.
    pub(super) fn blit_soft_fallback_tile(
        &mut self,
        device: &ID3D12Device,
        list: &ID3D12GraphicsCommandList,
        frame_index: usize,
        pixels: &[u32],
        target_width: i32,
        target_height: i32,
        tile: SoftFallbackTile,
    ) -> Result<()> {
        if target_width <= 0 || target_height <= 0 {
            return Err(invalid_input(format!(
                "invalid soft target {target_width}x{target_height}"
            )));
        }
        tile.validate_payload(pixels)?;
        if tile.dst_x.saturating_add(tile.width) > target_width
            || tile.dst_y.saturating_add(tile.height) > target_height
        {
            return Err(invalid_input(format!(
                "soft tile {}x{} at {},{} exceeds {target_width}x{target_height}",
                tile.width, tile.height, tile.dst_x, tile.dst_y
            )));
        }
        let tile_pixels = (tile.width as usize)
            .checked_mul(tile.height as usize)
            .ok_or_else(|| invalid_input("soft fallback tile pixel count overflow"))?;
        let (row_pitch, total_bytes) = validated_soft_layout(tile.width, tile.height, tile_pixels)?;
        self.ensure_soft_texture(device, target_width, target_height)?;
        let upload = self.acquire_upload(device, frame_index, total_bytes)?;

        let empty_read = D3D12_RANGE { Begin: 0, End: 0 };
        let mut mapped = std::ptr::null_mut();
        unsafe { upload.Map(0, Some(&empty_read), Some(&mut mapped)) }
            .map_err(|error| pipeline_error("ID3D12Resource::Map(soft upload)", error))?;
        if mapped.is_null() {
            unsafe { upload.Unmap(0, None) };
            return Err(Error::new(
                Errc::PlatformError,
                "D3d12Pipeline: soft upload Map returned null",
            ));
        }
        let row_bytes = tile.width as usize * std::mem::size_of::<u32>();
        for row in 0..tile.height as usize {
            let source = row * tile.width as usize;
            unsafe {
                std::ptr::copy_nonoverlapping(
                    pixels.as_ptr().add(source).cast::<u8>(),
                    mapped.cast::<u8>().add(row * row_pitch),
                    row_bytes,
                );
            }
        }
        let written = D3D12_RANGE {
            Begin: 0,
            End: total_bytes,
        };
        unsafe { upload.Unmap(0, Some(&written)) };

        let texture = self.soft_texture.as_ref().cloned().ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "D3d12Pipeline: soft texture missing after creation",
            )
        })?;
        record_transition(
            list,
            &texture,
            self.soft_texture_state,
            D3D12_RESOURCE_STATE_COPY_DEST,
        );
        let footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
            Offset: 0,
            Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                Width: tile.width as u32,
                Height: tile.height as u32,
                Depth: 1,
                RowPitch: row_pitch as u32,
            },
        };
        let mut destination = texture_copy_location_subresource(&texture);
        let mut source = texture_copy_location_footprint(&upload, footprint);
        unsafe {
            list.CopyTextureRegion(
                &destination,
                tile.dst_x as u32,
                tile.dst_y as u32,
                0,
                &source,
                None,
            );
        }
        release_copy_location(&mut destination);
        release_copy_location(&mut source);
        record_transition(
            list,
            &texture,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
        );
        self.soft_texture_state = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;

        let viewport = D3D12_VIEWPORT {
            TopLeftX: tile.dst_x as f32,
            TopLeftY: tile.dst_y as f32,
            Width: tile.width as f32,
            Height: tile.height as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        let scissor = RECT {
            left: tile.dst_x,
            top: tile.dst_y,
            right: tile.dst_x + tile.width,
            bottom: tile.dst_y + tile.height,
        };
        let gpu_handle = self.srv_gpu_handle(SOFT_SRV_SLOT);
        unsafe {
            list.SetDescriptorHeaps(&[Some(self.srv_heap.clone())]);
            list.SetGraphicsRootSignature(&self.root_signature);
            list.SetPipelineState(&self.soft_pso);
            let uv_rect = [
                tile.dst_x as f32 / target_width as f32,
                tile.dst_y as f32 / target_height as f32,
                tile.width as f32 / target_width as f32,
                tile.height as f32 / target_height as f32,
            ];
            list.SetGraphicsRoot32BitConstants(
                0,
                uv_rect.len() as u32,
                uv_rect.as_ptr().cast::<c_void>(),
                0,
            );
            list.SetGraphicsRootDescriptorTable(1, gpu_handle);
            list.RSSetViewports(&[viewport]);
            list.RSSetScissorRects(&[scissor]);
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            list.DrawInstanced(6, 1, 0, 0);
        }
        Ok(())
    }

    pub(super) fn retain_gpu_objects_after_undrained_drop(&self) {
        std::mem::forget(self.root_signature.clone());
        std::mem::forget(self.solid_pso.clone());
        std::mem::forget(self.soft_pso.clone());
        std::mem::forget(self.glyph_pso.clone());
        std::mem::forget(self.srv_heap.clone());
        if let Some(texture) = self.soft_texture.as_ref() {
            std::mem::forget(texture.clone());
        }
        if let Some(texture) = self.glyph_atlas.as_ref() {
            std::mem::forget(texture.clone());
        }
        for frame in &self.frame_uploads {
            for upload in &frame.buffers {
                std::mem::forget(upload.resource.clone());
            }
            for upload in &frame.transient {
                std::mem::forget(upload.clone());
            }
        }
    }
}

fn visible_pixel_bounds(pixels: &[u32], width: i32, height: i32) -> Option<(i32, i32, i32, i32)> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let expected = (width as usize).checked_mul(height as usize)?;
    if pixels.len() < expected {
        return None;
    }
    let mut left = width;
    let mut top = height;
    let mut right = 0;
    let mut bottom = 0;
    let mut visible = false;
    for y in 0..height {
        let row = y as usize * width as usize;
        for x in 0..width {
            if pixels[row + x as usize] & 0xff00_0000 == 0 {
                continue;
            }
            visible = true;
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    visible.then(|| (left, top, right - left, bottom - top))
}

//! Minimal D3D12 native raster pipeline: rounded solid rectangles and glyphs.

#![allow(nonstandard_style)]

use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::{size_of, ManuallyDrop};
use std::sync::Arc;

use crate::core::{Errc, Error, Result};
use crate::native::present::{GpuGlyphBlit, GpuSolidRect};
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
// compact soft upload 移除后，glyph atlas 独占第一个 SRV 槽位。
const GLYPH_SRV_SLOT: usize = 0;
// descriptor heap 只需为 glyph atlas 保留一个可见描述符。
const SRV_DESCRIPTOR_COUNT: u32 = 1;

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
    glyph_pso: ID3D12PipelineState,
    srv_heap: ID3D12DescriptorHeap,
    srv_stride: u32,
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

// 引入拆分后的布局验证辅助。
// 引入拆分后的 D3D12 pipeline 实现。
mod pipeline;

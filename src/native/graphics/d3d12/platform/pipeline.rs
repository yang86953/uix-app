//! Minimal D3D12 native raster pipeline: rounded solid rectangles + soft fallback.

#![allow(nonstandard_style)]

use std::ffi::c_void;
use std::mem::ManuallyDrop;

use crate::core::{Errc, Error, Result};
use crate::native::traits::present::{GpuSolidRect, SoftFallbackTile};
use ::windows::core::PCSTR;
use ::windows::Win32::Foundation::{FALSE, RECT, TRUE};
use ::windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use ::windows::Win32::Graphics::Direct3D::{ID3DBlob, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST};
use ::windows::Win32::Graphics::Direct3D12::*;
use ::windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};

use super::transfer::{
    record_transition, release_copy_location, texture_copy_location_footprint,
    texture_copy_location_subresource,
};

const RECT_ROOT_DWORDS: u32 = 20;

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

float4 PSMain(VSOut input) : SV_Target
{
    float mask = rounded_rect_mask(input.local, input.rect_size, u_radius);
    if (mask <= 0.0)
        discard;
    return float4(u_color.rgb, u_color.a * mask);
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

struct UploadBuffer {
    resource: ID3D12Resource,
    capacity: usize,
}

#[derive(Default)]
struct FrameUploads {
    buffers: Vec<UploadBuffer>,
    used: usize,
}

pub(super) struct D3d12Pipeline {
    root_signature: ID3D12RootSignature,
    solid_pso: ID3D12PipelineState,
    soft_pso: ID3D12PipelineState,
    srv_heap: ID3D12DescriptorHeap,
    soft_texture: Option<ID3D12Resource>,
    soft_texture_state: D3D12_RESOURCE_STATES,
    soft_width: i32,
    soft_height: i32,
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
        let solid_pso = create_pso(
            device,
            &root_signature,
            &rect_vs,
            &rect_ps,
            false,
            "CreateGraphicsPipelineState(solid)",
        )?;
        let soft_pso = create_pso(
            device,
            &root_signature,
            &blit_vs,
            &blit_ps,
            true,
            "CreateGraphicsPipelineState(soft blit)",
        )?;
        let heap_desc = D3D12_DESCRIPTOR_HEAP_DESC {
            Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            NumDescriptors: 1,
            Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
            NodeMask: 0,
        };
        let srv_heap = unsafe { device.CreateDescriptorHeap(&heap_desc) }
            .map_err(|error| pipeline_error("CreateDescriptorHeap(SRV)", error))?;
        Ok(Self {
            root_signature,
            solid_pso,
            soft_pso,
            srv_heap,
            soft_texture: None,
            soft_texture_state: D3D12_RESOURCE_STATE_COPY_DEST,
            soft_width: 0,
            soft_height: 0,
            frame_uploads: (0..frame_count).map(|_| FrameUploads::default()).collect(),
        })
    }

    pub(super) fn begin_frame(&mut self, frame_index: usize) {
        if let Some(frame) = self.frame_uploads.get_mut(frame_index) {
            frame.used = 0;
        }
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
                self.srv_heap.GetCPUDescriptorHandleForHeapStart(),
            );
        }
        self.soft_texture = Some(texture);
        self.soft_texture_state = D3D12_RESOURCE_STATE_COPY_DEST;
        self.soft_width = width;
        self.soft_height = height;
        Ok(())
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
        let gpu_handle = unsafe { self.srv_heap.GetGPUDescriptorHandleForHeapStart() };
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
        std::mem::forget(self.srv_heap.clone());
        if let Some(texture) = self.soft_texture.as_ref() {
            std::mem::forget(texture.clone());
        }
        for frame in &self.frame_uploads {
            for upload in &frame.buffers {
                std::mem::forget(upload.resource.clone());
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

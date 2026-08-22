//! D3D12 薄 RHI 的原生 pipeline 资源 Component。
//!
//! 本模块只把 platform 唯一 `PipelineContract` 机械投影为 Root Signature、
//! HLSL 字节码、输入布局、固定状态和目标格式 PSO 变体；不拥有 draw 或能力激活。

#![allow(nonstandard_style)]

use std::ffi::CStr;
use std::mem::ManuallyDrop;

use crate::core::{Errc, Error, Result};
use crate::native::presentation::graphics::d3d_shader_source::{
    BLUR_HLSL, GLYPH_HLSL, GRADIENT_HLSL, MESH_HLSL, MSDF_GLYPH_HLSL, RECT_HLSL,
    RHI_TEXTURED_PS_HLSL, SECTOR_HLSL, SHADOW_HLSL,
};
use crate::platform::presentation::rhi::{
    PipelineBlendFactor, PipelineBlendOperation, PipelineColorWriteMask, PipelineContract,
    PipelineCullMode, PipelineDepthClip, PipelineDepthState, PipelineDesc, PipelineDitherState,
    PipelineFrontFace, PipelineMultisampleState, PipelinePrimitiveTopology, PipelineSampling,
    PipelineStencilState, PipelineVertexFormat, PipelineVertexLayout, PipelineVertexSemantic,
};
use ::windows::Win32::Foundation::{FALSE, TRUE};
use ::windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use ::windows::Win32::Graphics::Direct3D::ID3DBlob;
use ::windows::Win32::Graphics::Direct3D12::*;
use ::windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R32G32_FLOAT,
    DXGI_FORMAT_R32G32B32A32_FLOAT, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
};
use ::windows::core::PCSTR;

use super::error::{d3d12_error, platform_error};

// 保存一个共享 kind 在 D3D 原生层唯一选择的 HLSL 入口对。
#[derive(Clone, Copy)]
struct D3d12ShaderPair {
    // 顶点阶段始终使用共享源中的 VSMain。
    vertex: &'static str,
    // 像素阶段始终使用共享源中的 PSMain。
    pixel: &'static str,
}

// 保存全部共享门禁完成后才能进入原生对象创建的不可变计划。
struct D3d12PipelinePlan {
    // 保留 platform 唯一契约值，禁止原生层另存同语义字段。
    contract: PipelineContract,
    // 保存由 kind 穷尽选择的唯一 HLSL 入口对。
    shaders: D3d12ShaderPair,
    // 保存从共享顶点布局机械生成的原生输入元素。
    input_layout: Vec<D3D12_INPUT_ELEMENT_DESC>,
    // 保存从共享固定状态机械生成的 D3D12 描述。
    fixed_state: D3d12PipelineFixedState,
}

// 保存 PSO 创建时使用且由 pipeline 资源继续唯一拥有的固定状态快照。
#[derive(Clone, Copy)]
struct D3d12PipelineFixedState {
    // 颜色与 alpha 因子直接来自共享 blend.state()。
    blend: D3D12_BLEND_DESC,
    // 二维 rasterizer 逐项来自共享 raster 与 multisample。
    rasterizer: D3D12_RASTERIZER_DESC,
    // 深度模板逐项来自共享禁用契约。
    depth_stencil: D3D12_DEPTH_STENCIL_DESC,
    // 输出采样掩码由共享 multisample 唯一提供。
    sample_mask: u32,
    // PSO 拓扑类型由共享 primitive topology 唯一提供。
    topology_type: D3D12_PRIMITIVE_TOPOLOGY_TYPE,
    // 目标采样数由共享 multisample 唯一提供。
    sample_desc: DXGI_SAMPLE_DESC,
}

// 同一 pipeline 为当前两个可渲染颜色格式持有真实 PSO 变体。
struct D3d12PipelineStateVariants {
    // Surface 与 BGRA8 离屏目标使用的真实 PSO。
    bgra8: ID3D12PipelineState,
    // RGBA8 离屏目标使用的真实 PSO。
    rgba8: ID3D12PipelineState,
}

// 由 D3D12 资源表唯一拥有的完整原生 pipeline 资源。
#[allow(dead_code)]
pub(crate) struct D3d12PipelineResource {
    // Rust 按字段声明顺序析构，先释放最后创建且引用 Root Signature 的全部 PSO。
    variants: D3d12PipelineStateVariants,
    // 保留真实编译后的顶点 shader bytecode COM owner。
    vertex_shader: ID3DBlob,
    // 保留真实编译后的像素 shader bytecode COM owner。
    pixel_shader: ID3DBlob,
    // 保留创建 PSO 时使用的真实 input layout 描述。
    input_layout: Vec<D3D12_INPUT_ELEMENT_DESC>,
    // 保留由共享契约生成的真实固定状态描述。
    fixed_state: D3d12PipelineFixedState,
    // 只保存同一份 platform 契约值，不复制 ABI 或状态权威。
    contract: PipelineContract,
    // Root Signature 最后释放，覆盖全部依赖它的 PSO 与描述 owner 生命周期。
    root_signature: ID3D12RootSignature,
}

impl D3d12PipelinePlan {
    // 先完成全部共享值域与穷尽映射，原生创建不得早于本计划成功。
    fn from_desc(desc: PipelineDesc) -> Result<Self> {
        // PipelineKind::contract() 是顶点、Uniform、采样、混合和固定状态唯一权威。
        let contract = desc.kind.contract();
        validate_shared_contract(contract)?;
        // kind 到 shader 的原生选择使用穷尽 match，不维护第二份闭集常量。
        let shaders = d3d12_shader_pair(desc.kind);
        // 输入元素只消费共享 vertex layout。
        let input_layout = d3d12_vertex_elements(contract.vertex)?;
        // 所有固定状态只消费同一 contract 值。
        let fixed_state = D3d12PipelineFixedState::from_contract(contract);
        Ok(Self {
            contract,
            shaders,
            input_layout,
            fixed_state,
        })
    }
}

impl D3d12PipelineFixedState {
    // 将共享固定状态完整且机械地投影为 D3D12 PSO 描述片段。
    fn from_contract(contract: PipelineContract) -> Self {
        Self {
            blend: d3d12_blend_desc(contract),
            rasterizer: d3d12_rasterizer_desc(contract),
            depth_stencil: d3d12_depth_stencil_desc(contract),
            sample_mask: contract.multisample.sample_mask(),
            topology_type: d3d12_topology_type(contract.topology),
            sample_desc: d3d12_sample_desc(contract.multisample),
        }
    }
}

impl D3d12PipelineStateVariants {
    // 在资源登记前创建全部当前可渲染目标格式变体。
    fn create(
        device: &ID3D12Device,
        root_signature: &ID3D12RootSignature,
        vertex_shader: &ID3DBlob,
        pixel_shader: &ID3DBlob,
        input_layout: &[D3D12_INPUT_ELEMENT_DESC],
        fixed_state: D3d12PipelineFixedState,
    ) -> Result<Self> {
        let bgra8 = create_pipeline_state(
            device,
            root_signature,
            vertex_shader,
            pixel_shader,
            input_layout,
            fixed_state,
            DXGI_FORMAT_B8G8R8A8_UNORM,
            "ID3D12Device::CreateGraphicsPipelineState(BGRA8)",
        )?;
        let rgba8 = create_pipeline_state(
            device,
            root_signature,
            vertex_shader,
            pixel_shader,
            input_layout,
            fixed_state,
            DXGI_FORMAT_R8G8B8A8_UNORM,
            "ID3D12Device::CreateGraphicsPipelineState(RGBA8)",
        )?;
        Ok(Self { bgra8, rgba8 })
    }
}

impl D3d12PipelineResource {
    // 创建完整原生资源；任一步失败时局部 COM owner 自动释放且不会登记 binding。
    pub(crate) fn create(device: &ID3D12Device, desc: PipelineDesc) -> Result<Self> {
        // 此计划完成前禁止 D3DCompile、Root Signature 或 PSO 原生副作用。
        let plan = D3d12PipelinePlan::from_desc(desc)?;
        let root_signature = create_root_signature(device, plan.contract)?;
        let vertex_shader = compile_shader(plan.shaders.vertex, c"VSMain", c"vs_5_1")?;
        let pixel_shader = compile_shader(plan.shaders.pixel, c"PSMain", c"ps_5_1")?;
        let variants = D3d12PipelineStateVariants::create(
            device,
            &root_signature,
            &vertex_shader,
            &pixel_shader,
            &plan.input_layout,
            plan.fixed_state,
        )?;
        Ok(Self {
            variants,
            vertex_shader,
            pixel_shader,
            input_layout: plan.input_layout,
            fixed_state: plan.fixed_state,
            contract: plan.contract,
            root_signature,
        })
    }

    // 未知 GPU 状态下保留整项资源，禁止释放命令列表可能引用的 PSO/Root Signature。
    pub(crate) fn retain_after_undrained_drop(self) {
        std::mem::forget(self);
    }
}

// 验证共享契约能够被当前 D3D12 固定 pipeline 完整表达。
fn validate_shared_contract(contract: PipelineContract) -> Result<()> {
    if !contract.vertex.is_valid() || contract.uniform.size_bytes() == 0 {
        return Err(Error::new(
            Errc::InvalidArgument,
            "D3D12 RHI pipeline contract is invalid",
        ));
    }
    // 当前 D3D12 PSO 没有隐式颜色抖动，只接受共享禁用语义。
    match contract.dither {
        PipelineDitherState::Disabled => {}
    }
    // 当前资源阶段只机械支持共享单样本语义。
    match contract.multisample {
        PipelineMultisampleState::SingleSample => {}
    }
    Ok(())
}

// 穷尽选择每个共享 kind 使用的同一份 Windows/D3D HLSL 源。
fn d3d12_shader_pair(kind: crate::platform::presentation::rhi::PipelineKind) -> D3d12ShaderPair {
    use crate::platform::presentation::rhi::PipelineKind;

    match kind {
        PipelineKind::SolidMesh => D3d12ShaderPair {
            vertex: MESH_HLSL,
            pixel: MESH_HLSL,
        },
        PipelineKind::TexturedQuad => D3d12ShaderPair {
            vertex: GLYPH_HLSL,
            pixel: RHI_TEXTURED_PS_HLSL,
        },
        PipelineKind::GradientRect => D3d12ShaderPair {
            vertex: GRADIENT_HLSL,
            pixel: GRADIENT_HLSL,
        },
        PipelineKind::GlyphCoverageQuad => D3d12ShaderPair {
            vertex: GLYPH_HLSL,
            pixel: GLYPH_HLSL,
        },
        PipelineKind::ShapeRect | PipelineKind::ShapeRectAdditive => D3d12ShaderPair {
            vertex: RECT_HLSL,
            pixel: RECT_HLSL,
        },
        PipelineKind::BoxShadow => D3d12ShaderPair {
            vertex: SHADOW_HLSL,
            pixel: SHADOW_HLSL,
        },
        PipelineKind::TexturedQuadAdditive => D3d12ShaderPair {
            vertex: GLYPH_HLSL,
            pixel: RHI_TEXTURED_PS_HLSL,
        },
        PipelineKind::BlurPass => D3d12ShaderPair {
            vertex: BLUR_HLSL,
            pixel: BLUR_HLSL,
        },
        PipelineKind::MsdfGlyphQuad => D3d12ShaderPair {
            vertex: MSDF_GLYPH_HLSL,
            pixel: MSDF_GLYPH_HLSL,
        },
        PipelineKind::Sector => D3d12ShaderPair {
            vertex: SECTOR_HLSL,
            pixel: SECTOR_HLSL,
        },
    }
}

// 从共享采样契约创建 b0 与条件 t0/s0 Root Signature。
fn create_root_signature(
    device: &ID3D12Device,
    contract: PipelineContract,
) -> Result<ID3D12RootSignature> {
    // 所有 pipeline 的唯一完整 Uniform 都绑定到 b0，供 VS/PS 共同读取。
    let constant_buffer = D3D12_ROOT_PARAMETER {
        ParameterType: D3D12_ROOT_PARAMETER_TYPE_CBV,
        Anonymous: D3D12_ROOT_PARAMETER_0 {
            Descriptor: D3D12_ROOT_DESCRIPTOR {
                ShaderRegister: 0,
                RegisterSpace: 0,
            },
        },
        ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
    };
    let shader_resource_range = D3D12_DESCRIPTOR_RANGE {
        RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
        NumDescriptors: 1,
        BaseShaderRegister: 0,
        RegisterSpace: 0,
        OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
    };
    let sampler_range = D3D12_DESCRIPTOR_RANGE {
        RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER,
        NumDescriptors: 1,
        BaseShaderRegister: 0,
        RegisterSpace: 0,
        OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
    };
    let mut parameters = vec![constant_buffer];
    match contract.sampling {
        PipelineSampling::None => {}
        PipelineSampling::PremultipliedColor
        | PipelineSampling::Coverage
        | PipelineSampling::Msdf => {
            parameters.push(descriptor_table_parameter(
                &shader_resource_range,
                D3D12_SHADER_VISIBILITY_PIXEL,
            ));
            parameters.push(descriptor_table_parameter(
                &sampler_range,
                D3D12_SHADER_VISIBILITY_PIXEL,
            ));
        }
    }
    let desc = D3D12_ROOT_SIGNATURE_DESC {
        NumParameters: parameters.len() as u32,
        pParameters: parameters.as_ptr(),
        NumStaticSamplers: 0,
        pStaticSamplers: std::ptr::null(),
        Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
    };
    let mut serialized = None;
    let mut errors = None;
    // SAFETY: desc 与其参数/范围指针在同步序列化期间保持有效，输出由 COM 智能指针接管。
    if let Err(error) = unsafe {
        D3D12SerializeRootSignature(
            &desc,
            D3D_ROOT_SIGNATURE_VERSION_1,
            &mut serialized,
            Some(&mut errors),
        )
    } {
        return Err(Error::new(
            Errc::PlatformError,
            format!(
                "D3d12Pipeline: D3D12SerializeRootSignature failed: {error}: {}",
                blob_detail(errors.as_ref())
            ),
        ));
    }
    let serialized = serialized.ok_or_else(|| {
        platform_error("D3d12Pipeline: root signature serialization returned no blob")
    })?;
    let bytes = blob_bytes(&serialized)?;
    // SAFETY: serialized blob 在同步创建期间存活且 bytes 覆盖完整序列化结果。
    unsafe { device.CreateRootSignature(0, bytes) }
        .map_err(|error| d3d12_error("ID3D12Device::CreateRootSignature", error))
}

// 创建一个指向单一范围的 descriptor table Root Parameter。
fn descriptor_table_parameter(
    range: &D3D12_DESCRIPTOR_RANGE,
    visibility: D3D12_SHADER_VISIBILITY,
) -> D3D12_ROOT_PARAMETER {
    D3D12_ROOT_PARAMETER {
        ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
        Anonymous: D3D12_ROOT_PARAMETER_0 {
            DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                NumDescriptorRanges: 1,
                pDescriptorRanges: range,
            },
        },
        ShaderVisibility: visibility,
    }
}

// 编译共享 HLSL 的单个入口并保留真实 bytecode owner。
fn compile_shader(source: &str, entry: &CStr, target: &CStr) -> Result<ID3DBlob> {
    let mut code = None;
    let mut errors = None;
    // SAFETY: source/entry/target 在同步编译期间有效，两个输出槽由 COM 接管。
    let result = unsafe {
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
    if let Err(error) = result {
        return Err(Error::new(
            Errc::PlatformError,
            format!(
                "D3d12Pipeline: D3DCompile {entry:?}/{target:?} failed: {error}: {}",
                blob_detail(errors.as_ref())
            ),
        ));
    }
    code.ok_or_else(|| {
        platform_error(format!(
            "D3d12Pipeline: D3DCompile {entry:?}/{target:?} returned no blob"
        ))
    })
}

// 从编译或序列化错误 blob 提取稳定诊断文本。
fn blob_detail(blob: Option<&ID3DBlob>) -> String {
    let Some(blob) = blob else {
        return String::new();
    };
    // SAFETY: blob 在本函数期间存活，返回指针与长度来自同一个 COM 对象。
    let pointer = unsafe { blob.GetBufferPointer() } as *const u8;
    let length = unsafe { blob.GetBufferSize() };
    if pointer.is_null() || length == 0 {
        return String::new();
    }
    // SAFETY: 指针已校验且 blob 保持存活，GetBufferSize 保证完整区域。
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
    String::from_utf8_lossy(bytes)
        .trim_end_matches(char::from(0))
        .to_owned()
}

// 借用存活 blob 的完整字节范围。
fn blob_bytes(blob: &ID3DBlob) -> Result<&[u8]> {
    // SAFETY: blob 存活，两个查询只读且来自同一对象。
    let pointer = unsafe { blob.GetBufferPointer() } as *const u8;
    let length = unsafe { blob.GetBufferSize() };
    if pointer.is_null() || length == 0 {
        return Err(platform_error("D3d12Pipeline: shader blob is empty"));
    }
    // SAFETY: 指针非空且 GetBufferSize 证明该区域可读。
    Ok(unsafe { std::slice::from_raw_parts(pointer, length) })
}

// 从共享属性序列机械构造 D3D12 输入布局。
fn d3d12_vertex_elements(layout: PipelineVertexLayout) -> Result<Vec<D3D12_INPUT_ELEMENT_DESC>> {
    if !layout.is_valid() {
        return Err(Error::new(
            Errc::InvalidArgument,
            "D3D12 RHI vertex layout is invalid",
        ));
    }
    Ok(layout
        .attributes()
        .iter()
        .map(|attribute| D3D12_INPUT_ELEMENT_DESC {
            SemanticName: d3d12_vertex_semantic(attribute.semantic()),
            SemanticIndex: 0,
            Format: d3d12_vertex_format(attribute.format()),
            InputSlot: 0,
            AlignedByteOffset: attribute.offset_bytes(),
            InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        })
        .collect())
}

// 穷尽映射共享顶点语义到 HLSL 输入名称。
fn d3d12_vertex_semantic(semantic: PipelineVertexSemantic) -> PCSTR {
    match semantic {
        PipelineVertexSemantic::Position => PCSTR::from_raw(c"POSITION".as_ptr().cast()),
        PipelineVertexSemantic::TextureCoordinate => PCSTR::from_raw(c"TEXCOORD".as_ptr().cast()),
        PipelineVertexSemantic::Color => PCSTR::from_raw(c"COLOR".as_ptr().cast()),
    }
}

// 穷尽映射共享顶点格式到 DXGI 格式。
fn d3d12_vertex_format(format: PipelineVertexFormat) -> DXGI_FORMAT {
    match format {
        PipelineVertexFormat::Float32x2 => DXGI_FORMAT_R32G32_FLOAT,
        PipelineVertexFormat::Float32x4 => DXGI_FORMAT_R32G32B32A32_FLOAT,
    }
}

// 从共享 blend.state() 创建完整 D3D12 颜色输出描述。
fn d3d12_blend_desc(contract: PipelineContract) -> D3D12_BLEND_DESC {
    let state = contract.blend.state();
    let target = D3D12_RENDER_TARGET_BLEND_DESC {
        BlendEnable: if state.enabled { TRUE } else { FALSE },
        LogicOpEnable: FALSE,
        SrcBlend: d3d12_blend_factor(state.source_color),
        DestBlend: d3d12_blend_factor(state.destination_color),
        BlendOp: d3d12_blend_operation(state.color_operation),
        SrcBlendAlpha: d3d12_blend_factor(state.source_alpha),
        DestBlendAlpha: d3d12_blend_factor(state.destination_alpha),
        BlendOpAlpha: d3d12_blend_operation(state.alpha_operation),
        LogicOp: D3D12_LOGIC_OP_NOOP,
        RenderTargetWriteMask: d3d12_color_write_mask(state.write_mask),
    };
    D3D12_BLEND_DESC {
        AlphaToCoverageEnable: if contract.multisample.alpha_to_coverage_enabled() {
            TRUE
        } else {
            FALSE
        },
        IndependentBlendEnable: FALSE,
        RenderTarget: [target; 8],
    }
}

// 穷尽映射共享 blend factor。
fn d3d12_blend_factor(factor: PipelineBlendFactor) -> D3D12_BLEND {
    match factor {
        PipelineBlendFactor::Zero => D3D12_BLEND_ZERO,
        PipelineBlendFactor::One => D3D12_BLEND_ONE,
        PipelineBlendFactor::SourceAlpha => D3D12_BLEND_SRC_ALPHA,
        PipelineBlendFactor::OneMinusSourceAlpha => D3D12_BLEND_INV_SRC_ALPHA,
    }
}

// 穷尽映射共享 blend operation。
fn d3d12_blend_operation(operation: PipelineBlendOperation) -> D3D12_BLEND_OP {
    match operation {
        PipelineBlendOperation::Add => D3D12_BLEND_OP_ADD,
    }
}

// 穷尽映射共享颜色写掩码。
fn d3d12_color_write_mask(mask: PipelineColorWriteMask) -> u8 {
    match mask {
        PipelineColorWriteMask::All => D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
    }
}

// 从共享 raster/multisample 创建完整 D3D12 rasterizer 描述。
fn d3d12_rasterizer_desc(contract: PipelineContract) -> D3D12_RASTERIZER_DESC {
    D3D12_RASTERIZER_DESC {
        FillMode: D3D12_FILL_MODE_SOLID,
        CullMode: match contract.raster.cull_mode {
            PipelineCullMode::None => D3D12_CULL_MODE_NONE,
        },
        FrontCounterClockwise: match contract.raster.front_face {
            PipelineFrontFace::CounterClockwise => TRUE,
        },
        DepthBias: 0,
        DepthBiasClamp: 0.0,
        SlopeScaledDepthBias: 0.0,
        DepthClipEnable: match contract.raster.depth_clip {
            PipelineDepthClip::Enabled => TRUE,
        },
        MultisampleEnable: if contract.multisample.raster_multisample_enabled() {
            TRUE
        } else {
            FALSE
        },
        AntialiasedLineEnable: FALSE,
        ForcedSampleCount: 0,
        ConservativeRaster: D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
    }
}

// 从共享 depth/stencil 创建完整 D3D12 禁用描述。
fn d3d12_depth_stencil_desc(contract: PipelineContract) -> D3D12_DEPTH_STENCIL_DESC {
    let face = D3D12_DEPTH_STENCILOP_DESC {
        StencilFailOp: D3D12_STENCIL_OP_KEEP,
        StencilDepthFailOp: D3D12_STENCIL_OP_KEEP,
        StencilPassOp: D3D12_STENCIL_OP_KEEP,
        StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS,
    };
    D3D12_DEPTH_STENCIL_DESC {
        DepthEnable: match contract.depth_stencil.depth {
            PipelineDepthState::Disabled => FALSE,
        },
        DepthWriteMask: match contract.depth_stencil.depth {
            PipelineDepthState::Disabled => D3D12_DEPTH_WRITE_MASK_ZERO,
        },
        DepthFunc: match contract.depth_stencil.depth {
            PipelineDepthState::Disabled => D3D12_COMPARISON_FUNC_ALWAYS,
        },
        StencilEnable: match contract.depth_stencil.stencil {
            PipelineStencilState::Disabled => FALSE,
        },
        StencilReadMask: 0,
        StencilWriteMask: 0,
        FrontFace: face,
        BackFace: face,
    }
}

// 穷尽映射共享 topology 到 D3D12 PSO 类型。
fn d3d12_topology_type(topology: PipelinePrimitiveTopology) -> D3D12_PRIMITIVE_TOPOLOGY_TYPE {
    match topology {
        PipelinePrimitiveTopology::TriangleList => D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
    }
}

// 穷尽映射共享 multisample 到 D3D12 采样描述。
fn d3d12_sample_desc(multisample: PipelineMultisampleState) -> DXGI_SAMPLE_DESC {
    match multisample {
        PipelineMultisampleState::SingleSample => DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
    }
}

// 为单一目标格式创建真实图形 PSO，并检查式释放描述中的临时 Root Signature 引用。
fn create_pipeline_state(
    device: &ID3D12Device,
    root_signature: &ID3D12RootSignature,
    vertex_shader: &ID3DBlob,
    pixel_shader: &ID3DBlob,
    input_layout: &[D3D12_INPUT_ELEMENT_DESC],
    fixed_state: D3d12PipelineFixedState,
    target_format: DXGI_FORMAT,
    operation: &'static str,
) -> Result<ID3D12PipelineState> {
    let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC::default();
    desc.pRootSignature = ManuallyDrop::new(Some(root_signature.clone()));
    desc.VS = shader_bytecode(vertex_shader)?;
    desc.PS = shader_bytecode(pixel_shader)?;
    desc.BlendState = fixed_state.blend;
    desc.SampleMask = fixed_state.sample_mask;
    desc.RasterizerState = fixed_state.rasterizer;
    desc.DepthStencilState = fixed_state.depth_stencil;
    desc.InputLayout = D3D12_INPUT_LAYOUT_DESC {
        pInputElementDescs: input_layout.as_ptr(),
        NumElements: input_layout.len() as u32,
    };
    desc.IBStripCutValue = D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED;
    desc.PrimitiveTopologyType = fixed_state.topology_type;
    desc.NumRenderTargets = 1;
    desc.RTVFormats[0] = target_format;
    desc.DSVFormat = DXGI_FORMAT_UNKNOWN;
    desc.SampleDesc = fixed_state.sample_desc;
    desc.NodeMask = 0;
    desc.Flags = D3D12_PIPELINE_STATE_FLAG_NONE;
    // SAFETY: desc 的 Root Signature、shader bytecode 与 input layout 在同步创建期间全部存活。
    let result = unsafe { device.CreateGraphicsPipelineState(&desc) }
        .map_err(|error| d3d12_error(operation, error));
    // SAFETY: 本函数刚写入一个克隆的 COM owner，且此后不再读取 desc.pRootSignature。
    unsafe { ManuallyDrop::drop(&mut desc.pRootSignature) };
    result
}

// 将存活 shader blob 投影为 PSO 同步消费的 bytecode 描述。
fn shader_bytecode(blob: &ID3DBlob) -> Result<D3D12_SHADER_BYTECODE> {
    let bytes = blob_bytes(blob)?;
    Ok(D3D12_SHADER_BYTECODE {
        pShaderBytecode: bytes.as_ptr().cast(),
        BytecodeLength: bytes.len(),
    })
}

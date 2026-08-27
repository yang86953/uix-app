//! Vulkan 对 platform thin RHI 固定状态的机械映射。
//!
//! 本模块不得重新定义 pipeline、资源或颜色语义；所有输入均来自
//! `platform::presentation::rhi`，这里只生成 Vulkan 原生描述。

use ash::vk;

use crate::core::{Errc, Error, Result};
use crate::platform::presentation::rhi::{
    IndexFormat, PipelineBlendFactor, PipelineBlendOperation, PipelineColorWriteMask,
    PipelineContract, PipelineCullMode, PipelineDepthClip, PipelineDepthState, PipelineDitherState,
    PipelineFrontFace, PipelineMultisampleState, PipelinePrimitiveTopology, PipelineStencilState,
    PipelineVertexFormat, SamplerAddressMode, SamplerDesc, SamplerFilter, SamplerMipMode,
    TextureFormat,
};

// 保存创建 Vulkan graphics pipeline 所需、且已经从共享契约穷尽映射的固定状态。
pub(super) struct VulkanPipelineState {
    pub(super) vertex_binding: vk::VertexInputBindingDescription,
    pub(super) vertex_attributes: Vec<vk::VertexInputAttributeDescription>,
    pub(super) topology: vk::PrimitiveTopology,
    pub(super) cull_mode: vk::CullModeFlags,
    pub(super) front_face: vk::FrontFace,
    pub(super) depth_clamp_enable: bool,
    pub(super) depth_test_enable: bool,
    pub(super) depth_write_enable: bool,
    pub(super) stencil_test_enable: bool,
    pub(super) blend_attachment: vk::PipelineColorBlendAttachmentState,
    pub(super) rasterization_samples: vk::SampleCountFlags,
    pub(super) alpha_to_coverage_enable: bool,
    pub(super) sample_mask: u32,
}

impl VulkanPipelineState {
    // 从 platform 唯一 pipeline ABI 构造 Vulkan 固定状态，拒绝无效顶点布局。
    pub(super) fn from_contract(contract: PipelineContract) -> Result<Self> {
        if !contract.vertex.is_valid() {
            return Err(Error::new(
                Errc::InvalidArgument,
                "Vulkan RHI vertex layout is invalid",
            ));
        }

        // Vulkan 没有可切换的颜色抖动状态；穷尽匹配用于锁定共享禁用语义。
        match contract.dither {
            PipelineDitherState::Disabled => {}
        }

        let (depth_test_enable, depth_write_enable) = match contract.depth_stencil.depth {
            PipelineDepthState::Disabled => (false, false),
        };
        let stencil_test_enable = match contract.depth_stencil.stencil {
            PipelineStencilState::Disabled => false,
        };
        let rasterization_samples = match contract.multisample {
            PipelineMultisampleState::SingleSample => vk::SampleCountFlags::TYPE_1,
        };

        let vertex_attributes = contract
            .vertex
            .attributes()
            .iter()
            .map(|attribute| vk::VertexInputAttributeDescription {
                location: attribute.location(),
                binding: 0,
                format: vertex_format(attribute.format()),
                offset: attribute.offset_bytes(),
            })
            .collect();

        Ok(Self {
            vertex_binding: vk::VertexInputBindingDescription {
                binding: 0,
                stride: contract.vertex.stride_bytes(),
                input_rate: vk::VertexInputRate::VERTEX,
            },
            vertex_attributes,
            topology: primitive_topology(contract.topology),
            cull_mode: cull_mode(contract.raster.cull_mode),
            front_face: front_face(contract.raster.front_face),
            depth_clamp_enable: depth_clamp_enable(contract.raster.depth_clip),
            depth_test_enable,
            depth_write_enable,
            stencil_test_enable,
            blend_attachment: blend_attachment(contract.blend.state()),
            rasterization_samples,
            alpha_to_coverage_enable: contract.multisample.alpha_to_coverage_enabled(),
            sample_mask: contract.multisample.sample_mask(),
        })
    }
}

// 保存 Vulkan sampler 创建需要的完整封闭状态。
pub(super) struct VulkanSamplerState {
    pub(super) min_filter: vk::Filter,
    pub(super) mag_filter: vk::Filter,
    pub(super) mipmap_mode: vk::SamplerMipmapMode,
    pub(super) address_mode_u: vk::SamplerAddressMode,
    pub(super) address_mode_v: vk::SamplerAddressMode,
    pub(super) address_mode_w: vk::SamplerAddressMode,
}

impl VulkanSamplerState {
    // 机械翻译共享 sampler 描述，不允许 Adapter 增加私有过滤或寻址策略。
    pub(super) fn from_desc(desc: SamplerDesc) -> Self {
        let filter = sampler_filter(desc.filter());
        let address_mode = sampler_address_mode(desc.address_mode());
        Self {
            min_filter: filter,
            mag_filter: filter,
            mipmap_mode: sampler_mip_mode(desc.mip_mode()),
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            address_mode_w: address_mode,
        }
    }
}

// 将共享纹理格式穷尽映射为 Vulkan 格式。
pub(super) const fn texture_format(format: TextureFormat) -> vk::Format {
    match format {
        TextureFormat::Bgra8Unorm => vk::Format::B8G8R8A8_UNORM,
        TextureFormat::Rgba8Unorm => vk::Format::R8G8B8A8_UNORM,
        TextureFormat::R8Unorm => vk::Format::R8_UNORM,
    }
}

// 将共享索引 ABI 穷尽映射为 Vulkan 索引类型。
pub(super) const fn index_type(format: IndexFormat) -> vk::IndexType {
    match format {
        IndexFormat::Uint32 => vk::IndexType::UINT32,
    }
}

// 返回所有 Vulkan 颜色 Image 操作共用的单 mip、单 layer 子资源范围。
pub(super) fn color_subresource_range() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1)
}

// 返回所有 Vulkan 颜色复制操作共用的单 mip、单 layer 子资源层。
pub(super) fn color_subresource_layers() -> vk::ImageSubresourceLayers {
    vk::ImageSubresourceLayers::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .mip_level(0)
        .base_array_layer(0)
        .layer_count(1)
}

fn primitive_topology(topology: PipelinePrimitiveTopology) -> vk::PrimitiveTopology {
    match topology {
        PipelinePrimitiveTopology::TriangleList => vk::PrimitiveTopology::TRIANGLE_LIST,
    }
}

fn vertex_format(format: PipelineVertexFormat) -> vk::Format {
    match format {
        PipelineVertexFormat::Float32 => vk::Format::R32_SFLOAT,
        PipelineVertexFormat::Float32x2 => vk::Format::R32G32_SFLOAT,
        PipelineVertexFormat::Float32x4 => vk::Format::R32G32B32A32_SFLOAT,
    }
}

fn cull_mode(mode: PipelineCullMode) -> vk::CullModeFlags {
    match mode {
        PipelineCullMode::None => vk::CullModeFlags::NONE,
    }
}

fn front_face(face: PipelineFrontFace) -> vk::FrontFace {
    match face {
        PipelineFrontFace::CounterClockwise => vk::FrontFace::COUNTER_CLOCKWISE,
    }
}

fn depth_clamp_enable(depth_clip: PipelineDepthClip) -> bool {
    match depth_clip {
        // Vulkan 关闭 depth clamp 即启用正常深度裁剪。
        PipelineDepthClip::Enabled => false,
    }
}

fn blend_factor(factor: PipelineBlendFactor) -> vk::BlendFactor {
    match factor {
        PipelineBlendFactor::Zero => vk::BlendFactor::ZERO,
        PipelineBlendFactor::One => vk::BlendFactor::ONE,
        PipelineBlendFactor::SourceAlpha => vk::BlendFactor::SRC_ALPHA,
        PipelineBlendFactor::OneMinusSourceAlpha => vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
    }
}

fn blend_operation(operation: PipelineBlendOperation) -> vk::BlendOp {
    match operation {
        PipelineBlendOperation::Add => vk::BlendOp::ADD,
    }
}

fn color_write_mask(mask: PipelineColorWriteMask) -> vk::ColorComponentFlags {
    match mask {
        PipelineColorWriteMask::All => {
            vk::ColorComponentFlags::R
                | vk::ColorComponentFlags::G
                | vk::ColorComponentFlags::B
                | vk::ColorComponentFlags::A
        }
    }
}

fn blend_attachment(
    state: crate::platform::presentation::rhi::PipelineBlendState,
) -> vk::PipelineColorBlendAttachmentState {
    vk::PipelineColorBlendAttachmentState::default()
        .blend_enable(state.enabled)
        .src_color_blend_factor(blend_factor(state.source_color))
        .dst_color_blend_factor(blend_factor(state.destination_color))
        .color_blend_op(blend_operation(state.color_operation))
        .src_alpha_blend_factor(blend_factor(state.source_alpha))
        .dst_alpha_blend_factor(blend_factor(state.destination_alpha))
        .alpha_blend_op(blend_operation(state.alpha_operation))
        .color_write_mask(color_write_mask(state.write_mask))
}

fn sampler_filter(filter: SamplerFilter) -> vk::Filter {
    match filter {
        SamplerFilter::Nearest => vk::Filter::NEAREST,
        SamplerFilter::Linear => vk::Filter::LINEAR,
    }
}

fn sampler_address_mode(mode: SamplerAddressMode) -> vk::SamplerAddressMode {
    match mode {
        SamplerAddressMode::ClampToEdge => vk::SamplerAddressMode::CLAMP_TO_EDGE,
    }
}

fn sampler_mip_mode(mode: SamplerMipMode) -> vk::SamplerMipmapMode {
    match mode {
        SamplerMipMode::SingleLevel => vk::SamplerMipmapMode::NEAREST,
    }
}

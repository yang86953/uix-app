//! Vulkan thin RHI 固定状态映射测试。

use super::*;
use crate::platform::presentation::rhi::{PipelineBlend, PipelineKind};

const PIPELINES: [PipelineKind; 12] = [
    PipelineKind::SolidMesh,
    PipelineKind::TexturedQuad,
    PipelineKind::GradientRect,
    PipelineKind::GlyphCoverageQuad,
    PipelineKind::ShapeRect,
    PipelineKind::ShapeRectAdditive,
    PipelineKind::BoxShadow,
    PipelineKind::TexturedQuadAdditive,
    PipelineKind::BlurPass,
    PipelineKind::MsdfGlyphQuad,
    PipelineKind::Sector,
    PipelineKind::LineSegment,
];

#[test]
fn all_shared_pipeline_contracts_map_to_vulkan_fixed_state() {
    for kind in PIPELINES {
        let contract = kind.contract();
        let state = VulkanPipelineState::from_contract(contract).expect("共享 pipeline 应可映射");

        assert_eq!(state.topology, vk::PrimitiveTopology::TRIANGLE_LIST);
        assert_eq!(state.cull_mode, vk::CullModeFlags::NONE);
        assert_eq!(state.front_face, vk::FrontFace::COUNTER_CLOCKWISE);
        assert!(!state.depth_clamp_enable);
        assert!(!state.depth_test_enable);
        assert!(!state.depth_write_enable);
        assert!(!state.stencil_test_enable);
        assert_eq!(state.rasterization_samples, vk::SampleCountFlags::TYPE_1);
        assert!(!state.alpha_to_coverage_enable);
        assert_eq!(state.sample_mask, u32::MAX);
        assert_eq!(state.vertex_binding.stride, contract.vertex.stride_bytes());
        assert_eq!(
            state.vertex_attributes.len(),
            contract.vertex.attributes().len()
        );
    }
}

#[test]
fn shared_blend_semantics_map_without_private_formula() {
    let straight = VulkanPipelineState::from_contract(PipelineKind::SolidMesh.contract())
        .expect("straight alpha pipeline 应可映射")
        .blend_attachment;
    assert_eq!(straight.blend_enable, vk::TRUE);
    assert_eq!(straight.src_color_blend_factor, vk::BlendFactor::SRC_ALPHA);
    assert_eq!(
        straight.dst_color_blend_factor,
        vk::BlendFactor::ONE_MINUS_SRC_ALPHA
    );

    let premultiplied = VulkanPipelineState::from_contract(PipelineKind::TexturedQuad.contract())
        .expect("premultiplied pipeline 应可映射")
        .blend_attachment;
    assert_eq!(premultiplied.src_color_blend_factor, vk::BlendFactor::ONE);
    assert_eq!(
        premultiplied.dst_color_blend_factor,
        vk::BlendFactor::ONE_MINUS_SRC_ALPHA
    );

    let additive = VulkanPipelineState::from_contract(PipelineKind::ShapeRectAdditive.contract())
        .expect("additive pipeline 应可映射")
        .blend_attachment;
    assert_eq!(additive.src_color_blend_factor, vk::BlendFactor::ONE);
    assert_eq!(additive.dst_color_blend_factor, vk::BlendFactor::ONE);

    let replace = VulkanPipelineState::from_contract(PipelineKind::BlurPass.contract())
        .expect("replace pipeline 应可映射")
        .blend_attachment;
    assert_eq!(replace.blend_enable, vk::FALSE);
    assert_eq!(
        PipelineBlend::Replace.state().destination_color,
        PipelineBlendFactor::Zero
    );
}

#[test]
fn shared_resource_formats_map_exhaustively() {
    assert_eq!(
        texture_format(TextureFormat::Bgra8Unorm),
        vk::Format::B8G8R8A8_UNORM
    );
    assert_eq!(
        texture_format(TextureFormat::Rgba8Unorm),
        vk::Format::R8G8B8A8_UNORM
    );
    assert_eq!(texture_format(TextureFormat::R8Unorm), vk::Format::R8_UNORM);
    assert_eq!(index_type(IndexFormat::Uint32), vk::IndexType::UINT32);
}

#[test]
fn shared_sampler_contract_maps_to_single_level_clamp() {
    let linear = VulkanSamplerState::from_desc(SamplerDesc::linear_clamp());
    assert_eq!(linear.min_filter, vk::Filter::LINEAR);
    assert_eq!(linear.mag_filter, vk::Filter::LINEAR);
    assert_eq!(linear.mipmap_mode, vk::SamplerMipmapMode::NEAREST);
    assert_eq!(linear.address_mode_u, vk::SamplerAddressMode::CLAMP_TO_EDGE);
    assert_eq!(linear.address_mode_v, vk::SamplerAddressMode::CLAMP_TO_EDGE);
    assert_eq!(linear.address_mode_w, vk::SamplerAddressMode::CLAMP_TO_EDGE);

    let nearest = VulkanSamplerState::from_desc(SamplerDesc::nearest_clamp());
    assert_eq!(nearest.min_filter, vk::Filter::NEAREST);
    assert_eq!(nearest.mag_filter, vk::Filter::NEAREST);
}

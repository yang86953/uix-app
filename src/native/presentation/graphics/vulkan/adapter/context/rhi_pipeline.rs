//! Vulkan 薄 RHI 的 Shader、Descriptor 与 PipelineLayout 资源。
//!
//! PipelineKind、顶点布局、uniform 大小、采样和混合语义均由 platform RHI
//! 冻结；本模块只把闭集映射为 Vulkan 原生对象。实际 VkPipeline 会在已知
//! RenderPass 格式后按需物化，避免把目标格式反向泄漏到共享接口。

use std::io::Cursor;

use ash::vk;

use crate::core::{Errc, Error, Result};
use crate::platform::presentation::rhi::{PipelineKind, PipelineSampling};

use super::super::rhi::VulkanPipelineState;
use super::vk_err;

// 保存共享 PipelineKind 唯一对应的预编译 SPIR-V 对。
struct VulkanShaderPair {
    vertex: &'static [u8],
    fragment: &'static [u8],
}

// Vulkan Pipeline 资源在目标格式未知阶段持有的 API 原生对象。
#[allow(dead_code)]
pub(super) struct VulkanRhiPipeline {
    // 保留共享身份，供后续 draw 物化目标格式变体。
    pub(super) kind: PipelineKind,
    // 保留从共享契约机械映射的固定状态。
    pub(super) state: VulkanPipelineState,
    // 持有顶点 SPIR-V 的原生模块。
    pub(super) vertex_shader: vk::ShaderModule,
    // 持有片元 SPIR-V 的原生模块。
    pub(super) fragment_shader: vk::ShaderModule,
    // 描述 set 0 的 uniform 与可选采样纹理 ABI。
    pub(super) descriptor_set_layout: vk::DescriptorSetLayout,
    // 固定一个 descriptor set 且不使用 push constants 的布局。
    pub(super) layout: vk::PipelineLayout,
}

impl VulkanRhiPipeline {
    // 从 platform PipelineKind 创建全部与目标格式无关的 Vulkan Pipeline 资源。
    pub(super) fn create(device: &ash::Device, kind: PipelineKind) -> Result<Self> {
        let contract = kind.contract();
        let state = VulkanPipelineState::from_contract(contract)?;
        let bindings = descriptor_bindings(contract.sampling);
        let descriptor_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        // SAFETY: bindings 来自封闭共享契约，生命周期覆盖同步创建调用。
        let descriptor_set_layout =
            unsafe { device.create_descriptor_set_layout(&descriptor_info, None) }
                .map_err(|error| vk_err("vkCreateDescriptorSetLayout RHI pipeline", error))?;

        let set_layouts = [descriptor_set_layout];
        let layout_info = vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts);
        // SAFETY: descriptor set layout 由同一 device 创建并保持存活。
        let layout = match unsafe { device.create_pipeline_layout(&layout_info, None) } {
            Ok(layout) => layout,
            Err(error) => {
                // SAFETY: descriptor set layout 尚未登记且没有下游引用。
                unsafe { device.destroy_descriptor_set_layout(descriptor_set_layout, None) };
                return Err(vk_err("vkCreatePipelineLayout RHI pipeline", error));
            }
        };

        let shaders = shader_pair(kind);
        let vertex_shader = match create_shader_module(device, shaders.vertex, "vertex") {
            Ok(shader) => shader,
            Err(error) => {
                // SAFETY: 两个布局尚未进入资源表，按创建逆序释放。
                unsafe {
                    device.destroy_pipeline_layout(layout, None);
                    device.destroy_descriptor_set_layout(descriptor_set_layout, None);
                }
                return Err(error);
            }
        };
        let fragment_shader = match create_shader_module(device, shaders.fragment, "fragment") {
            Ok(shader) => shader,
            Err(error) => {
                // SAFETY: 已创建对象尚未登记且没有提交引用，按创建逆序释放。
                unsafe {
                    device.destroy_shader_module(vertex_shader, None);
                    device.destroy_pipeline_layout(layout, None);
                    device.destroy_descriptor_set_layout(descriptor_set_layout, None);
                }
                return Err(error);
            }
        };

        Ok(Self {
            kind,
            state,
            vertex_shader,
            fragment_shader,
            descriptor_set_layout,
            layout,
        })
    }

    // 销毁一个已从共享资源表移交的 Vulkan Pipeline 资源。
    pub(super) fn destroy(self, device: &ash::Device) {
        // SAFETY: 所有对象由同一 device 创建；调用方保证没有在途提交引用。
        unsafe {
            device.destroy_shader_module(self.fragment_shader, None);
            device.destroy_shader_module(self.vertex_shader, None);
            device.destroy_pipeline_layout(self.layout, None);
            device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
        }
    }
}

// 创建 set 0 的固定绑定：uniform 始终为 0，采样图像与 sampler 合并为 1。
fn descriptor_bindings(sampling: PipelineSampling) -> Vec<vk::DescriptorSetLayoutBinding<'static>> {
    let mut bindings = vec![
        vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
    ];
    if !matches!(sampling, PipelineSampling::None) {
        bindings.push(
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        );
    }
    bindings
}

// 把已提交到源码树的 SPIR-V 字节创建为 Vulkan ShaderModule。
fn create_shader_module(
    device: &ash::Device,
    bytes: &'static [u8],
    stage: &'static str,
) -> Result<vk::ShaderModule> {
    let words = ash::util::read_spv(&mut Cursor::new(bytes)).map_err(|error| {
        Error::new(
            Errc::InvalidState,
            format!("Vulkan RHI {stage} SPIR-V is invalid: {error}"),
        )
    })?;
    let create_info = vk::ShaderModuleCreateInfo::default().code(&words);
    // SAFETY: read_spv 已验证字节对齐、长度和 SPIR-V magic，words 覆盖同步创建调用。
    unsafe { device.create_shader_module(&create_info, None) }
        .map_err(|error| vk_err("vkCreateShaderModule RHI pipeline", error))
}

// 穷尽映射 11 个共享 PipelineKind；Additive 只复用 shader，混合由固定状态区分。
fn shader_pair(kind: PipelineKind) -> VulkanShaderPair {
    match kind {
        PipelineKind::SolidMesh => VulkanShaderPair {
            vertex: include_bytes!("../shaders/spv/mesh.vert.spv"),
            fragment: include_bytes!("../shaders/spv/mesh.frag.spv"),
        },
        PipelineKind::TexturedQuad | PipelineKind::TexturedQuadAdditive => VulkanShaderPair {
            vertex: include_bytes!("../shaders/spv/sampled.vert.spv"),
            fragment: include_bytes!("../shaders/spv/textured.frag.spv"),
        },
        PipelineKind::GradientRect => VulkanShaderPair {
            vertex: include_bytes!("../shaders/spv/gradient.vert.spv"),
            fragment: include_bytes!("../shaders/spv/gradient.frag.spv"),
        },
        PipelineKind::GlyphCoverageQuad => VulkanShaderPair {
            vertex: include_bytes!("../shaders/spv/sampled.vert.spv"),
            fragment: include_bytes!("../shaders/spv/coverage.frag.spv"),
        },
        PipelineKind::ShapeRect | PipelineKind::ShapeRectAdditive => VulkanShaderPair {
            vertex: include_bytes!("../shaders/spv/shape.vert.spv"),
            fragment: include_bytes!("../shaders/spv/shape.frag.spv"),
        },
        PipelineKind::BoxShadow => VulkanShaderPair {
            vertex: include_bytes!("../shaders/spv/shadow.vert.spv"),
            fragment: include_bytes!("../shaders/spv/shadow.frag.spv"),
        },
        PipelineKind::BlurPass => VulkanShaderPair {
            vertex: include_bytes!("../shaders/spv/blur.vert.spv"),
            fragment: include_bytes!("../shaders/spv/blur.frag.spv"),
        },
        PipelineKind::MsdfGlyphQuad => VulkanShaderPair {
            vertex: include_bytes!("../shaders/spv/msdf.vert.spv"),
            fragment: include_bytes!("../shaders/spv/msdf.frag.spv"),
        },
        PipelineKind::Sector => VulkanShaderPair {
            vertex: include_bytes!("../shaders/spv/sector.vert.spv"),
            fragment: include_bytes!("../shaders/spv/sector.frag.spv"),
        },
    }
}

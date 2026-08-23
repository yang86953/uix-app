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
    // 按 render-target 格式缓存与兼容 RenderPass 共同创建的原生 Pipeline。
    variants: Vec<VulkanPipelineVariant>,
}

// 保存一个目标格式对应的 VkPipeline；LoadAction 不影响 render-pass 兼容性。
struct VulkanPipelineVariant {
    format: vk::Format,
    native: vk::Pipeline,
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
            variants: Vec::new(),
        })
    }

    // 返回或按需创建当前 render-target 格式的唯一 graphics pipeline。
    pub(super) fn materialize(
        &mut self,
        device: &ash::Device,
        format: vk::Format,
        render_pass: vk::RenderPass,
    ) -> Result<vk::Pipeline> {
        // 同一格式的 Clear/Load RenderPass 兼容，复用一个原生 pipeline。
        if let Some(variant) = self
            .variants
            .iter()
            .find(|variant| variant.format == format)
        {
            return Ok(variant.native);
        }

        let entry_name = c"main";
        let stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(self.vertex_shader)
                .name(entry_name),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(self.fragment_shader)
                .name(entry_name),
        ];
        let vertex_bindings = [self.state.vertex_binding];
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&vertex_bindings)
            .vertex_attribute_descriptions(&self.state.vertex_attributes);
        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(self.state.topology)
            .primitive_restart_enable(false);
        let viewport = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let rasterization = vk::PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(self.state.depth_clamp_enable)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(self.state.cull_mode)
            .front_face(self.state.front_face)
            .depth_bias_enable(false)
            .line_width(1.0);
        let sample_masks = [self.state.sample_mask];
        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(self.state.rasterization_samples)
            .sample_shading_enable(false)
            .sample_mask(&sample_masks)
            .alpha_to_coverage_enable(self.state.alpha_to_coverage_enable)
            .alpha_to_one_enable(false);
        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(self.state.depth_test_enable)
            .depth_write_enable(self.state.depth_write_enable)
            .depth_compare_op(vk::CompareOp::ALWAYS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(self.state.stencil_test_enable);
        let blend_attachments = [self.state.blend_attachment];
        let color_blend = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op_enable(false)
            .attachments(&blend_attachments);
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
        let create_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport)
            .rasterization_state(&rasterization)
            .multisample_state(&multisample)
            .depth_stencil_state(&depth_stencil)
            .color_blend_state(&color_blend)
            .dynamic_state(&dynamic)
            .layout(self.layout)
            .render_pass(render_pass)
            .subpass(0);
        // SAFETY: 所有状态、shader、layout 与兼容 render pass 均由同一 device 创建并存活。
        let native = match unsafe {
            device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                std::slice::from_ref(&create_info),
                None,
            )
        } {
            Ok(mut pipelines) => pipelines.pop().ok_or_else(|| {
                Error::new(
                    Errc::PlatformError,
                    "Vulkan RHI graphics pipeline creation returned no object",
                )
            })?,
            Err((partial, error)) => {
                // SAFETY: 失败返回的部分对象尚未登记，必须立即逐个销毁。
                unsafe {
                    for pipeline in partial {
                        device.destroy_pipeline(pipeline, None);
                    }
                }
                return Err(vk_err("vkCreateGraphicsPipelines RHI", error));
            }
        };
        self.variants.push(VulkanPipelineVariant { format, native });
        Ok(native)
    }

    // 销毁一个已从共享资源表移交的 Vulkan Pipeline 资源。
    pub(super) fn destroy(self, device: &ash::Device) {
        // SAFETY: 所有对象由同一 device 创建；调用方保证没有在途提交引用。
        unsafe {
            // VkPipeline 引用 layout 与 shader module，必须最先释放。
            for variant in self.variants.into_iter().rev() {
                device.destroy_pipeline(variant.native, None);
            }
            device.destroy_shader_module(self.fragment_shader, None);
            device.destroy_shader_module(self.vertex_shader, None);
            device.destroy_pipeline_layout(self.layout, None);
            device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
        }
    }
}

// 管理单帧 descriptor sets；所有 pool 只在上一 frame fence 完成后统一重置。
pub(super) struct VulkanDescriptorArena {
    pools: Vec<VulkanDescriptorPool>,
    active_pool: usize,
}

// 保存一个可复用 DescriptorPool 及其固定 set 容量。
struct VulkanDescriptorPool {
    native: vk::DescriptorPool,
    capacity: u32,
}

impl VulkanDescriptorArena {
    pub(super) const fn new() -> Self {
        Self {
            pools: Vec::new(),
            active_pool: 0,
        }
    }

    // 在上一提交完成后重置全部 pool，使 draw 数量不产生跨帧资源增长。
    pub(super) fn reset(&mut self, device: &ash::Device) -> Result<()> {
        for pool in &self.pools {
            // SAFETY: 调用方已等待唯一 frame fence，pool 内 set 不再被命令引用。
            unsafe {
                device
                    .reset_descriptor_pool(pool.native, vk::DescriptorPoolResetFlags::empty())
                    .map_err(|error| vk_err("vkResetDescriptorPool RHI frame", error))?;
            }
        }
        self.active_pool = 0;
        Ok(())
    }

    // 为一个 DrawPacket 分配 set；容量不足时按块增长，不设置静默上限。
    pub(super) fn allocate(
        &mut self,
        device: &ash::Device,
        layout: vk::DescriptorSetLayout,
    ) -> Result<vk::DescriptorSet> {
        loop {
            if self.active_pool == self.pools.len() {
                let capacity = self
                    .pools
                    .last()
                    .map_or(64, |pool| pool.capacity.saturating_mul(2).max(64));
                self.pools.push(create_descriptor_pool(device, capacity)?);
            }
            let pool = self.pools[self.active_pool].native;
            let layouts = [layout];
            let allocate_info = vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(pool)
                .set_layouts(&layouts);
            // SAFETY: pool 与 layout 同属当前 device，切片覆盖同步分配调用。
            match unsafe { device.allocate_descriptor_sets(&allocate_info) } {
                Ok(mut sets) => {
                    return sets.pop().ok_or_else(|| {
                        Error::new(
                            Errc::PlatformError,
                            "Vulkan RHI descriptor allocation returned no set",
                        )
                    });
                }
                Err(vk::Result::ERROR_OUT_OF_POOL_MEMORY)
                | Err(vk::Result::ERROR_FRAGMENTED_POOL) => {
                    self.active_pool += 1;
                }
                Err(error) => return Err(vk_err("vkAllocateDescriptorSets RHI frame", error)),
            }
        }
    }

    pub(super) fn shutdown(&mut self, device: &ash::Device) {
        // SAFETY: Context shutdown 已等待 device idle，pool 内 set 不再被引用。
        unsafe {
            for pool in self.pools.drain(..).rev() {
                device.destroy_descriptor_pool(pool.native, None);
            }
        }
        self.active_pool = 0;
    }
}

// 创建同时容纳 uniform 与可选 combined sampler 的 descriptor pool 块。
fn create_descriptor_pool(device: &ash::Device, capacity: u32) -> Result<VulkanDescriptorPool> {
    let sizes = [
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: capacity,
        },
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: capacity,
        },
    ];
    let create_info = vk::DescriptorPoolCreateInfo::default()
        .max_sets(capacity)
        .pool_sizes(&sizes);
    // SAFETY: create_info 只引用同步调用期间存活的固定容量数组。
    let native = unsafe { device.create_descriptor_pool(&create_info, None) }
        .map_err(|error| vk_err("vkCreateDescriptorPool RHI frame", error))?;
    Ok(VulkanDescriptorPool { native, capacity })
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

// 穷尽映射共享 PipelineKind；Additive 只复用 shader，混合由固定状态区分。
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
        PipelineKind::LineSegment => VulkanShaderPair {
            vertex: include_bytes!("../shaders/spv/line.vert.spv"),
            fragment: include_bytes!("../shaders/spv/line.frag.spv"),
        },
    }
}

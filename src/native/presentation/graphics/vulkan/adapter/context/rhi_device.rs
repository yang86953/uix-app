//! Vulkan `GraphicsDevice` 资源生命周期与阶段性命令入口。
//!
//! 资源身份、描述验证和陈旧句柄语义全部由 platform RHI 资源表拥有；本模块
//! 只持有 Vulkan 对象并执行 Vulkan 内存、上传与销毁操作。

use std::ptr;

use ash::vk;

use crate::core::{Errc, Error, Result};
use crate::platform::presentation::rhi::{
    BufferDesc, BufferHandle, BufferUsage, DrawPacket, GraphicsDevice,
    GraphicsDeviceCapabilities, LoadAction, RenderTargetHandle, RhiBufferResource,
    RhiBufferResourceTable, RhiBufferUpload, RhiBufferUploadPreflight, RhiResourceTable,
    SamplerDesc, SamplerHandle, SubmissionHandle, TextureCopy, UIX_COLOR_CONTRACT,
};

use super::super::rhi::VulkanSamplerState;
use super::transfer::find_memory_type;
use super::{VulkanContext, vk_err};

// 保存一个 Vulkan Buffer 及其由 platform 契约冻结的描述。
struct VulkanRhiBuffer {
    native: vk::Buffer,
    memory: vk::DeviceMemory,
    desc: BufferDesc,
}

impl RhiBufferResource for VulkanRhiBuffer {
    fn desc(&self) -> BufferDesc {
        self.desc
    }
}

// 保存一个 Vulkan Sampler 及其共享描述，供后续 Draw 资源预检使用。
struct VulkanRhiSampler {
    native: vk::Sampler,
    #[allow(dead_code)]
    desc: SamplerDesc,
}

// Vulkan context 唯一拥有的 RHI 资源状态。
pub(super) struct VulkanRhiDevice {
    buffers: RhiBufferResourceTable<VulkanRhiBuffer>,
    samplers: RhiResourceTable<SamplerHandle, VulkanRhiSampler>,
}

impl VulkanRhiDevice {
    pub(super) const fn new() -> Self {
        Self {
            buffers: RhiBufferResourceTable::new(),
            samplers: RhiResourceTable::new(),
        }
    }

    fn create_buffer(
        &mut self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        desc: BufferDesc,
    ) -> Result<BufferHandle> {
        let native_desc = desc.validate()?;
        let usage = match desc.usage() {
            BufferUsage::Vertex => vk::BufferUsageFlags::VERTEX_BUFFER,
            BufferUsage::Index => vk::BufferUsageFlags::INDEX_BUFFER,
            BufferUsage::Uniform => vk::BufferUsageFlags::UNIFORM_BUFFER,
        };
        let create_info = vk::BufferCreateInfo::default()
            .size(u64::from(native_desc.size_bytes_u32()))
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        // SAFETY: device 存活；创建描述只含经过 platform 共同值域校验的容量和用途。
        let native = unsafe { device.create_buffer(&create_info, None) }
            .map_err(|error| vk_err("vkCreateBuffer RHI", error))?;
        // SAFETY: native 是当前 device 刚创建且仍存活的 Buffer。
        let requirements = unsafe { device.get_buffer_memory_requirements(native) };
        let memory_type_index = match find_memory_type(
            instance,
            physical_device,
            requirements.memory_type_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            "host-visible coherent RHI buffer",
        ) {
            Ok(index) => index,
            Err(error) => {
                // SAFETY: Buffer 尚未进入资源表且没有绑定内存或提交引用。
                unsafe { device.destroy_buffer(native, None) };
                return Err(error);
            }
        };
        let allocate_info = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_type_index);
        // SAFETY: 内存类型来自同一 physical device 的需求交集。
        let memory = match unsafe { device.allocate_memory(&allocate_info, None) } {
            Ok(memory) => memory,
            Err(error) => {
                // SAFETY: Buffer 尚未登记且分配失败后不会再使用。
                unsafe { device.destroy_buffer(native, None) };
                return Err(vk_err("vkAllocateMemory RHI buffer", error));
            }
        };
        // SAFETY: native 与 memory 同属当前 device，容量满足查询所得 requirements。
        if let Err(error) = unsafe { device.bind_buffer_memory(native, memory, 0) } {
            // SAFETY: 绑定失败的两个对象尚未登记，按创建逆序释放。
            unsafe {
                device.free_memory(memory, None);
                device.destroy_buffer(native, None);
            }
            return Err(vk_err("vkBindBufferMemory RHI", error));
        }
        Ok(self.buffers.insert(VulkanRhiBuffer {
            native,
            memory,
            desc,
        }))
    }

    fn update_buffer(&mut self, device: &ash::Device, upload: RhiBufferUpload<'_>) -> Result<()> {
        let resource = self.buffers.get(upload.buffer())?;
        let validated = upload.validate(resource.desc)?;
        let size = u64::from(validated.size_bytes_u32());
        // SAFETY: memory 为 HOST_VISIBLE；范围已由共享描述验证且从偏移零开始。
        let mapped = unsafe {
            device.map_memory(resource.memory, 0, size, vk::MemoryMapFlags::empty())
        }
        .map_err(|error| vk_err("vkMapMemory RHI buffer", error))?;
        // SAFETY: mapped 指向至少 size 字节可写范围，源切片同长且二者不重叠。
        unsafe {
            ptr::copy_nonoverlapping(validated.data().as_ptr(), mapped.cast(), size as usize);
            // HOST_COHERENT 资源不需要显式 flush；本次映射由当前调用唯一拥有。
            device.unmap_memory(resource.memory);
        }
        Ok(())
    }

    fn create_sampler(&mut self, device: &ash::Device, desc: SamplerDesc) -> Result<SamplerHandle> {
        let state = VulkanSamplerState::from_desc(desc);
        let create_info = vk::SamplerCreateInfo::default()
            .min_filter(state.min_filter)
            .mag_filter(state.mag_filter)
            .mipmap_mode(state.mipmap_mode)
            .address_mode_u(state.address_mode_u)
            .address_mode_v(state.address_mode_v)
            .address_mode_w(state.address_mode_w)
            .mip_lod_bias(0.0)
            .anisotropy_enable(false)
            .compare_enable(false)
            .min_lod(0.0)
            .max_lod(0.0)
            .unnormalized_coordinates(false);
        // SAFETY: create_info 只含共享 sampler 契约的穷尽 Vulkan 映射。
        let native = unsafe { device.create_sampler(&create_info, None) }
            .map_err(|error| vk_err("vkCreateSampler RHI", error))?;
        Ok(self.samplers.insert(VulkanRhiSampler { native, desc }))
    }

    fn destroy_buffer(&mut self, device: &ash::Device, handle: BufferHandle) -> Result<()> {
        let resource = self.buffers.take(handle)?;
        // SAFETY: 资源表刚移交唯一对象，调用方保证没有在途提交引用。
        unsafe {
            device.destroy_buffer(resource.native, None);
            device.free_memory(resource.memory, None);
        }
        Ok(())
    }

    fn destroy_sampler(&mut self, device: &ash::Device, handle: SamplerHandle) -> Result<()> {
        let resource = self.samplers.take(handle)?;
        // SAFETY: 资源表刚移交唯一对象，Sampler 没有独立内存或子资源。
        unsafe { device.destroy_sampler(resource.native, None) };
        Ok(())
    }

    pub(super) fn shutdown(&mut self, device: &ash::Device) {
        // Sampler 与 Buffer 没有父子关系；均按各自创建逆序回收。
        for sampler in self.samplers.drain_reverse() {
            // SAFETY: device 已 idle 或 lost，资源表移交的对象不会再被引用。
            unsafe { device.destroy_sampler(sampler.native, None) };
        }
        for buffer in self.buffers.drain_reverse() {
            // SAFETY: device 已 idle 或 lost，先删 Buffer 再释放其绑定内存。
            unsafe {
                device.destroy_buffer(buffer.native, None);
                device.free_memory(buffer.memory, None);
            }
        }
    }
}

impl GraphicsDevice for VulkanContext {
    fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
        // 只声明已经真实实现的资源能力；完整基线完成前启动门禁必须拒绝 GpuNative。
        GraphicsDeviceCapabilities {
            color_contract: UIX_COLOR_CONTRACT,
            dynamic_buffers: true,
            texture_upload: false,
            texture_copy: false,
            texture_region_move: false,
            clear_rect: false,
            sampled_textures: false,
            render_to_texture: false,
            scissor: false,
            premultiplied_alpha_blend: false,
            additive_blend: false,
        }
    }

    fn create_buffer(&mut self, desc: BufferDesc) -> Result<BufferHandle> {
        let owner = self.active_device()?;
        let instance = self
            .runtime
            .as_ref()
            .ok_or_else(|| vulkan_rhi_unavailable("create_buffer after shutdown"))?
            .instance()
            .clone();
        let result = self.rhi_device.create_buffer(
            &instance,
            self.physical_device,
            &self.device,
            desc,
        );
        owner.observe(result)
    }

    fn update_buffer(&mut self, upload: RhiBufferUpload<'_>) -> Result<()> {
        let owner = self.active_device()?;
        let result = self.rhi_device.update_buffer(&self.device, upload);
        owner.observe(result)
    }

    fn preflight_buffer_upload(&self, upload: RhiBufferUploadPreflight) -> Result<()> {
        self.rhi_device.buffers.validate_upload(upload)
    }

    fn create_sampler(&mut self, desc: SamplerDesc) -> Result<SamplerHandle> {
        let owner = self.active_device()?;
        let result = self.rhi_device.create_sampler(&self.device, desc);
        owner.observe(result)
    }

    fn destroy_buffer(&mut self, buffer: BufferHandle) -> Result<()> {
        let owner = self.active_device()?;
        let result = self.rhi_device.destroy_buffer(&self.device, buffer);
        owner.observe(result)
    }

    fn destroy_sampler(&mut self, sampler: SamplerHandle) -> Result<()> {
        let owner = self.active_device()?;
        let result = self.rhi_device.destroy_sampler(&self.device, sampler);
        owner.observe(result)
    }

    fn begin_render_pass(&mut self, _: RenderTargetHandle, _: LoadAction) -> Result<()> {
        Err(vulkan_rhi_unavailable("begin_render_pass"))
    }

    fn draw(&mut self, _: DrawPacket) -> Result<()> {
        Err(vulkan_rhi_unavailable("draw"))
    }

    fn copy_texture(&mut self, _: TextureCopy) -> Result<()> {
        Err(vulkan_rhi_unavailable("copy_texture"))
    }

    fn end_render_pass(&mut self) -> Result<()> {
        Err(vulkan_rhi_unavailable("end_render_pass"))
    }

    fn submit(&mut self) -> Result<SubmissionHandle> {
        Err(vulkan_rhi_unavailable("submit"))
    }

    fn maintain(&mut self) -> Result<()> {
        self.active_device()?.ensure_healthy()
    }
}

fn vulkan_rhi_unavailable(operation: &'static str) -> Error {
    Error::new(
        Errc::NotImplemented,
        format!("Vulkan GPU-native RHI {operation} is not implemented yet"),
    )
}

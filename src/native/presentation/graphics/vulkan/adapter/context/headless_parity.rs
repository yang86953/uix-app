//! 显式 Vulkan parity 的无窗口生产 Device fixture。
//!
//! fixture 只省略 WSI Surface/Swapchain；资源、FramePlan 命令和提交仍经过
//! 生产 `VulkanContext` 的 `GraphicsDevice` 实现。

use super::*;
use crate::platform::presentation::rhi::{RhiExtent, TextureHandle};

impl VulkanContext {
    // 创建只允许 Device/offscreen 角色的真实 VulkanContext。
    pub(crate) fn new_headless_for_parity_test(extent: RhiExtent) -> Result<Self> {
        let (width, height) = extent.native_size_i32().ok_or_else(|| {
            Error::new(
                Errc::InvalidArgument,
                "Vulkan headless parity extent must fit the native positive domain",
            )
        })?;
        let runtime = VulkanRuntime::acquire()?;
        let instance = runtime.instance().clone();
        let selection = super::super::adapter::select_headless_test_queue(&instance)?;
        let queue_family_index = selection.family_index;
        let device_lease = runtime.acquire_device(selection)?;
        let physical_device = device_lease.physical_device();
        let adapter_info = device_lease.info().clone();
        let device = device_lease.device().clone();
        let swapchain_loader = ash::khr::swapchain::Device::new(&instance, &device);
        let command_pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family_index)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        // SAFETY: queue family 来自生产 adapter 选择，device 在完整 fixture 生命周期内存活。
        let command_pool = unsafe { device.create_command_pool(&command_pool_info, None) }
            .map_err(|error| vk_err("vkCreateCommandPool headless parity", error))?;
        let command_alloc = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        // SAFETY: command_pool 刚创建且仍由当前函数唯一持有。
        let command_buffer = match unsafe { device.allocate_command_buffers(&command_alloc) } {
            Ok(mut buffers) => match buffers.pop() {
                Some(buffer) => buffer,
                None => {
                    // SAFETY: 空分配没有在途 command，立即回收唯一 pool。
                    unsafe { device.destroy_command_pool(command_pool, None) };
                    return Err(Error::new(
                        Errc::PlatformError,
                        "Vulkan headless parity returned no command buffer",
                    ));
                }
            },
            Err(error) => {
                // SAFETY: 分配失败后 pool 尚未移交正式 owner。
                unsafe { device.destroy_command_pool(command_pool, None) };
                return Err(vk_err("vkAllocateCommandBuffers headless parity", error));
            }
        };
        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        // SAFETY: device 存活，signaled fence 在首次 FramePlan prepare 时可直接等待。
        let frame_fence = match unsafe { device.create_fence(&fence_info, None) } {
            Ok(fence) => fence,
            Err(error) => {
                // SAFETY: fence 创建失败且 command pool 没有在途工作。
                unsafe { device.destroy_command_pool(command_pool, None) };
                return Err(vk_err("vkCreateFence headless parity", error));
            }
        };
        // SAFETY: physical_device 来自同一 instance 的生产 adapter 选择。
        let uniform_alignment = unsafe {
            instance
                .get_physical_device_properties(physical_device)
                .limits
                .min_uniform_buffer_offset_alignment
                .max(1)
        };
        let native_extent = vk::Extent2D {
            width: extent.width,
            height: extent.height,
        };
        Ok(Self {
            runtime: Some(runtime.clone()),
            device_lease: Some(device_lease),
            surface_loader: ash::khr::surface::Instance::new(runtime.entry(), &instance),
            #[cfg(target_os = "linux")]
            _wayland_surface_loader: ash::khr::wayland_surface::Instance::new(
                runtime.entry(),
                &instance,
            ),
            #[cfg(windows)]
            _win32_surface_loader: ash::khr::win32_surface::Instance::new(
                runtime.entry(),
                &instance,
            ),
            #[cfg(target_os = "macos")]
            _metal_surface_loader: ash::ext::metal_surface::Instance::new(
                runtime.entry(),
                &instance,
            ),
            #[cfg(target_os = "macos")]
            metal_layer: std::ptr::null_mut(),
            surface: vk::SurfaceKHR::null(),
            physical_device,
            adapter_info,
            device,
            swapchain_loader,
            swapchain: vk::SwapchainKHR::null(),
            swapchain_images: Vec::new(),
            swapchain_image_views: Vec::new(),
            image_layouts: Vec::new(),
            swapchain_format: vk::Format::UNDEFINED,
            surface_supported_usage_flags: vk::ImageUsageFlags::empty(),
            extent: native_extent,
            command_pool,
            command_buffer,
            rhi_device: VulkanRhiDevice::new(uniform_alignment),
            surface_readback: VulkanSurfaceReadbackBuffer::new(),
            upload: UploadBuffer {
                buffer: vk::Buffer::null(),
                memory: vk::DeviceMemory::null(),
                size: 0,
            },
            image_available: vk::Semaphore::null(),
            render_finished: Vec::new(),
            present_fences: PresentFenceSet::empty(),
            present_lifetime: PresentLifetime::new(),
            frame_fence,
            acquired_frame: None,
            submitted_frame: None,
            surface_fault_for_parity: None,
            replace_present_sync_for_parity: false,
            surface_lifecycle:
                crate::platform::presentation::rhi::RhiSurfaceLifecycle::uninitialized(extent),
            native_surface: std::ptr::null_mut(),
            logical_width: width,
            logical_height: height,
            width,
            height,
            cpu_shadow: Vec::new(),
            shutdown: false,
        })
    }

    // 返回真实生产 adapter 选择产生的稳定设备/API 诊断。
    pub(crate) fn parity_adapter_diagnostic(&self) -> String {
        self.adapter_info.diagnostic_summary()
    }

    // 只向显式 parity 组合根报告本代真实 WSI usage、格式与投影能力。
    pub(crate) fn parity_surface_diagnostic(&self) -> String {
        format!(
            "surface_format={}({}); supported_usage_flags=0x{:08x}; transfer_src={}; readback={}",
            rhi_surface_readback::surface_readback_format_name(self.swapchain_format),
            self.swapchain_format.as_raw(),
            self.surface_supported_usage_flags.as_raw(),
            self.surface_supported_usage_flags
                .contains(vk::ImageUsageFlags::TRANSFER_SRC),
            crate::platform::presentation::rhi::GraphicsSurface::surface_capabilities(self)
                .readback,
        )
    }

    // 在 FramePlan submit 完成后机械回读一个离屏纹理。
    pub(crate) fn readback_texture_for_parity_test(&mut self, texture: TextureHandle) -> Vec<u8> {
        rhi_device::readback_production_texture(self, texture)
    }
}

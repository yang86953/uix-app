//! Vulkan instance 与逻辑 device 的显式所有权边界。

use std::rc::Rc;

use ash::{vk, Entry};

use crate::core::Result;

use super::adapter::{device_extension_names, QueueSelection, VulkanAdapterInfo};
use super::context::{loader_err, vk_err};
use super::surface::surface_instance_extensions;

/// 与窗口 surface 无关的 Vulkan instance 资源。
pub(super) struct VulkanRuntime {
    entry: Entry,
    instance: ash::Instance,
    surface_loader: ash::khr::surface::Instance,
}

impl VulkanRuntime {
    pub(super) fn new() -> Result<Rc<Self>> {
        let entry =
            unsafe { Entry::load() }.map_err(|error| loader_err("load Vulkan loader", error))?;
        let app_info = vk::ApplicationInfo::default()
            .application_name(c"uix")
            .application_version(1)
            .engine_name(c"uix")
            .engine_version(1)
            .api_version(vk::API_VERSION_1_0);
        let instance_extensions = surface_instance_extensions();
        #[cfg(target_os = "macos")]
        let instance_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&instance_extensions)
            .flags(vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR);
        #[cfg(not(target_os = "macos"))]
        let instance_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&instance_extensions);
        // SAFETY: extension 名称在调用期间有效，返回 instance 由本类型唯一销毁。
        let instance = unsafe { entry.create_instance(&instance_info, None) }
            .map_err(|error| vk_err("vkCreateInstance", error))?;
        let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);
        Ok(Rc::new(Self {
            entry,
            instance,
            surface_loader,
        }))
    }

    pub(super) fn entry(&self) -> &Entry {
        &self.entry
    }

    pub(super) fn instance(&self) -> &ash::Instance {
        &self.instance
    }

    pub(super) fn surface_loader(&self) -> &ash::khr::surface::Instance {
        &self.surface_loader
    }
}

impl Drop for VulkanRuntime {
    fn drop(&mut self) {
        // SAFETY: 所有 VulkanDevice 均持有本 runtime，故到此已无存活 device/surface。
        unsafe {
            self.instance.destroy_instance(None);
        }
    }
}

/// 与具体窗口 surface 无关、可被逐窗 context 持有的逻辑 device。
pub(super) struct VulkanDevice {
    _runtime: Rc<VulkanRuntime>,
    physical_device: vk::PhysicalDevice,
    info: VulkanAdapterInfo,
    device: ash::Device,
    queue: vk::Queue,
}

impl VulkanDevice {
    pub(super) fn new(runtime: Rc<VulkanRuntime>, selection: QueueSelection) -> Result<Rc<Self>> {
        let queue_priority = [1.0_f32];
        let queue_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(selection.family_index)
            .queue_priorities(&queue_priority);
        let extensions = device_extension_names(runtime.instance(), selection.physical_device)?;
        let device_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_info))
            .enabled_extension_names(&extensions);
        // SAFETY: selection 来自同一 runtime instance，扩展名称在调用期间有效。
        let device = unsafe {
            runtime
                .instance()
                .create_device(selection.physical_device, &device_info, None)
        }
        .map_err(|error| vk_err("vkCreateDevice", error))?;
        // SAFETY: 创建 device 时声明了该 queue family 的一个 queue。
        let queue = unsafe { device.get_device_queue(selection.family_index, 0) };
        Ok(Rc::new(Self {
            _runtime: runtime,
            physical_device: selection.physical_device,
            info: selection.info,
            device,
            queue,
        }))
    }

    pub(super) fn physical_device(&self) -> vk::PhysicalDevice {
        self.physical_device
    }

    pub(super) fn info(&self) -> &VulkanAdapterInfo {
        &self.info
    }

    pub(super) fn device(&self) -> &ash::Device {
        &self.device
    }

    pub(super) fn queue(&self) -> vk::Queue {
        self.queue
    }
}

impl Drop for VulkanDevice {
    fn drop(&mut self) {
        // SAFETY: context 的 checked shutdown 先回收全部 child；本对象唯一销毁 device。
        unsafe {
            self.device.destroy_device(None);
        }
    }
}

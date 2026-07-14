//! Vulkan 物理设备、扩展与 graphics+present queue 选择。

use std::ffi::CStr;

use ash::vk;

use crate::core::{Errc, Error, Result};

use super::context::vk_err;

pub(super) struct QueueSelection {
    pub(super) physical_device: vk::PhysicalDevice,
    pub(super) family_index: u32,
    pub(super) info: VulkanAdapterInfo,
}

/// 已选 Vulkan 物理设备与呈现队列的稳定诊断快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VulkanAdapterInfo {
    pub(crate) description: String,
    pub(crate) device_type: &'static str,
    pub(crate) vendor_id: u32,
    pub(crate) device_id: u32,
    pub(crate) api_version: u32,
    pub(crate) driver_version: u32,
    pub(crate) queue_family_index: u32,
}

impl VulkanAdapterInfo {
    pub(crate) fn diagnostic_summary(&self) -> String {
        format!(
            "adapter=\"{}\"; type={}; vendor={:#06X}; device={:#06X}; api={}; driver={:#010X}; queue_family={}",
            self.description,
            self.device_type,
            self.vendor_id,
            self.device_id,
            version_string(self.api_version),
            self.driver_version,
            self.queue_family_index
        )
    }
}

pub(super) fn select_queue(
    instance: &ash::Instance,
    surface_loader: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> Result<QueueSelection> {
    // SAFETY: instance 在枚举期间有效，ash 负责返回句柄数组的所有权。
    let physical_devices = unsafe { instance.enumerate_physical_devices() }
        .map_err(|err| vk_err("vkEnumeratePhysicalDevices", err))?;
    let mut failures = Vec::new();
    for physical_device in physical_devices {
        // SAFETY: physical_device 来自同一有效 instance 的枚举结果。
        let properties = unsafe { instance.get_physical_device_properties(physical_device) };
        if !device_supports_swapchain(instance, physical_device)? {
            failures.push(format!(
                "{}: missing VK_KHR_swapchain",
                properties_summary(&properties)
            ));
            continue;
        }
        // SAFETY: physical_device 来自同一有效 instance 的枚举结果。
        let queues =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
        for (index, queue) in queues.iter().enumerate() {
            // SAFETY: surface 与 physical_device 均属于仍存活的 instance。
            let supports_present = unsafe {
                surface_loader.get_physical_device_surface_support(
                    physical_device,
                    index as u32,
                    surface,
                )
            }
            .map_err(|err| vk_err("vkGetPhysicalDeviceSurfaceSupportKHR", err))?;
            if queue.queue_flags.contains(vk::QueueFlags::GRAPHICS) && supports_present {
                return Ok(QueueSelection {
                    physical_device,
                    family_index: index as u32,
                    info: adapter_info(&properties, index as u32),
                });
            }
        }
        failures.push(format!(
            "{}: no graphics+present queue",
            properties_summary(&properties)
        ));
    }
    Err(Error::new(
        Errc::PlatformError,
        format!(
            "VulkanContext: no graphics+present queue with VK_KHR_swapchain; candidates=[{}]",
            failures.join("; ")
        ),
    ))
}

pub(super) fn device_extension_names(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
) -> Result<Vec<*const std::ffi::c_char>> {
    #[cfg(target_os = "macos")]
    {
        let mut extensions = vec![ash::khr::swapchain::NAME.as_ptr()];
        if !device_has_extension(
            instance,
            physical_device,
            ash::khr::portability_subset::NAME,
        )? {
            return Err(Error::new(
                Errc::PlatformError,
                "VulkanContext: MoltenVK requires VK_KHR_portability_subset on the selected device",
            ));
        }
        extensions.push(ash::khr::portability_subset::NAME.as_ptr());
        Ok(extensions)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (instance, physical_device);
        Ok(vec![ash::khr::swapchain::NAME.as_ptr()])
    }
}

fn device_supports_swapchain(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
) -> Result<bool> {
    device_has_extension(instance, physical_device, ash::khr::swapchain::NAME)
}

fn device_has_extension(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    name: &CStr,
) -> Result<bool> {
    // SAFETY: physical_device 来自 instance，返回属性由 Vulkan 驱动按值填充。
    let extensions = unsafe { instance.enumerate_device_extension_properties(physical_device) }
        .map_err(|err| vk_err("vkEnumerateDeviceExtensionProperties", err))?;
    Ok(extensions.iter().any(|extension| {
        // SAFETY: Vulkan 保证 extension_name 是结构体内以 NUL 结尾的固定数组。
        let extension_name = unsafe { CStr::from_ptr(extension.extension_name.as_ptr()) };
        extension_name == name
    }))
}

fn adapter_info(
    properties: &vk::PhysicalDeviceProperties,
    queue_family_index: u32,
) -> VulkanAdapterInfo {
    VulkanAdapterInfo {
        description: device_name(properties),
        device_type: device_type_name(properties.device_type),
        vendor_id: properties.vendor_id,
        device_id: properties.device_id,
        api_version: properties.api_version,
        driver_version: properties.driver_version,
        queue_family_index,
    }
}

fn properties_summary(properties: &vk::PhysicalDeviceProperties) -> String {
    format!(
        "adapter=\"{}\" vendor={:#06X} device={:#06X}",
        device_name(properties),
        properties.vendor_id,
        properties.device_id
    )
}

fn device_name(properties: &vk::PhysicalDeviceProperties) -> String {
    // SAFETY: Vulkan 保证 device_name 是结构体内以 NUL 结尾的固定数组。
    unsafe { CStr::from_ptr(properties.device_name.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

fn device_type_name(device_type: vk::PhysicalDeviceType) -> &'static str {
    match device_type {
        vk::PhysicalDeviceType::DISCRETE_GPU => "discrete_gpu",
        vk::PhysicalDeviceType::INTEGRATED_GPU => "integrated_gpu",
        vk::PhysicalDeviceType::VIRTUAL_GPU => "virtual_gpu",
        vk::PhysicalDeviceType::CPU => "cpu",
        _ => "other",
    }
}

fn version_string(version: u32) -> String {
    format!(
        "{}.{}.{}",
        vk::api_version_major(version),
        vk::api_version_minor(version),
        vk::api_version_patch(version)
    )
}

//! Vulkan 物理设备、扩展与 graphics+present queue 选择。

use std::ffi::CStr;

use ash::vk;

use crate::core::{Errc, Error, Result};

use super::context::vk_err;

#[derive(Clone)]
pub(super) struct QueueSelection {
    pub(super) physical_device: vk::PhysicalDevice,
    pub(super) family_index: u32,
    pub(super) info: VulkanAdapterInfo,
    pub(super) extensions: DeviceExtensions,
}

#[derive(Clone, Copy)]
pub(super) struct DeviceExtensions {
    device_fault: bool,
    swapchain_maintenance1: bool,
    #[cfg(target_os = "macos")]
    portability_subset: bool,
}

impl DeviceExtensions {
    pub(super) fn supports_device_fault(self) -> bool {
        self.device_fault
    }

    pub(super) fn supports_swapchain_maintenance1(self) -> bool {
        self.swapchain_maintenance1
    }

    pub(super) fn enabled_names(
        self,
        enable_device_fault: bool,
        enable_swapchain_maintenance1: bool,
    ) -> Vec<*const std::ffi::c_char> {
        let mut extensions = vec![ash::khr::swapchain::NAME.as_ptr()];
        #[cfg(target_os = "macos")]
        extensions.push(ash::khr::portability_subset::NAME.as_ptr());
        if enable_device_fault {
            extensions.push(ash::ext::device_fault::NAME.as_ptr());
        }
        if enable_swapchain_maintenance1 {
            extensions.push(ash::ext::swapchain_maintenance1::NAME.as_ptr());
        }
        extensions
    }
}

#[derive(Default)]
pub(crate) struct AdapterSelectionRejections {
    entries: Vec<String>,
    first_error: Option<Error>,
}

impl AdapterSelectionRejections {
    pub(crate) fn reject(&mut self, adapter: impl AsRef<str>, reason: impl AsRef<str>) {
        self.entries
            .push(format!("{}: {}", adapter.as_ref(), reason.as_ref()));
    }

    pub(crate) fn reject_with_error(
        &mut self,
        adapter: impl AsRef<str>,
        operation: impl AsRef<str>,
        error: Error,
    ) {
        self.entries.push(format!(
            "{}: {} failed: {}",
            adapter.as_ref(),
            operation.as_ref(),
            error.message()
        ));
        if self.first_error.is_none() {
            self.first_error = Some(error);
        }
    }

    pub(crate) fn into_error(self) -> Error {
        let code = self
            .first_error
            .as_ref()
            .map_or(Errc::PlatformError, |error| error.code());
        let candidates = if self.entries.is_empty() {
            "none enumerated".to_owned()
        } else {
            self.entries.join("; ")
        };
        let error = Error::new(
            code,
            format!(
                "VulkanContext: no graphics+present queue with VK_KHR_swapchain; candidates=[{candidates}]"
            ),
        );
        match self.first_error {
            Some(source) => error.with_source(source),
            None => error,
        }
    }
}

pub(crate) fn select_graphics_present_queue<F>(
    queues: &[vk::QueueFamilyProperties],
    adapter: &str,
    rejections: &mut AdapterSelectionRejections,
    mut query_present: F,
) -> Option<u32>
where
    F: FnMut(u32) -> Result<bool>,
{
    let mut found_graphics = false;
    for (index, queue) in queues.iter().enumerate() {
        if !queue.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
            continue;
        }
        found_graphics = true;
        let index = index as u32;
        match query_present(index) {
            Ok(true) => return Some(index),
            Ok(false) => rejections.reject(
                adapter,
                format!("queue_family={index} lacks present support"),
            ),
            Err(error) => rejections.reject_with_error(
                adapter,
                format!("vkGetPhysicalDeviceSurfaceSupportKHR queue_family={index}"),
                error,
            ),
        }
    }
    if !found_graphics {
        rejections.reject(adapter, "no graphics queue");
    }
    None
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
    let mut failures = AdapterSelectionRejections::default();
    for physical_device in physical_devices {
        // SAFETY: physical_device 来自同一有效 instance 的枚举结果。
        let properties = unsafe { instance.get_physical_device_properties(physical_device) };
        let summary = properties_summary(&properties);
        let extension_support = match query_device_extensions(instance, physical_device) {
            Ok(extension_support) => extension_support,
            Err(error) => {
                failures.reject_with_error(&summary, "vkEnumerateDeviceExtensionProperties", error);
                continue;
            }
        };
        if !extension_support.supports_swapchain {
            failures.reject(&summary, "missing VK_KHR_swapchain");
            continue;
        }
        #[cfg(target_os = "macos")]
        if !extension_support.enabled.portability_subset {
            failures.reject(&summary, "missing VK_KHR_portability_subset");
            continue;
        }
        // SAFETY: physical_device 来自同一有效 instance 的枚举结果。
        let queues =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
        let selected_family =
            select_graphics_present_queue(&queues, &summary, &mut failures, |index| {
                // SAFETY: surface 与 physical_device 均属于仍存活的 instance。
                unsafe {
                    surface_loader.get_physical_device_surface_support(
                        physical_device,
                        index,
                        surface,
                    )
                }
                .map_err(|err| vk_err("vkGetPhysicalDeviceSurfaceSupportKHR", err))
            });
        if let Some(family_index) = selected_family {
            return Ok(QueueSelection {
                physical_device,
                family_index,
                info: adapter_info(&properties, family_index),
                extensions: extension_support.enabled,
            });
        }
    }
    Err(failures.into_error())
}


struct DeviceExtensionSupport {
    supports_swapchain: bool,
    enabled: DeviceExtensions,
}

fn query_device_extensions(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
) -> Result<DeviceExtensionSupport> {
    // SAFETY: physical_device 来自 instance，返回属性由 Vulkan 驱动按值填充。
    let extensions = unsafe { instance.enumerate_device_extension_properties(physical_device) }
        .map_err(|err| vk_err("vkEnumerateDeviceExtensionProperties", err))?;
    let has = |name: &CStr| {
        extensions.iter().any(|extension| {
            // SAFETY: Vulkan 保证 extension_name 是结构体内以 NUL 结尾的固定数组。
            let extension_name = unsafe { CStr::from_ptr(extension.extension_name.as_ptr()) };
            extension_name == name
        })
    };
    Ok(DeviceExtensionSupport {
        supports_swapchain: has(ash::khr::swapchain::NAME),
        enabled: DeviceExtensions {
            device_fault: has(ash::ext::device_fault::NAME),
            swapchain_maintenance1: has(ash::ext::swapchain_maintenance1::NAME),
            #[cfg(target_os = "macos")]
            portability_subset: has(ash::khr::portability_subset::NAME),
        },
    })
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

// GPU 验证专用实现位于 tests-src（模块级 include! 保持原作用域与 cfg），
// 仅 cargo test（含 RUSTFLAGS parity 入口）构建读取，发布包不携带。
#[cfg(test)]
include!("../../../../../../tests-src/native/presentation/graphics/vulkan/adapter/adapter_parity_fns.rs");

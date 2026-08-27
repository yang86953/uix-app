//! Vulkan 物理设备枚举。

use ash::{Entry, vk};

use crate::core::{Errc, Error, Result};
use crate::platform::graphics::{GpuAdapterInfo, GpuDeviceType, GraphicsBackend};

use super::context::{loader_err, vk_err};

/// 创建短生命周期的 Vulkan instance，并返回当前物理设备的 owned 快照。
pub(crate) fn enumerate_adapters() -> Result<Box<[GpuAdapterInfo]>> {
    // SAFETY: ash 负责加载平台 Vulkan 动态库及校验入口符号；Entry 在本函数内保持存活。
    let entry = unsafe { Entry::load() }
        .map_err(|error| loader_err("load Vulkan loader for adapter enumeration", error))?;
    let app_info = vk::ApplicationInfo::default()
        .application_name(c"uix")
        .application_version(1)
        .engine_name(c"uix")
        .engine_version(1)
        .api_version(vk::API_VERSION_1_0);

    #[cfg(target_os = "macos")]
    let portability_extensions = [ash::khr::portability_enumeration::NAME.as_ptr()];
    #[cfg(target_os = "macos")]
    let instance_info = vk::InstanceCreateInfo::default()
        .application_info(&app_info)
        .enabled_extension_names(&portability_extensions)
        .flags(vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR);
    #[cfg(not(target_os = "macos"))]
    let instance_info = vk::InstanceCreateInfo::default().application_info(&app_info);

    // SAFETY: instance 创建输入只借用本函数内存；instance 在枚举完成后显式销毁。
    let instance = unsafe { entry.create_instance(&instance_info, None) }
        .map_err(|error| vk_err("vkCreateInstance for adapter enumeration", error))?;
    let result = enumerate_physical_devices(&instance);
    // SAFETY: 本函数未创建 child device 或 surface，instance 可以在返回前直接销毁。
    unsafe {
        instance.destroy_instance(None);
    }
    result
}

fn enumerate_physical_devices(instance: &ash::Instance) -> Result<Box<[GpuAdapterInfo]>> {
    // SAFETY: instance 在整个调用期间有效，ash 返回由调用方拥有的句柄数组。
    let physical_devices = unsafe { instance.enumerate_physical_devices() }
        .map_err(|error| vk_err("vkEnumeratePhysicalDevices for adapter enumeration", error))?;
    if physical_devices.is_empty() {
        return Err(Error::new(
            Errc::NotFound,
            "Platform::gpu_adapters: Vulkan reported no physical devices",
        ));
    }

    let adapters = physical_devices
        .into_iter()
        .map(|physical_device| {
            // SAFETY: physical_device 来自同一有效 instance 的枚举结果；属性按值返回。
            let properties = unsafe { instance.get_physical_device_properties(physical_device) };
            GpuAdapterInfo::new(
                GraphicsBackend::Vulkan,
                map_device_type(properties.device_type),
                non_empty(device_name(&properties)),
                (properties.vendor_id != 0).then_some(properties.vendor_id),
                (properties.device_id != 0).then_some(properties.device_id),
                driver_description(properties.driver_version),
            )
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    Ok(adapters)
}

fn device_name(properties: &vk::PhysicalDeviceProperties) -> String {
    // SAFETY: Vulkan 保证 device_name 是结构体内以 NUL 结尾的固定数组。
    unsafe { std::ffi::CStr::from_ptr(properties.device_name.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

fn map_device_type(device_type: vk::PhysicalDeviceType) -> GpuDeviceType {
    match device_type {
        vk::PhysicalDeviceType::DISCRETE_GPU => GpuDeviceType::Discrete,
        vk::PhysicalDeviceType::INTEGRATED_GPU => GpuDeviceType::Integrated,
        vk::PhysicalDeviceType::VIRTUAL_GPU => GpuDeviceType::Virtual,
        vk::PhysicalDeviceType::CPU => GpuDeviceType::Software,
        _ => GpuDeviceType::Unknown,
    }
}

fn driver_description(driver_version: u32) -> Option<String> {
    (driver_version != 0).then(|| format!("Vulkan driver {driver_version:#010X}"))
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

//! 可选 Vulkan device-fault feature 协商与诊断边界。

use std::ffi::CStr;

use ash::{vk, Entry};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeviceFaultFeatureQueryMode {
    Core11,
    Khr,
    Unavailable,
}

pub(super) struct DeviceFaultFeatureQuery {
    mode: DeviceFaultFeatureQueryMode,
    khr: Option<ash::khr::get_physical_device_properties2::Instance>,
}

#[derive(Clone, Copy, Default)]
pub(super) struct DeviceFaultSupport {
    reporting: bool,
}

impl DeviceFaultSupport {
    pub(super) fn reporting(self) -> bool {
        self.reporting
    }

    pub(super) fn requested_features(self) -> vk::PhysicalDeviceFaultFeaturesEXT<'static> {
        vk::PhysicalDeviceFaultFeaturesEXT::default().device_fault(self.reporting)
    }
}

pub(super) fn configure_instance(
    entry: &Entry,
    extensions: &mut Vec<*const std::ffi::c_char>,
) -> (u32, DeviceFaultFeatureQueryMode) {
    // SAFETY: entry 已成功加载；该查询不创建或借用外部 Vulkan 对象。
    let loader_version = match unsafe { entry.try_enumerate_instance_version() } {
        Ok(version) => version,
        Err(error) => {
            crate::core::log::warn_fn(format!(
                "VulkanContext: vkEnumerateInstanceVersion failed; device fault diagnostics disabled: {error:?}"
            ));
            None
        }
    };
    let khr_available = loader_version.is_none_or(|version| version < vk::API_VERSION_1_1)
        && instance_has_extension(entry, ash::khr::get_physical_device_properties2::NAME);
    let (api_version, mode) = select_feature_query_mode(loader_version, khr_available);
    if mode == DeviceFaultFeatureQueryMode::Khr {
        extensions.push(ash::khr::get_physical_device_properties2::NAME.as_ptr());
    }
    (api_version, mode)
}

pub(crate) fn select_feature_query_mode(
    loader_version: Option<u32>,
    khr_available: bool,
) -> (u32, DeviceFaultFeatureQueryMode) {
    if loader_version.is_some_and(|version| version >= vk::API_VERSION_1_1) {
        return (vk::API_VERSION_1_1, DeviceFaultFeatureQueryMode::Core11);
    }
    if khr_available {
        return (vk::API_VERSION_1_0, DeviceFaultFeatureQueryMode::Khr);
    }
    (
        vk::API_VERSION_1_0,
        DeviceFaultFeatureQueryMode::Unavailable,
    )
}

impl DeviceFaultFeatureQuery {
    pub(super) fn new(
        mode: DeviceFaultFeatureQueryMode,
        entry: &Entry,
        instance: &ash::Instance,
    ) -> Self {
        let khr = (mode == DeviceFaultFeatureQueryMode::Khr)
            .then(|| ash::khr::get_physical_device_properties2::Instance::new(entry, instance));
        Self { mode, khr }
    }

    pub(super) fn query(
        &self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        extension_available: bool,
    ) -> DeviceFaultSupport {
        if !extension_available || self.mode == DeviceFaultFeatureQueryMode::Unavailable {
            return DeviceFaultSupport::default();
        }
        let mut fault_features = vk::PhysicalDeviceFaultFeaturesEXT::default();
        let mut features = vk::PhysicalDeviceFeatures2::default().push_next(&mut fault_features);
        // SAFETY: physical_device 属于 instance；pNext 在调用期间指向存活且可写的 feature 结构。
        unsafe {
            match (&self.mode, &self.khr) {
                (DeviceFaultFeatureQueryMode::Core11, _) => {
                    instance.get_physical_device_features2(physical_device, &mut features);
                }
                (DeviceFaultFeatureQueryMode::Khr, Some(khr)) => {
                    khr.get_physical_device_features2(physical_device, &mut features);
                }
                _ => return DeviceFaultSupport::default(),
            }
        }
        DeviceFaultSupport {
            reporting: fault_features.device_fault == vk::TRUE,
        }
    }
}

fn instance_has_extension(entry: &Entry, name: &CStr) -> bool {
    // SAFETY: entry 已加载；None 表示枚举全局 instance extensions。
    let extensions = match unsafe { entry.enumerate_instance_extension_properties(None) } {
        Ok(extensions) => extensions,
        Err(error) => {
            crate::core::log::warn_fn(format!(
                "VulkanContext: vkEnumerateInstanceExtensionProperties failed; optional device fault diagnostics disabled: {error:?}"
            ));
            return false;
        }
    };
    extensions.iter().any(|extension| {
        // SAFETY: Vulkan 保证 extension_name 是结构体内以 NUL 结尾的固定数组。
        unsafe { CStr::from_ptr(extension.extension_name.as_ptr()) == name }
    })
}

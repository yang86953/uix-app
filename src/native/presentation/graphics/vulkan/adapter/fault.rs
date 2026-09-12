//! 可选 Vulkan device-fault feature 协商与诊断边界。

use std::cell::RefCell;
use std::ffi::CStr;
use std::ptr;

use ash::{Entry, vk};

use crate::core::{Errc, Error};

const MAX_ADDRESS_INFOS: usize = 16;
const MAX_VENDOR_INFOS: usize = 16;

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

pub(super) struct DeviceFaultReporter {
    loader: ash::ext::device_fault::Device,
}

#[derive(Default)]
pub(crate) struct DeviceLossState {
    first_error: RefCell<Option<Error>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeviceFaultAddress {
    pub(crate) address_type: i32,
    pub(crate) reported_address: u64,
    pub(crate) address_precision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeviceFaultVendor {
    pub(crate) description: String,
    pub(crate) code: u64,
    pub(crate) data: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeviceFaultReport {
    pub(crate) description: String,
    pub(crate) addresses: Vec<DeviceFaultAddress>,
    pub(crate) vendors: Vec<DeviceFaultVendor>,
    pub(crate) vendor_binary_bytes: u64,
    pub(crate) truncated: bool,
}

impl DeviceFaultReport {
    pub(crate) fn diagnostic_summary(&self) -> String {
        let addresses = if self.addresses.is_empty() {
            "none".to_owned()
        } else {
            self.addresses
                .iter()
                .map(|address| {
                    format!(
                        "{}@{:#018X}±{}",
                        address_type_name(address.address_type),
                        address.reported_address,
                        address.address_precision
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        };
        let vendors = if self.vendors.is_empty() {
            "none".to_owned()
        } else {
            self.vendors
                .iter()
                .map(|vendor| {
                    format!(
                        "\"{}\" code={:#018X} data={:#018X}",
                        normalize_text(&vendor.description),
                        vendor.code,
                        vendor.data
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        };
        format!(
            "VK_EXT_device_fault: description=\"{}\"; addresses=[{}]; vendors=[{}]; vendor_binary_bytes={}; truncated={}",
            normalize_text(&self.description),
            addresses,
            vendors,
            self.vendor_binary_bytes,
            self.truncated
        )
    }
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
            tracing::warn!(
                "VulkanContext: vkEnumerateInstanceVersion failed; device fault diagnostics disabled: {error:?}"
            );
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
        if !self.query_features2(instance, physical_device, &mut features) {
            return DeviceFaultSupport::default();
        }
        DeviceFaultSupport {
            reporting: fault_features.device_fault == vk::TRUE,
        }
    }

    pub(super) fn query_swapchain_maintenance1(
        &self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        extension_available: bool,
    ) -> bool {
        if !extension_available || self.mode == DeviceFaultFeatureQueryMode::Unavailable {
            return false;
        }
        let mut maintenance = vk::PhysicalDeviceSwapchainMaintenance1FeaturesEXT::default();
        let mut features = vk::PhysicalDeviceFeatures2::default().push_next(&mut maintenance);
        self.query_features2(instance, physical_device, &mut features)
            && maintenance.swapchain_maintenance1 == vk::TRUE
    }

    fn query_features2(
        &self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        features: &mut vk::PhysicalDeviceFeatures2<'_>,
    ) -> bool {
        // SAFETY: physical_device 属于 instance；pNext 在调用期间指向存活且可写的 feature 结构。
        unsafe {
            match (&self.mode, &self.khr) {
                (DeviceFaultFeatureQueryMode::Core11, _) => {
                    instance.get_physical_device_features2(physical_device, features);
                    true
                }
                (DeviceFaultFeatureQueryMode::Khr, Some(khr)) => {
                    khr.get_physical_device_features2(physical_device, features);
                    true
                }
                _ => false,
            }
        }
    }
}

impl DeviceFaultReporter {
    pub(super) fn new(instance: &ash::Instance, device: &ash::Device) -> Self {
        Self {
            loader: ash::ext::device_fault::Device::new(instance, device),
        }
    }

    pub(super) fn collect(&self) -> std::result::Result<DeviceFaultReport, vk::Result> {
        let mut counts = vk::DeviceFaultCountsEXT::default();
        let status = self.get_fault_info(&mut counts, ptr::null_mut());
        accept_fault_query_status(status)?;

        let available_addresses = counts.address_info_count as usize;
        let available_vendors = counts.vendor_info_count as usize;
        let available_vendor_binary = counts.vendor_binary_size;
        let mut addresses = vec![
            vk::DeviceFaultAddressInfoEXT::default();
            available_addresses.min(MAX_ADDRESS_INFOS)
        ];
        let mut vendors =
            vec![vk::DeviceFaultVendorInfoEXT::default(); available_vendors.min(MAX_VENDOR_INFOS)];
        counts.address_info_count = addresses.len() as u32;
        counts.vendor_info_count = vendors.len() as u32;
        // 二进制 crash dump 需要厂商工具解释且可能很大；框架只采集有界文本诊断。
        counts.vendor_binary_size = 0;
        let mut info = vk::DeviceFaultInfoEXT::default();
        if !addresses.is_empty() {
            info.p_address_infos = addresses.as_mut_ptr();
        }
        if !vendors.is_empty() {
            info.p_vendor_infos = vendors.as_mut_ptr();
        }
        let status = self.get_fault_info(&mut counts, &mut info);
        accept_fault_query_status(status)?;

        addresses.truncate((counts.address_info_count as usize).min(addresses.len()));
        vendors.truncate((counts.vendor_info_count as usize).min(vendors.len()));
        let description = info.description_as_c_str().map_or_else(
            |_| String::new(),
            |text| text.to_string_lossy().into_owned(),
        );
        let addresses = addresses
            .into_iter()
            .map(|address| DeviceFaultAddress {
                address_type: address.address_type.as_raw(),
                reported_address: address.reported_address,
                address_precision: address.address_precision,
            })
            .collect();
        let vendors = vendors
            .into_iter()
            .map(|vendor| DeviceFaultVendor {
                description: vendor.description_as_c_str().map_or_else(
                    |_| String::new(),
                    |text| text.to_string_lossy().into_owned(),
                ),
                code: vendor.vendor_fault_code,
                data: vendor.vendor_fault_data,
            })
            .collect();
        Ok(DeviceFaultReport {
            description,
            addresses,
            vendors,
            vendor_binary_bytes: available_vendor_binary,
            truncated: status == vk::Result::INCOMPLETE
                || available_addresses > MAX_ADDRESS_INFOS
                || available_vendors > MAX_VENDOR_INFOS
                || available_vendor_binary > 0,
        })
    }

    fn enrich(&self, error: Error) -> Error {
        match self.collect() {
            Ok(report) => error.with_appended_source(Error::new(
                Errc::GraphicsDeviceLost,
                report.diagnostic_summary(),
            )),
            Err(status) => error.with_appended_source(Error::new(
                Errc::GraphicsDeviceLost,
                format!("VK_EXT_device_fault: vkGetDeviceFaultInfoEXT failed: {status:?}"),
            )),
        }
    }

    fn get_fault_info(
        &self,
        counts: &mut vk::DeviceFaultCountsEXT<'_>,
        info: *mut vk::DeviceFaultInfoEXT<'_>,
    ) -> vk::Result {
        // SAFETY: loader 只在启用 VK_EXT_device_fault 后构造；device 已由驱动报告 lost，
        // counts 与可选 info 指向本调用期间存活且按声明容量分配的可写结构。
        unsafe { (self.loader.fp().get_device_fault_info_ext)(self.loader.device(), counts, info) }
    }
}

impl DeviceLossState {
    pub(super) fn record(&self, error: Error, reporter: Option<&DeviceFaultReporter>) -> Error {
        self.record_with(error, |error| match reporter {
            Some(reporter) => reporter.enrich(error),
            None => error,
        })
    }

    pub(crate) fn record_with(&self, error: Error, enrich: impl FnOnce(Error) -> Error) -> Error {
        if error.code() != Errc::GraphicsDeviceLost {
            return error;
        }
        if let Some(first_error) = self.first_error.borrow().clone() {
            return error.with_appended_source(first_error);
        }
        let error = enrich(error);
        *self.first_error.borrow_mut() = Some(error.clone());
        error
    }

    pub(crate) fn peer_error(&self) -> Option<Error> {
        self.first_error.borrow().as_ref().map(|first_error| {
            Error::new(
                Errc::GraphicsDeviceLost,
                "VulkanContext: shared logical device is lost",
            )
            .with_source(first_error.clone())
        })
    }


    pub(super) fn is_lost(&self) -> bool {
        self.first_error.borrow().is_some()
    }
}

fn accept_fault_query_status(status: vk::Result) -> std::result::Result<(), vk::Result> {
    match status {
        vk::Result::SUCCESS | vk::Result::INCOMPLETE => Ok(()),
        error => Err(error),
    }
}

fn normalize_text(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalized = normalized.replace('"', "'");
    if normalized.is_empty() {
        "unavailable".to_owned()
    } else {
        normalized
    }
}

fn address_type_name(address_type: i32) -> &'static str {
    let address_type = vk::DeviceFaultAddressTypeEXT::from_raw(address_type);
    match address_type {
        vk::DeviceFaultAddressTypeEXT::READ_INVALID => "read_invalid",
        vk::DeviceFaultAddressTypeEXT::WRITE_INVALID => "write_invalid",
        vk::DeviceFaultAddressTypeEXT::EXECUTE_INVALID => "execute_invalid",
        vk::DeviceFaultAddressTypeEXT::INSTRUCTION_POINTER_UNKNOWN => "ip_unknown",
        vk::DeviceFaultAddressTypeEXT::INSTRUCTION_POINTER_INVALID => "ip_invalid",
        vk::DeviceFaultAddressTypeEXT::INSTRUCTION_POINTER_FAULT => "ip_fault",
        _ => "unknown",
    }
}

fn instance_has_extension(entry: &Entry, name: &CStr) -> bool {
    // SAFETY: entry 已加载；None 表示枚举全局 instance extensions。
    let extensions = match unsafe { entry.enumerate_instance_extension_properties(None) } {
        Ok(extensions) => extensions,
        Err(error) => {
            tracing::warn!(
                "VulkanContext: vkEnumerateInstanceExtensionProperties failed; optional device fault diagnostics disabled: {error:?}"
            );
            return false;
        }
    };
    extensions.iter().any(|extension| {
        // SAFETY: Vulkan 保证 extension_name 是结构体内以 NUL 结尾的固定数组。
        unsafe { CStr::from_ptr(extension.extension_name.as_ptr()) == name }
    })
}

// GPU 验证专用实现位于 tests-src（模块级 include! 保持原作用域与 cfg），
// 仅 cargo test（含 RUSTFLAGS parity 入口）构建读取，发布包不携带。
#[cfg(test)]
include!("../../../../../../tests-src/native/presentation/graphics/vulkan/adapter/fault_parity_fns.rs");

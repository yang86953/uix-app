//! Vulkan instance 与逻辑 device 的显式所有权边界。

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use ash::{vk, Entry};

use crate::core::{Error, Result};

use super::adapter::{QueueSelection, VulkanAdapterInfo};
use super::context::{loader_err, vk_err};
use super::fault::{
    configure_instance, DeviceFaultFeatureQuery, DeviceFaultReporter, DeviceLossState,
};
use super::surface::surface_instance_extensions;

type DeviceKey = (vk::PhysicalDevice, u32);

thread_local! {
    /// Vulkan 调用受 `ThreadBoundGraphicsContext` 约束；线程本地弱引用既复用父资源，
    /// 又不延长最后一个窗口 context 的生命周期。
    static THREAD_RUNTIME: RefCell<Weak<VulkanRuntime>> = RefCell::new(Weak::new());
}

/// 与窗口 surface 无关的 Vulkan instance 资源。
pub(super) struct VulkanRuntime {
    entry: Entry,
    instance: ash::Instance,
    surface_loader: ash::khr::surface::Instance,
    fault_feature_query: DeviceFaultFeatureQuery,
    devices: RefCell<HashMap<DeviceKey, Weak<VulkanDevice>>>,
}

impl VulkanRuntime {
    pub(super) fn acquire() -> Result<Rc<Self>> {
        THREAD_RUNTIME.with(|slot| {
            if let Some(runtime) = slot.borrow().upgrade() {
                return Ok(runtime);
            }
            let runtime = Self::create()?;
            *slot.borrow_mut() = Rc::downgrade(&runtime);
            Ok(runtime)
        })
    }

    fn create() -> Result<Rc<Self>> {
        let entry =
            unsafe { Entry::load() }.map_err(|error| loader_err("load Vulkan loader", error))?;
        let mut instance_extensions = surface_instance_extensions();
        let (api_version, fault_query_mode) = configure_instance(&entry, &mut instance_extensions);
        let app_info = vk::ApplicationInfo::default()
            .application_name(c"uix")
            .application_version(1)
            .engine_name(c"uix")
            .engine_version(1)
            .api_version(api_version);
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
        let fault_feature_query = DeviceFaultFeatureQuery::new(fault_query_mode, &entry, &instance);
        Ok(Rc::new(Self {
            entry,
            instance,
            surface_loader,
            fault_feature_query,
            devices: RefCell::new(HashMap::new()),
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

    pub(super) fn acquire_device(
        self: &Rc<Self>,
        selection: QueueSelection,
    ) -> Result<Rc<VulkanDevice>> {
        let key = (selection.physical_device, selection.family_index);
        if let Some(device) = self.devices.borrow().get(&key).and_then(Weak::upgrade) {
            if !device.is_lost() {
                return Ok(device);
            }
        }

        let device = VulkanDevice::new(Rc::clone(self), selection)?;
        let mut devices = self.devices.borrow_mut();
        devices.retain(|_, device| device.strong_count() > 0);
        devices.insert(key, Rc::downgrade(&device));
        Ok(device)
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
    fault_reporter: Option<DeviceFaultReporter>,
    loss: DeviceLossState,
}

impl VulkanDevice {
    pub(super) fn new(runtime: Rc<VulkanRuntime>, selection: QueueSelection) -> Result<Rc<Self>> {
        let queue_priority = [1.0_f32];
        let queue_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(selection.family_index)
            .queue_priorities(&queue_priority);
        let fault_support = runtime.fault_feature_query.query(
            runtime.instance(),
            selection.physical_device,
            selection.extensions.supports_device_fault(),
        );
        let extensions = selection
            .extensions
            .enabled_names(fault_support.reporting());
        let mut fault_features = fault_support.requested_features();
        let base_device_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_info))
            .enabled_extension_names(&extensions);
        let device_info = if fault_support.reporting() {
            base_device_info.push_next(&mut fault_features)
        } else {
            base_device_info
        };
        // SAFETY: selection 来自同一 runtime instance，扩展名称在调用期间有效。
        let device = unsafe {
            runtime
                .instance()
                .create_device(selection.physical_device, &device_info, None)
        }
        .map_err(|error| vk_err("vkCreateDevice", error))?;
        // SAFETY: 创建 device 时声明了该 queue family 的一个 queue。
        let queue = unsafe { device.get_device_queue(selection.family_index, 0) };
        let fault_reporter = fault_support
            .reporting()
            .then(|| DeviceFaultReporter::new(runtime.instance(), &device));
        Ok(Rc::new(Self {
            _runtime: runtime,
            physical_device: selection.physical_device,
            info: selection.info,
            device,
            queue,
            fault_reporter,
            loss: DeviceLossState::default(),
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

    pub(super) fn fault_reporting_enabled(&self) -> bool {
        self.fault_reporter.is_some()
    }

    pub(super) fn ensure_healthy(&self) -> Result<()> {
        if let Some(error) = self.loss.peer_error() {
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn observe<T>(&self, result: Result<T>) -> Result<T> {
        result.map_err(|error| self.observe_error(error))
    }

    pub(super) fn observe_wait(&self, result: std::result::Result<(), vk::Result>) {
        if result == Err(vk::Result::ERROR_DEVICE_LOST) {
            let error = self.observe_error(vk_err(
                "vkDeviceWaitIdle during shutdown",
                vk::Result::ERROR_DEVICE_LOST,
            ));
            crate::core::log::error_fn(error.what());
        }
    }

    pub(super) fn error(&self, operation: &str, error: vk::Result) -> Error {
        self.observe_error(vk_err(operation, error))
    }

    #[cfg(test)]
    pub(super) fn mark_lost(&self) {
        self.loss.mark_for_test();
    }

    fn observe_error(&self, error: Error) -> Error {
        self.loss.record(error, self.fault_reporter.as_ref())
    }

    fn is_lost(&self) -> bool {
        self.loss.is_lost()
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

pub(crate) fn staging_size(width: i32, height: i32) -> vk::DeviceSize {
    (width.max(1) as vk::DeviceSize)
        .saturating_mul(height.max(1) as vk::DeviceSize)
        .saturating_mul(4)
}

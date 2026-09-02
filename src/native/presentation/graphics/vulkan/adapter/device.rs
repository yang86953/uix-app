//! Vulkan instance 与逻辑 device 的显式所有权边界。

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

use ash::{Entry, vk};

use crate::core::{Error, Result};

#[cfg(feature = "vulkan-parity-test")]
use super::adapter::select_headless_test_queue;
use super::adapter::{QueueSelection, VulkanAdapterInfo};
use super::context::{loader_err, vk_err};
use super::fault::{
    DeviceFaultFeatureQuery, DeviceFaultReporter, DeviceLossState, configure_instance,
};
use super::surface::surface_instance_extensions;

type DeviceKey = (vk::PhysicalDevice, u32);

thread_local! {
    /// Vulkan 调用受 `ThreadBoundGraphicsContext` 约束；线程本地弱引用既复用父资源，
    /// 又不延长最后一个窗口 context 的生命周期。
    #[allow(
        clippy::missing_const_for_thread_local,
        reason = "the initializer is already const and the lint fires through thread_local"
    )]
    static THREAD_RUNTIME: RefCell<Weak<VulkanRuntime>> = const { RefCell::new(Weak::new()) };
}

/// 与窗口 surface 无关的 Vulkan instance 资源。
pub(super) struct VulkanRuntime {
    entry: Entry,
    instance: ash::Instance,
    surface_loader: ash::khr::surface::Instance,
    // 记录 instance 创建时是否实际启用了 swapchain maintenance1 的必需依赖。
    surface_maintenance1: bool,
    fault_feature_query: DeviceFaultFeatureQuery,
    devices: RefCell<HashMap<DeviceKey, Weak<VulkanDevice>>>,
    // 显式禁止 runtime 跨线程迁移；所有 Vulkan owner 与调用都留在创建线程。
    _thread_bound: PhantomData<Rc<()>>,
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
        // SAFETY: ash 负责按平台加载 Vulkan 动态库及校验入口符号，返回的 Entry 持有符号所需的库生命周期。
        let entry =
            unsafe { Entry::load() }.map_err(|error| loader_err("load Vulkan loader", error))?;
        let (mut instance_extensions, surface_maintenance1) = surface_instance_extensions(&entry);
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
            surface_maintenance1,
            fault_feature_query,
            devices: RefCell::new(HashMap::new()),
            _thread_bound: PhantomData,
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

// 同一逻辑 device 的 queue 是 Vulkan 外部同步对象；租约禁止重入与交叉提交。
#[derive(Default)]
struct QueueSerial {
    active: Cell<bool>,
}

impl QueueSerial {
    fn enter(&self, operation: &str) -> Result<QueueSerialLease<'_>> {
        if self.active.replace(true) {
            return Err(Error::new(
                crate::core::Errc::InvalidState,
                format!("VulkanDevice: shared queue re-entry during {operation}"),
            ));
        }
        Ok(QueueSerialLease { serial: self })
    }
}

struct QueueSerialLease<'a> {
    serial: &'a QueueSerial,
}

impl Drop for QueueSerialLease<'_> {
    fn drop(&mut self) {
        self.serial.active.set(false);
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
    queue_serial: QueueSerial,
    fault_reporter: Option<DeviceFaultReporter>,
    swapchain_maintenance1: bool,
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
        let swapchain_maintenance1 = runtime.surface_maintenance1
            && runtime.fault_feature_query.query_swapchain_maintenance1(
                runtime.instance(),
                selection.physical_device,
                selection.extensions.supports_swapchain_maintenance1(),
            );
        let extensions = selection
            .extensions
            .enabled_names(fault_support.reporting(), swapchain_maintenance1);
        let mut fault_features = fault_support.requested_features();
        let mut maintenance_features =
            vk::PhysicalDeviceSwapchainMaintenance1FeaturesEXT::default()
                .swapchain_maintenance1(swapchain_maintenance1);
        let mut device_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_info))
            .enabled_extension_names(&extensions);
        if fault_support.reporting() {
            device_info = device_info.push_next(&mut fault_features);
        }
        if swapchain_maintenance1 {
            device_info = device_info.push_next(&mut maintenance_features);
        }
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
            queue_serial: QueueSerial::default(),
            fault_reporter,
            swapchain_maintenance1,
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

    pub(super) fn fault_reporting_enabled(&self) -> bool {
        self.fault_reporter.is_some()
    }

    pub(super) fn swapchain_maintenance1_enabled(&self) -> bool {
        self.swapchain_maintenance1
    }

    pub(super) fn ensure_healthy(&self) -> Result<()> {
        if let Some(error) = self.loss.peer_error() {
            return Err(error);
        }
        Ok(())
    }

    /// 在当前 device 的唯一 queue 租约内执行一次原生 queue 操作。
    pub(super) fn with_queue<T>(
        &self,
        operation: &str,
        use_queue: impl FnOnce(vk::Queue) -> T,
    ) -> Result<T> {
        self.ensure_healthy()?;
        let _serial = self.queue_serial.enter(operation)?;
        Ok(use_queue(self.queue))
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
            // shutdown 期 device-lost 观察经边界观察入口记录。
            crate::diagnostics::observe_boundary_error("vulkan/device", &error);
        }
    }

    pub(super) fn error(&self, operation: &str, error: vk::Result) -> Error {
        self.observe_error(vk_err(operation, error))
    }

    #[cfg(any(test, feature = "vulkan-parity-test", all(windows, feature = "vulkan")))]
    pub(super) fn mark_lost(&self) {
        self.loss.mark_for_test(self.fault_reporter.is_some());
    }

    fn observe_error(&self, error: Error) -> Error {
        self.loss.record(error, self.fault_reporter.as_ref())
    }

    pub(super) fn is_lost(&self) -> bool {
        self.loss.is_lost()
    }

    #[cfg(feature = "vulkan-parity-test")]
    fn identity(&self) -> usize {
        self as *const Self as usize
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

// 在真实 Vulkan device 上验证共享、peer loss、重取、旧代释放与 queue 身份合同。
#[cfg(feature = "vulkan-parity-test")]
pub(super) fn run_shared_device_contract_test() {
    let runtime = VulkanRuntime::acquire().expect("shared-device runtime must be created");
    let selection = select_headless_test_queue(runtime.instance())
        .expect("a graphics queue with production Vulkan extensions must exist");
    let adapter = selection.info.diagnostic_summary();

    let first = runtime
        .acquire_device(selection.clone())
        .expect("first shared device must be created");
    let peer = runtime
        .acquire_device(selection.clone())
        .expect("healthy shared device must be reused");
    assert!(Rc::ptr_eq(&first, &peer));

    let first_identity = first.identity();
    first
        .with_queue("shared-device identity test", |_queue| {
            let reentry = peer
                .with_queue("peer concurrent submit test", |_queue| ())
                .expect_err("one shared queue must reject re-entry");
            assert_eq!(reentry.code(), crate::core::Errc::InvalidState);
        })
        .expect("outer shared queue lease must be valid");
    let submit_status = first
        .with_queue("shared-device empty submit", |queue| unsafe {
            first
                .device()
                .queue_submit(queue, &[], vk::Fence::null())
                .and_then(|()| first.device().queue_wait_idle(queue))
        })
        .expect("shared queue lease must serialize native submit");
    first
        .observe(submit_status.map_err(|error| vk_err("shared-device empty submit", error)))
        .expect("serialized native submit must complete");

    first.mark_lost();
    for owner in [&first, &peer] {
        let error = owner
            .ensure_healthy()
            .expect_err("every old-device peer must observe the same typed loss");
        assert_eq!(error.code(), crate::core::Errc::GraphicsDeviceLost);
        let queue_error = owner
            .with_queue("submit after peer loss", |_queue| ())
            .expect_err("lost peer must fail before touching the native queue");
        assert_eq!(queue_error.code(), crate::core::Errc::GraphicsDeviceLost);
    }

    let old = Rc::downgrade(&first);
    let replacement = runtime
        .acquire_device(selection.clone())
        .expect("lost device must be replaced");
    assert_ne!(first_identity, replacement.identity());
    assert!(old.upgrade().is_some());
    let replacement_peer = runtime
        .acquire_device(selection)
        .expect("healthy replacement must be shared");
    assert!(Rc::ptr_eq(&replacement, &replacement_peer));

    drop(first);
    assert!(old.upgrade().is_some());
    drop(peer);
    assert!(old.upgrade().is_none());
    replacement
        .with_queue("replacement identity test", |_queue| ())
        .expect("replacement queue must have an independent serial identity");

    eprintln!(
        "Vulkan shared device verified: {adapter}; old={first_identity:#x}; new={:#x}",
        replacement.identity()
    );
}

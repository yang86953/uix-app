// 各项自带原 cfg 门控（uix_gpu_parity_* 或含 test/生产分支的 any 组合），
// 在源文件模块作用域 include! 展开。
// 四个观测/注入入口当前无调用者（原 any 分支中的 windows+vulkan 组合无生产调用链，
// 见本轮 result 调用链证据），实体保留待重新接线。

#[allow(dead_code)]
impl super::VulkanContext {
    #[cfg(any(test, uix_gpu_parity_vulkan, all(windows, feature = "vulkan")))]
    pub(crate) fn shared_device_identity(&self) -> usize {
        self.device_lease
            .as_ref()
            .map_or(0, |device| Rc::as_ptr(device) as usize)
    }

    #[cfg(any(test, uix_gpu_parity_vulkan, all(windows, feature = "vulkan")))]
    pub(crate) fn device_fault_reporting_enabled_for_test(&self) -> bool {
        self.device_lease
            .as_ref()
            .is_some_and(|device| device.fault_reporting_enabled())
    }

    #[cfg(any(test, uix_gpu_parity_vulkan, all(windows, feature = "vulkan")))]
    pub(crate) fn swapchain_maintenance1_enabled_for_test(&self) -> bool {
        self.device_lease
            .as_ref()
            .is_some_and(|device| device.swapchain_maintenance1_enabled())
    }

    #[cfg(any(test, uix_gpu_parity_vulkan, all(windows, feature = "vulkan")))]
    pub(crate) fn mark_shared_device_lost_for_test(&self) {
        if let Some(device) = self.device_lease.as_ref() {
            device.mark_lost();
        }
    }
}

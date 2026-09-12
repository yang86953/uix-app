// 各项自带原 cfg 门控（uix_gpu_parity_* 或含 test/生产分支的 any 组合），
// 在源文件模块作用域 include! 展开。

impl DeviceLossState {
    #[cfg(any(test, uix_gpu_parity_vulkan, all(windows, feature = "vulkan")))]
    pub(super) fn mark_for_test(&self, device_fault_supported: bool) {
        let diagnostic = if device_fault_supported {
            "ERROR_DEVICE_LOST; VK_EXT_device_fault: synthetic external reset"
        } else {
            "ERROR_DEVICE_LOST"
        };
        let error = Error::new(
            Errc::GraphicsDeviceLost,
            format!("VulkanContext: shared logical device marked lost by test ({diagnostic})"),
        );
        let _ = self.record_with(error, |error| error);
    }
}

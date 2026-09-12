// 各项自带原 cfg 门控（uix_gpu_parity_* 或含 test/生产分支的 any 组合），
// 在源文件模块作用域 include! 展开。

impl VulkanContext {
    // 只允许显式 parity 组合根在无在途 frame 时安排一个原生 Surface 结果。
    #[cfg(uix_gpu_parity_vulkan)]
    pub(crate) fn inject_surface_fault_for_parity_test(
        &mut self,
        fault: super::VulkanSurfaceFaultForParity,
    ) -> Result<()> {
        self.active_device()?;
        if self.acquired_frame.is_some() || self.submitted_frame.is_some() {
            return Err(invalid_state(
                "Vulkan Surface parity fault requires an idle frame boundary",
            ));
        }
        if self.surface_fault_for_parity.is_some() {
            return Err(invalid_state(
                "Vulkan Surface parity fault is already pending",
            ));
        }
        self.surface_fault_for_parity = Some(fault);
        Ok(())
    }

    // 只在对应原生调用边界消费一次匹配的 parity 故障。
    #[cfg(uix_gpu_parity_vulkan)]
    fn take_surface_fault_for_parity_test(
        &mut self,
        fault: super::VulkanSurfaceFaultForParity,
    ) -> bool {
        if self.surface_fault_for_parity == Some(fault) {
            self.surface_fault_for_parity = None;
            true
        } else {
            false
        }
    }
}

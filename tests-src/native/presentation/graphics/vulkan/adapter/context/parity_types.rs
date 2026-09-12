// 各项自带原 cfg 门控（uix_gpu_parity_* 或含 test/生产分支的 any 组合），
// 在源文件模块作用域 include! 展开。

// 显式 parity feature 可安排的一次性 Vulkan Surface 原生结果。
#[cfg(uix_gpu_parity_vulkan)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VulkanSurfaceFaultForParity {
    // 在调用 vkAcquireNextImageKHR 前模拟旧 swapchain 已失效。
    AcquireOutOfDate,
    // 保留真实 acquire image，只把其返回状态提升为 SUBOPTIMAL。
    AcquireSuboptimal,
    // 在调用 vkQueuePresentKHR 前模拟当前 swapchain 已失效。
    PresentOutOfDate,
    // 保留真实 queue present，只把其返回状态提升为 SUBOPTIMAL。
    PresentSuboptimal,
}

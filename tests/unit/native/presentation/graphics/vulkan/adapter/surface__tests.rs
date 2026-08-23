use super::*;

// 可用时优先 MAILBOX，避免交互帧在 FIFO 队列中增加一帧等待。
#[test]
fn present_mode_prefers_mailbox_over_fifo() {
    let modes = [vk::PresentModeKHR::FIFO, vk::PresentModeKHR::MAILBOX];
    assert!(choose_present_mode(&modes) == vk::PresentModeKHR::MAILBOX);
}

// 不支持 MAILBOX 的实现必须继续选择 Vulkan 保证存在的 FIFO。
#[test]
fn present_mode_falls_back_to_fifo() {
    let modes = [vk::PresentModeKHR::IMMEDIATE, vk::PresentModeKHR::FIFO];
    assert!(choose_present_mode(&modes) == vk::PresentModeKHR::FIFO);
}

// 防御性空集合仍返回规范 FIFO，不把无值传播进 swapchain 创建。
#[test]
fn empty_present_modes_use_fifo_default() {
    assert!(choose_present_mode(&[]) == vk::PresentModeKHR::FIFO);
}

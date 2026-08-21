//! 显式 feature 驱动的真实 Vulkan 离屏像素一致性测试。

#[test]
fn solid_mesh_matches_cpu_unorm_reference_on_real_vulkan_device() {
    uix::__run_vulkan_gpu_parity_test();
}

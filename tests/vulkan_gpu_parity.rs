//! 显式 feature 驱动的真实 Vulkan 离屏像素一致性测试。

#[test]
fn every_pipeline_matches_shared_consistency_scene_on_real_vulkan_device() {
    uix::__run_vulkan_gpu_parity_test();
}

// 不依赖原生窗口，确定性验证 OUT_OF_DATE 与 SUBOPTIMAL 的帧收尾语义。
#[test]
fn surface_lifecycle_distinguishes_retry_from_presented_rebuild() {
    uix::__run_surface_lifecycle_contract_test();
}

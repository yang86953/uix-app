//! 显式 feature 驱动的真实 Vulkan 离屏像素一致性测试。

#[test]
fn every_pipeline_matches_shared_consistency_scene_on_real_vulkan_device() {
    uix::__run_vulkan_gpu_parity_test();
}

// 从真实 UI WidgetRender 入口闭合 Drawing FramePlan 到生产 Vulkan Device adapter。
#[test]
fn ui_drawing_frame_plan_executes_and_reads_back_on_real_vulkan_device() {
    uix::__run_vulkan_ui_production_chain_test();
}

// 不依赖原生窗口，确定性验证 OUT_OF_DATE 与 SUBOPTIMAL 的帧收尾语义。
#[test]
fn surface_lifecycle_distinguishes_retry_from_presented_rebuild() {
    uix::__run_surface_lifecycle_contract_test();
}

// 在真实逻辑 device 与确定性窗口夹具上闭合共享丢失恢复合同。
#[test]
fn shared_device_loss_reacquires_once_per_window_without_crossing_identity() {
    uix::__run_vulkan_shared_device_contract_test();
}

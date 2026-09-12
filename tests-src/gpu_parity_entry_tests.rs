//! GPU parity 入口的 lib 内可执行用例：把 `__run_*` 生产链/WSI/生命周期合同
//! 接入 cargo test（复刻 2b464b77e 删除前 tests/{vulkan,opengl,d3d11}_gpu_parity.rs
//! 的用例名与调用语义），经 #[path] 挂载于 lib，仅在各后端专用 cfg 键开启时编译。
//! 不进发布包；失败按测试失败真实传播，不用默认后端 readback 冒充其他后端。

/// 从真实 UI WidgetRender 入口验收 Drawing FramePlan 到生产 Vulkan Device adapter。
#[cfg(all(test, uix_gpu_parity_vulkan))]
#[test]
fn ui_drawing_frame_plan_executes_and_reads_back_on_real_vulkan_device() {
    crate::__run_vulkan_ui_production_chain_test();
}

/// 在当前真实窗口会话上闭合 Vulkan WSI acquire/render/present、resize 与销毁事务。
#[cfg(all(test, uix_gpu_parity_vulkan))]
#[test]
fn ui_drawing_frame_plan_presents_through_real_vulkan_wsi() {
    crate::__run_vulkan_wsi_production_chain_test();
}

/// 不依赖原生窗口，确定性验证 OUT_OF_DATE 与 SUBOPTIMAL 的帧收尾语义。
#[cfg(all(test, uix_gpu_parity_vulkan))]
#[test]
fn surface_lifecycle_distinguishes_retry_from_presented_rebuild() {
    crate::__run_surface_lifecycle_contract_test();
}

/// 在真实逻辑 device 与确定性窗口夹具上闭合共享丢失恢复合同。
#[cfg(all(test, uix_gpu_parity_vulkan))]
#[test]
fn shared_device_loss_reacquires_once_per_window_without_crossing_identity() {
    crate::__run_vulkan_shared_device_contract_test();
}

/// 从真实 Wayland 窗口验收 EGL Surface、共享生命周期与最终 present。
#[cfg(all(test, target_os = "linux", uix_gpu_parity_opengl))]
#[test]
fn ui_drawing_frame_plan_presents_through_real_opengl_wsi() {
    crate::__run_opengl_wsi_production_chain_test();
}

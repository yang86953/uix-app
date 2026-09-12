//! 显式 GPU 配置才执行；生产依赖本身不以 cfg(test) 构建。
#[cfg(uix_gpu_parity_vulkan)]
#[test]
fn vulkan_all_primitives() {
    uix_app::__run_vulkan_gpu_parity_test();
}
#[cfg(uix_gpu_parity_vulkan)]
#[test]
fn vulkan_ui_production_chain() {
    uix_app::__run_vulkan_ui_production_chain_test();
}
#[cfg(all(target_os = "linux", uix_gpu_parity_opengl))]
#[test]
fn opengl_all_primitives() {
    uix_app::__run_opengl_gpu_parity_test();
}

#[cfg(all(windows, uix_gpu_parity_d3d11))]
#[test]
fn d3d11_all_primitives() {
    uix_app::__run_d3d11_gpu_parity_test();
}

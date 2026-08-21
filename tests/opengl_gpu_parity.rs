//! 显式 feature 驱动的 Linux 真实 EGL/OpenGL ES 离屏像素一致性测试。

#![cfg(target_os = "linux")]

#[test]
fn every_pipeline_matches_shared_consistency_scene_on_real_opengl_es_device() {
    uix::__run_opengl_gpu_parity_test();
}

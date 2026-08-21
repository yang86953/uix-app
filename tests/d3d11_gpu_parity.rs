//! 显式 feature 驱动的 Windows 真实 D3D11 离屏像素一致性测试。
//! Windows 单命令：`cargo test --no-default-features --features d3d11-parity-test --test d3d11_gpu_parity -- --nocapture`。

#![cfg(windows)]

#[test]
fn every_pipeline_matches_shared_consistency_scene_on_real_d3d11_device() {
    uix::__run_d3d11_gpu_parity_test();
}

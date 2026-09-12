// 各项自带原 cfg 门控（uix_gpu_parity_* 或含 test/生产分支的 any 组合），
// 在源文件模块作用域 include! 展开。

// 让显式 Vulkan 验证 feature 在不创建窗口或原生对象时执行共享状态机。
#[cfg(uix_gpu_parity_vulkan)]
pub(crate) fn run_surface_lifecycle_contract_test() {
    let initial_extent = RhiExtent::new(640, 480);
    let mut lifecycle = RhiSurfaceLifecycle::uninitialized(initial_extent);
    let initial = lifecycle
        .begin_recreate(initial_extent, RhiSurfaceRecreateReason::Initialize)
        .unwrap_or_else(|error| panic!("surface initialize transaction failed: {error}"));
    let ready = lifecycle
        .commit_recreate(initial, initial_extent)
        .unwrap_or_else(|error| panic!("surface initialize commit failed: {error}"));
    assert!(matches!(ready, RhiSurfaceRecreateCommit::Ready(_)));

    let out_of_date = lifecycle
        .begin_recreate(
            initial_extent,
            RhiSurfaceRecreateReason::AcquisitionRejected,
        )
        .unwrap_or_else(|error| panic!("OUT_OF_DATE transaction failed: {error}"));
    let retry = lifecycle
        .commit_recreate(out_of_date, initial_extent)
        .unwrap_or_else(|error| panic!("OUT_OF_DATE commit failed: {error}"));
    let retry_error = retry
        .complete_frame(Error::new(
            Errc::GraphicsSurfaceLost,
            "deterministic OUT_OF_DATE status",
        ))
        .expect_err("OUT_OF_DATE must preserve dirty state and retry");
    assert_eq!(retry_error.code(), Errc::GraphicsSurfaceChanged);

    let suboptimal = lifecycle
        .begin_recreate(
            initial_extent,
            RhiSurfaceRecreateReason::PresentedNeedsRecreate,
        )
        .unwrap_or_else(|error| panic!("SUBOPTIMAL transaction failed: {error}"));
    let presented = lifecycle
        .commit_recreate(suboptimal, initial_extent)
        .unwrap_or_else(|error| panic!("SUBOPTIMAL commit failed: {error}"));
    presented
        .complete_frame(Error::new(
            Errc::GraphicsSurfaceLost,
            "deterministic SUBOPTIMAL status",
        ))
        .expect("SUBOPTIMAL must keep the already-presented frame successful");
    assert_eq!(lifecycle.token().generation, 2);
}

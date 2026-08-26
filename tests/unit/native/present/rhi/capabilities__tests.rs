// 引入默认与显式 Surface 一致性事实。
use crate::core::PresentCoherency;
// 导入两个正交能力快照。
use super::{GraphicsDeviceCapabilities, GraphicsSurfaceCapabilities};

// 验证 Device profile 只由可执行的底层原语组成。
#[test]
fn device_profile_exposes_a_complete_primitive_baseline() {
    // 构造不依赖 Renderer 派生策略的 Device 基线。
    let baseline = GraphicsDeviceCapabilities::full_gpu_baseline();
    // 完整 profile 必须满足全部底层 GPU Device 原语。
    assert!(baseline.has_gpu_baseline());
    // 完整 profile 不应制造新的 Device 基线缺口。
    assert_eq!(baseline.first_missing_gpu_baseline(), None);
}

// 验证 Surface 默认能力不会伪造可选操作。
#[test]
fn surface_defaults_do_not_claim_optional_operations() {
    // 构造测试与未声明 Adapter 使用的空 Surface profile。
    let capabilities = GraphicsSurfaceCapabilities::default();
    // 未声明的 Surface 必须保守退回完整呈现。
    assert_eq!(capabilities.present_coherency, PresentCoherency::FullOnly);
    // 默认 Surface 不得宣称可回读。
    assert!(!capabilities.readback);
}

// 验证显式 Surface profile 同时保存呈现和回读事实。
#[test]
// 锁定 Surface capability 是 present coherency 的薄 RHI 权威来源。
fn surface_profile_owns_present_coherency() {
    // 模拟具备 per-image 历史的 D3D11 swapchain Surface。
    let capabilities = GraphicsSurfaceCapabilities::with_readback(
        // 使用 tracked 事实证明构造器不会硬编码 FullOnly。
        PresentCoherency::TrackedSwapchain,
    );
    // Surface profile 必须精确保留 Adapter 给出的呈现一致性。
    assert_eq!(
        capabilities.present_coherency,
        // 期望仍为 tracked swapchain 语义。
        PresentCoherency::TrackedSwapchain
    );
    // 同一 profile 必须继续声明同步回读实现。
    assert!(capabilities.readback);
}

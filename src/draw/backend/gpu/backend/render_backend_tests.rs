// 引入 typed failure 分类用于断言 lowering 边界。
use crate::core::Errc;
// 引入待验证的 RHI-only 能力与 extent 推导函数。
use super::{
    // 验证未覆盖 lowering 返回稳定 typed failure。
    require_lossless_rhi_submission,
    // 验证高层能力只由 RHI owner 推导。
    retained_gpu_capabilities,
    // 验证 Picture extent 的单一 owner 门禁。
    rhi_offscreen_extent,
};

// 验证局部重绘只由 retained 主目标与 tracked swapchain 的合取事实开启。
#[test]
// 锁定 tracked 与 legacy 两种能力投影。
fn retained_capabilities_gate_partial_redraw_from_present_proof() {
    // 构造同时具备 Picture owner 和完整窄提交证明的生产能力。
    let tracked = retained_gpu_capabilities(true, true);
    // tracked swapchain 可以向场景层开放局部重绘。
    assert!(tracked.partial_redraw);
    // 独立离屏能力仍应透传给 Picture 与效果管线。
    assert!(tracked.offscreen);

    // 构造 retained renderer 存在但 swapchain 只能 FullOnly 的回退能力。
    let legacy = retained_gpu_capabilities(true, false);
    // legacy DISCARD 不得开放局部重绘。
    assert!(!legacy.partial_redraw);
    // present 回退不应关闭独立的 Picture 能力。
    assert!(legacy.offscreen);

    // 构造没有通用 RHI owner 的防御性组合。
    let without_rhi = retained_gpu_capabilities(false, false);
    // 缺少 retained renderer 时不得开放 partial。
    assert!(!without_rhi.partial_redraw);
    // 没有通用 RHI owner 时不得虚构 Picture 支持。
    assert!(!without_rhi.offscreen);
}

// 验证 Picture 资源只能由通用 RHI owner 创建。
#[test]
// 覆盖缺少 owner、无效尺寸和有效单一 owner 三类边界。
fn rhi_offscreen_extent_enforces_single_owner_gate() {
    // 没有 RHI owner 时即使尺寸有效也必须拒绝创建。
    assert!(rhi_offscreen_extent(false, 64, 32).is_none());
    // 非正宽度不能进入纹理创建边界。
    assert!(rhi_offscreen_extent(true, 0, 32).is_none());
    // 非正高度不能进入纹理创建边界。
    assert!(rhi_offscreen_extent(true, 64, -1).is_none());
    // 有效 owner 与尺寸应产生唯一的 RHI extent。
    let extent = rhi_offscreen_extent(true, 64, 32).expect("有效 RHI owner 应产生 extent");
    // 验证宽度按原始 Picture 逻辑尺寸创建。
    assert_eq!(extent.width, 64);
    // 验证高度按原始 Picture 逻辑尺寸创建。
    assert_eq!(extent.height, 32);
}

// 验证未完整 lowering 会在 adapter 高层回退之前变成 typed failure。
#[test]
// 同时覆盖成功透传与失败分类，锁定兼容分叉不得复活。
fn submissions_require_lossless_rhi_lowering() {
    // 完整提交应允许调用方继续消费 staging。
    assert!(require_lossless_rhi_submission(true, "unused").is_ok());
    // 模拟通用 RHI 无法覆盖当前 Picture queue。
    let failure = require_lossless_rhi_submission(false, "missing RHI lowering");
    // 失败必须保持 NotImplemented 分类供恢复层识别。
    assert!(matches!(
        // 检查 helper 返回的 typed error。
        failure,
        // 禁止把 lowering 缺口伪装成成功或参数错误。
        Err(error) if error.code() == Errc::NotImplemented
    ));
}

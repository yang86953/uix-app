// 引入 typed failure 分类用于断言 lowering 边界。
use crate::core::Errc;
// 引入待验证的 RHI-only 能力与 extent 推导函数。
use super::{
    // 验证高层能力只由 RHI owner 推导。
    migration_safe_gpu_capabilities,
    // 验证未覆盖 lowering 返回稳定 typed failure。
    require_lossless_rhi_submission,
    // 验证 Picture extent 的单一 owner 门禁。
    rhi_offscreen_extent,
};

// 验证离屏支持不会重新开启不安全的主 surface 局部重绘。
#[test]
// 锁定迁移期的完整重绘能力边界。
fn migration_capabilities_keep_partial_redraw_disabled() {
    // 构造持有通用 RHI Picture owner 的生产能力组合。
    let with_offscreen = migration_safe_gpu_capabilities(true);
    // 主 surface 必须保持完整重绘，避免兼容回退只留下 damage 区域。
    assert!(!with_offscreen.partial_redraw);
    // 独立离屏能力仍应透传给 Picture 与效果管线。
    assert!(with_offscreen.offscreen);

    // 构造没有通用 RHI Picture owner 的生产能力组合。
    let without_offscreen = migration_safe_gpu_capabilities(false);
    // 无离屏能力时同样不得依赖 swapchain 内容保留。
    assert!(!without_offscreen.partial_redraw);
    // 没有通用 RHI owner 时不得从 adapter 能力虚构 Picture 支持。
    assert!(!without_offscreen.offscreen);
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

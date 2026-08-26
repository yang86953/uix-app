// 引入被测纯数据函数与常量。
use super::{
    DXGI_SWAP_EFFECT_DISCARD, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, legacy_swap_chain_contract,
    legacy_swap_chain_desc, map_dxgi_device_removed_reason, map_dxgi_present_result,
    map_dxgi_present_test_result, native_dirty_rects, tracked_swap_chain_contract,
    tracked_swap_chain_desc,
};
// 引入共享 damage、错误码与 coherency 类型。
use crate::core::{Errc, PresentCoherency, PresentDamage};
// 引入 Windows HRESULT 及设备移除状态。
use ::windows::core::HRESULT;
// 引入 DXGI 遮挡状态。
use ::windows::Win32::Foundation::DXGI_STATUS_OCCLUDED;
// 引入 DXGI 设备移除与重置码。
use ::windows::Win32::Graphics::Dxgi::{DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET};

// 验证 S_OK 被视为健康设备状态。
#[test]
fn device_removed_reason_accepts_success() {
    // 传入成功 HRESULT，不应触发恢复错误。
    assert!(map_dxgi_device_removed_reason(HRESULT(0)).is_ok());
}

// 验证已知设备移除状态保持 typed device-lost 语义。
#[test]
fn device_removed_reason_maps_device_loss() {
    // 把 DXGI 设备移除码交给统一分类边界。
    let error = match map_dxgi_device_removed_reason(DXGI_ERROR_DEVICE_REMOVED) {
        // 已知移除码必须产生错误。
        Err(error) => error,
        // 设备移除被忽略会绕过恢复路径。
        Ok(()) => panic!("device removal HRESULT must be reported"),
    };
    // 验证恢复层可以按 GraphicsDeviceLost 选择重建 device。
    assert_eq!(error.code(), Errc::GraphicsDeviceLost);
}

// 验证正常 present 的遮挡状态保持可恢复 typed error。
#[test]
fn present_maps_occlusion_without_committing_frame() {
    // 将 DXGI 的成功状态形态遮挡码交给最终 present 分类。
    let error = map_dxgi_present_result(DXGI_STATUS_OCCLUDED)
        // 遮挡不能被当作成功帧。
        .expect_err("occluded present must not report a committed frame");
    // 上层恢复与 idle 调度必须收到 GraphicsOccluded。
    assert_eq!(error.code(), Errc::GraphicsOccluded);
}

// 验证无帧探测把遮挡编码为状态而不是失败。
#[test]
fn present_test_reports_occluded_state() {
    // 执行与 DXGI_PRESENT_TEST 相同的 HRESULT 分类。
    let result = map_dxgi_present_test_result(DXGI_STATUS_OCCLUDED)
        // 遮挡探测本身应成功返回状态。
        .expect("present test occlusion is a recoverable probe result");
    // 调度器必须继续保持 Occluded idle。
    assert_eq!(
        result,
        crate::platform::presentation::PresentTestResult::Occluded
    );
}

// 验证最终 present 的设备重置进入统一 device-lost 恢复路径。
#[test]
fn present_maps_device_reset_to_device_loss() {
    // 将交换链 present 可能返回的 reset HRESULT 交给分类边界。
    let error = map_dxgi_present_result(DXGI_ERROR_DEVICE_RESET)
        // 设备重置不得伪装为 present 成功。
        .expect_err("device reset must fail the current present");
    // 恢复层必须按 GraphicsDeviceLost 重建设备与 surface。
    assert_eq!(error.code(), Errc::GraphicsDeviceLost);
}

// 验证 flip descriptor 与 tracked coherency 来自同一事实。
#[test]
fn flip_sequential_descriptor_exposes_tracked_present_coherency() {
    // 读取主路径能力事实。
    let contract = tracked_swap_chain_contract();
    // 构造纯数据 descriptor；测试不调用 DXGI。
    let descriptor = tracked_swap_chain_desc(640, 480);
    // descriptor 必须使用契约声明的双缓冲数量。
    assert_eq!(descriptor.BufferCount, contract.buffer_count);
    // 交换效果必须保留每个 back buffer 内容。
    assert_eq!(descriptor.SwapEffect, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL);
    // 真实 image index 允许上层跟踪 per-image 历史。
    assert_eq!(
        contract.present_coherency,
        PresentCoherency::TrackedSwapchain
    );
}

// 验证 legacy descriptor 保持完整提交一致性。
#[test]
fn discard_fallback_keeps_full_only_present_contract() {
    // 读取回退能力事实。
    let contract = legacy_swap_chain_contract();
    // 使用空窗口句柄构造纯数据 descriptor；本测试不会调用 DXGI。
    let descriptor = legacy_swap_chain_desc(std::ptr::null_mut(), 640, 480);
    // descriptor 必须使用契约声明的双缓冲数量。
    assert_eq!(descriptor.BufferCount, contract.buffer_count);
    // descriptor 必须继续使用不保留内容的 DISCARD 模型。
    assert_eq!(descriptor.SwapEffect, DXGI_SWAP_EFFECT_DISCARD);
    // DISCARD 模型只能向 Graphics System 提供完整提交证明。
    assert_eq!(contract.present_coherency, PresentCoherency::FullOnly);
}

// 验证物理 damage 按 DXGI left/top/right/bottom 形态转换。
#[test]
fn dirty_rect_conversion_preserves_physical_region() {
    // 构造一个位于 surface 内部的物理矩形。
    let damage = PresentDamage::Partial(vec![(10, 20, 30, 40)]);
    // Adapter 只做一次原生 RECT 机械投影。
    let rects = native_dirty_rects(&damage);
    // 断言左上坐标不变。
    assert_eq!((rects[0].left, rects[0].top), (10, 20));
    // 断言宽高被转换为开区间右下坐标。
    assert_eq!((rects[0].right, rects[0].bottom), (40, 60));
}

// 验证共享门禁发布的 Full damage 被机械编码为全帧参数。
#[test]
fn full_damage_maps_to_zero_dirty_rects() {
    // 显式 Full damage 不携带任何 DXGI 私有矩形。
    let rects = native_dirty_rects(&PresentDamage::Full);
    // Present1 以零矩形表示完整提交。
    assert!(rects.is_empty());
}

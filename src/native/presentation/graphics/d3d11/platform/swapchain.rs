//! D3D11 bitblt swapchain 描述与 HRESULT 分类边界。

use std::ffi::c_void;

use crate::core::{Errc, Error, Result};
// 引入跨后端共享的 present 保留性证明类型，避免 D3D11 自造第二套语义。
use crate::native::present::PresentCoherency;
use crate::native::present::PresentTestResult;
use ::windows::Win32::Foundation::{DXGI_STATUS_OCCLUDED, E_OUTOFMEMORY, HWND, TRUE};
use ::windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_DESC, DXGI_MODE_SCALING_UNSPECIFIED,
    DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED, DXGI_RATIONAL, DXGI_SAMPLE_DESC,
};
use ::windows::Win32::Graphics::Dxgi::{
    DXGI_ERROR_DEVICE_HUNG, DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET,
    DXGI_ERROR_DRIVER_INTERNAL_ERROR, DXGI_ERROR_REMOTE_OUTOFMEMORY, DXGI_SWAP_CHAIN_DESC,
    DXGI_SWAP_EFFECT_DISCARD, DXGI_USAGE_RENDER_TARGET_OUTPUT,
};

// 集中保存当前 D3D11 swapchain 与上层 present 能力之间的事实契约。
pub(crate) struct D3d11SwapChainContract {
    // 保存实际创建的交换链缓冲数量。
    pub(crate) buffer_count: u32,
    // 保存 DXGI 交换效果，供 descriptor 与一致性测试共同消费。
    pub(crate) swap_effect: ::windows::Win32::Graphics::Dxgi::DXGI_SWAP_EFFECT,
    // 保存 GraphicsContext 对外声明的跨帧保留性证明。
    pub(crate) present_coherency: PresentCoherency,
    // 保存薄 RHI 是否允许向 compositor 提交窄损伤区域。
    pub(crate) partial_present: bool,
    // 结束 D3D11 swapchain 事实契约定义。
}

// 返回当前生产 D3D11 bitblt swapchain 的唯一能力事实。
pub(crate) const fn swap_chain_contract() -> D3d11SwapChainContract {
    // DISCARD 不证明 backbuffer 内容保留，因此必须维持完整提交回退。
    D3d11SwapChainContract {
        // 保留现有双缓冲创建参数。
        buffer_count: 2,
        // 保留已验证的 legacy bitblt DISCARD 交换效果。
        swap_effect: DXGI_SWAP_EFFECT_DISCARD,
        // DISCARD 无法为上层提供可证明的窄 present coherency。
        present_coherency: PresentCoherency::FullOnly,
        // 在 #899 完成交换链决策前禁止宣称 compositor 窄提交能力。
        partial_present: false,
        // 结束当前生产契约值。
    }
    // 结束 D3D11 swapchain 契约查询。
}

pub(crate) fn swap_chain_desc(hwnd: *mut c_void, width: i32, height: i32) -> DXGI_SWAP_CHAIN_DESC {
    // 从唯一事实契约读取交换链创建参数。
    let contract = swap_chain_contract();
    DXGI_SWAP_CHAIN_DESC {
        BufferDesc: DXGI_MODE_DESC {
            Width: width.max(1) as u32,
            Height: height.max(1) as u32,
            RefreshRate: DXGI_RATIONAL {
                Numerator: 60,
                Denominator: 1,
            },
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            ScanlineOrdering: DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
            Scaling: DXGI_MODE_SCALING_UNSPECIFIED,
        },
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        // 缓冲数量必须与对外能力事实保持同步。
        BufferCount: contract.buffer_count,
        OutputWindow: HWND(hwnd),
        Windowed: TRUE,
        // 交换效果必须与 present coherency 和 partial-present 事实保持同步。
        SwapEffect: contract.swap_effect,
        Flags: 0,
    }
}

fn d3d_hresult_code(result: ::windows::core::HRESULT) -> Errc {
    match result {
        DXGI_STATUS_OCCLUDED => Errc::GraphicsOccluded,
        DXGI_ERROR_DEVICE_HUNG
        | DXGI_ERROR_DEVICE_REMOVED
        | DXGI_ERROR_DEVICE_RESET
        | DXGI_ERROR_DRIVER_INTERNAL_ERROR => Errc::GraphicsDeviceLost,
        E_OUTOFMEMORY | DXGI_ERROR_REMOTE_OUTOFMEMORY => Errc::GraphicsOutOfMemory,
        _ => Errc::PlatformError,
    }
}

pub(super) fn d3d_error(operation: &str, err: ::windows::core::Error) -> Error {
    Error::new(
        d3d_hresult_code(err.code()),
        format!("D3d11Context: {operation} failed: {err}"),
    )
}

pub(crate) fn map_dxgi_present_result(result: ::windows::core::HRESULT) -> Result<()> {
    if result == DXGI_STATUS_OCCLUDED {
        return Err(Error::new(
            Errc::GraphicsOccluded,
            format!("D3d11Context: IDXGISwapChain::Present reported occlusion: {result:?}"),
        ));
    }
    map_dxgi_operation_result("IDXGISwapChain::Present", result)
}

pub(crate) fn map_dxgi_present_test_result(
    result: ::windows::core::HRESULT,
) -> Result<PresentTestResult> {
    if result == DXGI_STATUS_OCCLUDED {
        return Ok(PresentTestResult::Occluded);
    }
    map_dxgi_operation_result("IDXGISwapChain::Present(DXGI_PRESENT_TEST)", result)?;
    Ok(PresentTestResult::Presentable)
}

pub(crate) fn map_dxgi_resize_result(result: ::windows::core::HRESULT) -> Result<()> {
    map_dxgi_operation_result("IDXGISwapChain::ResizeBuffers", result)
}

// 把 D3D11 设备移除查询统一纳入已有 HRESULT 分类边界。
pub(crate) fn map_dxgi_device_removed_reason(result: ::windows::core::HRESULT) -> Result<()> {
    // 健康设备返回 S_OK，已移除或重置设备返回 GraphicsDeviceLost。
    map_dxgi_operation_result("ID3D11Device::GetDeviceRemovedReason", result)
}

fn map_dxgi_operation_result(operation: &str, result: ::windows::core::HRESULT) -> Result<()> {
    if result.is_err() {
        return Err(Error::new(
            d3d_hresult_code(result),
            format!("D3d11Context: {operation} failed: {result:?}"),
        ));
    }
    Ok(())
}

// 固定 D3D11 设备健康查询的 HRESULT 分类契约。
#[cfg(test)]
mod tests {
    // 引入设备移除映射函数。
    use super::map_dxgi_device_removed_reason;
    // 引入 swapchain descriptor 与单一能力事实。
    use super::{swap_chain_contract, swap_chain_desc};
    // 引入当前生产交换效果常量。
    use super::DXGI_SWAP_EFFECT_DISCARD;
    // 引入统一错误码。
    use crate::core::Errc;
    // 引入跨后端共享的 present coherency 类型。
    use crate::native::present::PresentCoherency;
    // 引入 Windows HRESULT 及设备移除常量。
    use ::windows::core::HRESULT;
    // 引入 DXGI 设备移除状态。
    use ::windows::Win32::Graphics::Dxgi::DXGI_ERROR_DEVICE_REMOVED;

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

    // 验证 DISCARD descriptor 不会对上层误报窄 present 能力。
    #[test]
    // 执行当前生产 swapchain 的一致性断言。
    fn discard_swapchain_keeps_full_only_present_contract() {
        // 读取单一能力事实。
        let contract = swap_chain_contract();
        // 使用空窗口句柄构造纯数据 descriptor；本测试不会调用 DXGI。
        let descriptor = swap_chain_desc(std::ptr::null_mut(), 640, 480);
        // descriptor 必须使用契约声明的双缓冲数量。
        assert_eq!(descriptor.BufferCount, contract.buffer_count);
        // descriptor 必须继续使用不保留内容的 DISCARD 模型。
        assert_eq!(descriptor.SwapEffect, DXGI_SWAP_EFFECT_DISCARD);
        // DISCARD 模型只能向 Graphics System 提供完整提交证明。
        assert_eq!(contract.present_coherency, PresentCoherency::FullOnly);
        // DISCARD 模型不得宣称支持 compositor 脏矩形。
        assert!(!contract.partial_present);
    }
}

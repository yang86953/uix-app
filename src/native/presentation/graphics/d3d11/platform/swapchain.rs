//! D3D11 bitblt swapchain 描述与 HRESULT 分类边界。

use std::ffi::c_void;

use crate::core::{Errc, Error, Result};
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

pub(crate) fn swap_chain_desc(hwnd: *mut c_void, width: i32, height: i32) -> DXGI_SWAP_CHAIN_DESC {
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
        BufferCount: 2,
        OutputWindow: HWND(hwnd),
        Windowed: TRUE,
        SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
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
    // 引入统一错误码。
    use crate::core::Errc;
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
}

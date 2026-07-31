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

fn map_dxgi_operation_result(operation: &str, result: ::windows::core::HRESULT) -> Result<()> {
    if result.is_err() {
        return Err(Error::new(
            d3d_hresult_code(result),
            format!("D3d11Context: {operation} failed: {result:?}"),
        ));
    }
    Ok(())
}

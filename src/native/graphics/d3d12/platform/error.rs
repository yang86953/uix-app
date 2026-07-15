//! D3D12/DXGI 错误到框架 typed error 的统一映射。

use crate::core::{Errc, Error};
use ::windows::Win32::Foundation::E_OUTOFMEMORY;
use ::windows::Win32::Graphics::Dxgi::{
    DXGI_ERROR_DEVICE_HUNG, DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET,
    DXGI_ERROR_DRIVER_INTERNAL_ERROR, DXGI_ERROR_REMOTE_OUTOFMEMORY,
};

pub(crate) fn d3d12_hresult_code(result: ::windows::core::HRESULT) -> Errc {
    match result {
        DXGI_ERROR_DEVICE_HUNG
        | DXGI_ERROR_DEVICE_REMOVED
        | DXGI_ERROR_DEVICE_RESET
        | DXGI_ERROR_DRIVER_INTERNAL_ERROR => Errc::GraphicsDeviceLost,
        E_OUTOFMEMORY | DXGI_ERROR_REMOTE_OUTOFMEMORY => Errc::GraphicsOutOfMemory,
        _ => Errc::PlatformError,
    }
}

pub(super) fn d3d12_error(operation: &str, error: ::windows::core::Error) -> Error {
    Error::new(
        d3d12_hresult_code(error.code()),
        format!("D3d12Context: {operation} failed: {error}"),
    )
}

pub(super) fn platform_error(message: impl Into<String>) -> Error {
    Error::new(Errc::PlatformError, message)
}

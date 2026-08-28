//! D3D12/DXGI 错误到框架 typed error 的统一映射。

use crate::core::{Errc, Error};
// HRESULT 分类归 D3D11/D3D12 共享的 DXGI 组件唯一持有；D3D12 由此恢复
// 遮挡、桌面访问丢失与显示模式切换的 Surface 级恢复语义。
use crate::native::presentation::graphics::dxgi::d3d_hresult_code;

pub(super) fn d3d12_error(operation: &str, error: ::windows::core::Error) -> Error {
    Error::new(
        d3d_hresult_code(error.code()),
        format!("D3d12Context: {operation} failed: {error}"),
    )
}

pub(super) fn platform_error(message: impl Into<String>) -> Error {
    Error::new(Errc::PlatformError, message)
}

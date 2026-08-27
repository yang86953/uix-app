//! 原生 D3D11 adapter 枚举。

use crate::core::Result;
#[cfg(not(windows))]
use crate::core::{Errc, Error};
use crate::platform::graphics::GpuAdapterInfo;
#[cfg(windows)]
use crate::platform::graphics::GraphicsBackend;

/// 枚举当前系统可供 D3D11 使用的 owned adapter 描述。
#[cfg(windows)]
pub(crate) fn enumerate_adapters() -> Result<Box<[GpuAdapterInfo]>> {
    crate::native::presentation::graphics::dxgi::enumerate_adapters(GraphicsBackend::Direct3D11)
}

/// 非 Windows 目标保留稳定的类型化不支持结果。
#[cfg(not(windows))]
pub(crate) fn enumerate_adapters() -> Result<Box<[GpuAdapterInfo]>> {
    Err(Error::new(
        Errc::NotImplemented,
        "Platform::gpu_adapters: Direct3D11 enumeration is unavailable on this target",
    ))
}

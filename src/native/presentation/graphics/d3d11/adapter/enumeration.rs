//! 原生 D3D11 adapter 枚举。

use crate::core::{Errc, Error, Result};
use crate::platform::graphics::GpuAdapterInfo;
#[cfg(windows)]
use crate::platform::graphics::{GpuDeviceType, GraphicsBackend};

/// 枚举当前系统可供 D3D11 使用的 owned adapter 描述。
#[cfg(windows)]
pub(crate) fn enumerate_adapters() -> Result<Box<[GpuAdapterInfo]>> {
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_ADAPTER_DESC1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND,
        IDXGIFactory1,
    };

    // SAFETY: CreateDXGIFactory1 返回进程级工厂，不借用外部句柄。
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.map_err(|error| {
        Error::new(
            Errc::PlatformError,
            format!("Platform::gpu_adapters: CreateDXGIFactory1 failed: {error}"),
        )
    })?;
    let mut adapters = Vec::new();
    for index in 0.. {
        // SAFETY: 返回的 adapter 由 factory 管理，只在本次循环查询。
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(error) => {
                return Err(Error::new(
                    Errc::PlatformError,
                    format!("Platform::gpu_adapters: EnumAdapters1 failed: {error}"),
                ));
            }
        };
        // SAFETY: 描述值是尺寸正确且唯一可写的栈对象。
        let desc: DXGI_ADAPTER_DESC1 = unsafe { adapter.GetDesc1() }.map_err(|error| {
            Error::new(
                Errc::PlatformError,
                format!("Platform::gpu_adapters: GetDesc1 failed: {error}"),
            )
        })?;
        // DXGI 只可靠标识软件设备，其他类型保持 Unknown。
        let software =
            desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 || desc.VendorId == 0x1414;
        adapters.push(GpuAdapterInfo::new(
            GraphicsBackend::Direct3D11,
            if software {
                GpuDeviceType::Software
            } else {
                GpuDeviceType::Unknown
            },
            non_empty(String::from_utf16_lossy(&desc.Description)),
            (desc.VendorId != 0).then_some(desc.VendorId),
            (desc.DeviceId != 0).then_some(desc.DeviceId),
            None,
        ));
    }
    Ok(adapters.into_boxed_slice())
}

/// 非 Windows 目标保留稳定的类型化不支持结果。
#[cfg(not(windows))]
pub(crate) fn enumerate_adapters() -> Result<Box<[GpuAdapterInfo]>> {
    Err(Error::new(
        Errc::NotImplemented,
        "Platform::gpu_adapters: Direct3D11 enumeration is unavailable on this target",
    ))
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

//! Windows DXGI 共享组件：adapter 枚举与 D3D11/D3D12 共用的 HRESULT 分类。

use crate::core::{Errc, Error, Result};
use crate::platform::graphics::{GpuAdapterInfo, GpuDeviceType, GraphicsBackend};

/// 把 D3D/DXGI HRESULT 归一到 UIX typed error（D3D11 与 D3D12 共用的唯一分类权威）。
///
/// 新增后端或原生状态时在此扩展分类，禁止在单个 adapter 内复制谓词，
/// 否则同一 HRESULT 在两条图形路径会得到不同恢复语义。
pub(crate) fn d3d_hresult_code(result: ::windows::core::HRESULT) -> Errc {
    use windows::Win32::Foundation::{
        DXGI_STATUS_OCCLUDED, E_OUTOFMEMORY,
    };
    use windows::Win32::Graphics::Dxgi::{
        DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_DEVICE_HUNG, DXGI_ERROR_DEVICE_REMOVED,
        DXGI_ERROR_DEVICE_RESET, DXGI_ERROR_DRIVER_INTERNAL_ERROR,
        DXGI_ERROR_MODE_CHANGE_IN_PROGRESS, DXGI_ERROR_REMOTE_OUTOFMEMORY,
    };
    // 按已批准的恢复分类映射原生状态。
    match result {
        // 遮挡是可恢复的窗口呈现状态。
        DXGI_STATUS_OCCLUDED => Errc::GraphicsOccluded,
        // 设备挂起、移除、重置和驱动内部错误进入 device rebuild。
        DXGI_ERROR_DEVICE_HUNG
        | DXGI_ERROR_DEVICE_REMOVED
        | DXGI_ERROR_DEVICE_RESET
        | DXGI_ERROR_DRIVER_INTERNAL_ERROR => Errc::GraphicsDeviceLost,
        // 桌面访问或显示模式切换失效只重建当前窗口 Surface。
        DXGI_ERROR_ACCESS_LOST | DXGI_ERROR_MODE_CHANGE_IN_PROGRESS => Errc::GraphicsSurfaceLost,
        // 本地或远程图形内存不足保留独立分类。
        E_OUTOFMEMORY | DXGI_ERROR_REMOTE_OUTOFMEMORY => Errc::GraphicsOutOfMemory,
        // 其它 HRESULT 由平台错误通道报告。
        _ => Errc::PlatformError,
    }
}

/// 通过 DXGI 枚举当前系统的 owned adapter 描述。
pub(crate) fn enumerate_adapters(backend: GraphicsBackend) -> Result<Box<[GpuAdapterInfo]>> {
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
            backend,
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
    if adapters.is_empty() {
        return Err(Error::new(
            Errc::NotFound,
            "Platform::gpu_adapters: DXGI reported no adapters",
        ));
    }
    Ok(adapters.into_boxed_slice())
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

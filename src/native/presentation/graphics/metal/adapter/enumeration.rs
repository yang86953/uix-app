//! Metal 物理设备枚举。

use objc2_metal::{MTLCopyAllDevices, MTLDevice};

use crate::core::{Errc, Error, Result};
use crate::platform::graphics::{GpuAdapterInfo, GpuDeviceType, GraphicsBackend};

/// 通过 Metal 系统 API 返回 owned adapter 描述，不保留原生设备句柄。
pub(crate) fn enumerate_adapters() -> Result<Box<[GpuAdapterInfo]>> {
    let adapters = MTLCopyAllDevices()
        .to_vec()
        .into_iter()
        .map(|device| {
            let device_type = if device.hasUnifiedMemory() || device.isLowPower() {
                GpuDeviceType::Integrated
            } else {
                GpuDeviceType::Discrete
            };
            GpuAdapterInfo::new(
                GraphicsBackend::Metal,
                device_type,
                non_empty(device.name().to_string()),
                None,
                None,
                None,
            )
        })
        .collect::<Vec<_>>();
    if adapters.is_empty() {
        return Err(Error::new(
            Errc::NotFound,
            "Platform::gpu_adapters: Metal reported no devices",
        ));
    }
    Ok(adapters.into_boxed_slice())
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

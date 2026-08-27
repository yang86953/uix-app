//! Vulkan graphics context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::platform::graphics::GpuAdapterInfo;
use crate::platform::presentation::GraphicsContextCandidate;

#[path = "adapter/mod.rs"]
pub(crate) mod platform;

// 原生 factory 只经本模块入口获取 Vulkan adapter 快照，不穿透内部目录。
#[cfg(any(unix, windows))]
pub(crate) fn enumerate_adapters() -> Result<Box<[GpuAdapterInfo]>> {
    platform::enumerate_adapters()
}

// 非桌面目标保留明确的 Vulkan 枚举不支持结果。
#[cfg(not(any(unix, windows)))]
pub(crate) fn enumerate_adapters() -> Result<Box<[GpuAdapterInfo]>> {
    Err(Error::new(
        crate::core::Errc::NotImplemented,
        "Platform::gpu_adapters: Vulkan enumeration is unavailable on this target",
    ))
}

pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 返回平台层已经组装的 context candidate。
) -> Result<GraphicsContextCandidate, Error> {
    platform::create(surface, width, height)
}

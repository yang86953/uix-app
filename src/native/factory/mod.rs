//! 原生图形 recipe 工厂；平台聚合由 `platform::composition_root` 唯一组装。

pub(crate) mod registry;
#[cfg(target_os = "linux")]
pub(crate) mod registry_linux;
#[cfg(target_os = "macos")]
pub(crate) mod registry_macos;
#[cfg(windows)]
pub(crate) mod registry_windows;
pub(crate) mod thread_bound;

#[cfg(any(feature = "opengles", all(feature = "metal", not(target_os = "macos"))))]
use crate::core::error::Error;
use crate::platform::graphics::{GpuAdapterInfo, GraphicsBackend};
#[cfg(any(windows, target_os = "linux"))]
use crate::platform::system::info::ISystemInfo;

pub(crate) use registry::try_create_gpu_recipe_with_queue;
pub(crate) use registry::{
    GraphicsRecipe, describe_backend_availability, gpu_recipe_candidates, graphics_runtime_platform,
};

// 按公开中立选择值挑选唯一原生 adapter；factory 不读取任何 API 句柄。
#[cfg(any(
    feature = "d3d11",
    feature = "vulkan",
    feature = "d3d12",
    feature = "metal",
    feature = "opengles"
))]
pub(crate) fn enumerate_gpu_adapters(
    backend: GraphicsBackend,
) -> crate::core::Result<Box<[GpuAdapterInfo]>> {
    match backend {
        // 具体枚举实现留在对应原生 adapter。
        #[cfg(feature = "d3d11")]
        GraphicsBackend::Direct3D11 => {
            crate::native::presentation::graphics::d3d11::enumerate_adapters()
        }
        #[cfg(feature = "d3d12")]
        GraphicsBackend::Direct3D12 => {
            crate::native::presentation::graphics::d3d12::enumerate_adapters()
        }
        #[cfg(feature = "vulkan")]
        GraphicsBackend::Vulkan => {
            crate::native::presentation::graphics::vulkan::enumerate_adapters()
        }
        #[cfg(all(target_os = "macos", feature = "metal"))]
        GraphicsBackend::Metal => {
            crate::native::presentation::graphics::metal::enumerate_adapters()
        }
        // 当前平台未实现的已启用 API 保持明确诊断。
        #[cfg(any(feature = "opengles", all(feature = "metal", not(target_os = "macos"))))]
        _ => Err(Error::new(
            crate::core::Errc::NotImplemented,
            format!(
                "Platform::gpu_adapters: {:?} has no native enumerator on this target",
                backend
            ),
        )),
    }
}

// 没有图形 feature 时选择枚举不可构造，空 match 保持编译期闭合。
#[cfg(not(any(
    feature = "d3d11",
    feature = "vulkan",
    feature = "d3d12",
    feature = "metal",
    feature = "opengles"
)))]
pub(crate) fn enumerate_gpu_adapters(
    backend: GraphicsBackend,
) -> crate::core::Result<Box<[GpuAdapterInfo]>> {
    match backend {}
}


/// 探测系统可用空闲内存（字节）。
// 保留平台诊断查询入口，默认应用路径不直接依赖它。
#[allow(dead_code)]
#[cfg(target_os = "linux")]
pub(crate) fn available_memory_bytes() -> u64 {
    crate::native::backends::linux::system_info::LinuxSystemInfo::new()
        .memory_info()
        .map(|info| info.available_bytes)
        .unwrap_or(0)
}

// 保留平台诊断查询入口，默认应用路径不直接依赖它。
#[allow(dead_code)]
#[cfg(windows)]
pub(crate) fn available_memory_bytes() -> u64 {
    crate::native::backends::windows::system_info::WindowsSystemInfo::new()
        .memory_info()
        .map(|info| info.available_bytes)
        .unwrap_or(0)
}

// 保留平台诊断查询入口，默认应用路径不直接依赖它。
#[allow(dead_code)]
#[cfg(target_os = "macos")]
pub(crate) fn available_memory_bytes() -> u64 {
    512 * 1024 * 1024
}

// 保留平台诊断查询入口，默认应用路径不直接依赖它。
#[allow(dead_code)]
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub(crate) fn available_memory_bytes() -> u64 {
    512 * 1024 * 1024
}

// 仅测试构建的 WARP/绘制辅助位于 tests-src，经模块级 include! 保持原作用域。
#[cfg(test)]
include!("../../../tests-src/native/factory/warp_test_fns.rs");

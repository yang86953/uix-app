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

// 为测试专用 WARP context 建立线程亲和 wrapper。
#[cfg(all(test, any(feature = "d3d11", feature = "d3d12")))]
fn bind_test_context_to_current_thread(
    // 接收测试 adapter 新创建的类型化 GPU context。
    context: Box<dyn crate::platform::presentation::GpuRecipeContext>,
) -> Box<dyn crate::platform::presentation::GpuRecipeContext> {
    // WARP 测试只验证原生资源与生命周期，不再反向探测静态 capability。
    let bound = thread_bound::bind_to_current_thread(
        // 固化 WARP 测试入口的 GPU recipe 类型。
        crate::platform::presentation::GraphicsRecipeContext::Gpu(context),
    );
    // 取回线程绑定后的同一 GPU recipe 分支。
    match bound {
        // 返回不可选的 GPU owner。
        crate::platform::presentation::GraphicsRecipeContext::Gpu(context) => context,
        // 构造输入固定为 GPU，因此该分支不可达。
        crate::platform::presentation::GraphicsRecipeContext::PixelUpload(_) => {
            // 防止未来 bind 实现破坏 recipe 保持契约。
            unreachable!("GPU test context binding changed recipe variant")
        }
    }
}

// 测试目标保留 D3D11 WARP 工厂入口，供显式后端矩阵按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, feature = "d3d11"))]
pub(crate) fn create_d3d11_warp_test_context(
    surface: *mut std::ffi::c_void,
    width: i32,
    height: i32,
) -> crate::core::Result<Box<dyn crate::platform::presentation::GpuRecipeContext>> {
    crate::native::presentation::graphics::d3d11::create_warp_test_context(surface, width, height)
        // 在测试 factory 边界建立线程门禁。
        .map(bind_test_context_to_current_thread)
}

// 测试目标保留 D3D11 WARP 可用性探测，供显式后端矩阵按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, feature = "d3d11"))]
pub(crate) fn d3d11_warp_test_context_available() -> bool {
    crate::native::presentation::graphics::d3d11::warp_test_context_available()
}

#[cfg(all(test, feature = "d3d12"))]
pub(crate) fn create_d3d12_warp_test_context(
    surface: *mut std::ffi::c_void,
    width: i32,
    height: i32,
) -> crate::core::Result<Box<dyn crate::platform::presentation::GpuRecipeContext>> {
    crate::native::presentation::graphics::d3d12::create_warp_test_context(surface, width, height)
        // 在测试 factory 边界建立线程门禁。
        .map(bind_test_context_to_current_thread)
}

#[cfg(all(test, feature = "d3d12"))]
pub(crate) fn d3d12_warp_test_context_available() -> bool {
    crate::native::presentation::graphics::d3d12::warp_test_context_available()
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

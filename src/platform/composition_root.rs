//! Platform System 唯一原生组合根。
//!
//! 本叶只负责消费中立启动输入，选择当前目标的具体实现并转移对象所有权。

use crate::core::Result;
#[cfg(not(any(windows, unix)))]
use crate::core::{Errc, Error};
use crate::platform::graphics::{GpuAdapterInfo, GraphicsBackend};
use crate::platform::platform::PlatformSystem;
use crate::platform::presentation::GraphicsApi;

use super::composition::PendingNativeOptions;

// 原生图形 recipe 与 surface owner 只经唯一组合根进入 platform 公开窄路径。
pub(crate) use crate::native::factory::{
    GraphicsRecipe, describe_backend_availability, gpu_recipe_candidates,
    graphics_runtime_platform, try_create_gpu_recipe_with_queue,
};

impl GraphicsBackend {
    /// 将公开具体 API 转换为 crate-private registry 身份。
    pub(crate) const fn into_native(self) -> GraphicsApi {
        match self {
            // 公开 D3D11 变体只在对应实现参与构建时存在。
            #[cfg(feature = "d3d11")]
            Self::Direct3D11 => GraphicsApi::D3d11,
            // 公开 Vulkan 变体只在对应实现参与构建时存在。
            #[cfg(feature = "vulkan")]
            Self::Vulkan => GraphicsApi::Vulkan,
            // 公开 D3D12 变体只在对应实现参与构建时存在。
            #[cfg(feature = "d3d12")]
            Self::Direct3D12 => GraphicsApi::D3d12,
            // 公开 Metal 变体只在对应实现参与构建时存在。
            #[cfg(feature = "metal")]
            Self::Metal => GraphicsApi::Metal,
            // 公开 OpenGL ES 变体只在对应实现参与构建时存在。
            #[cfg(feature = "opengles")]
            Self::OpenGlEs => GraphicsApi::OpenGlEs,
        }
    }
}

/// 按中立公开值枚举当前目标的原生 GPU adapter。
pub(super) fn enumerate_gpu_adapters(backend: GraphicsBackend) -> Result<Box<[GpuAdapterInfo]>> {
    crate::native::factory::enumerate_gpu_adapters(backend)
}

/// 调用当前 Linux 目标的原生文件多选 adapter。
#[cfg(target_os = "linux")]
pub(super) fn choose_native_files(
    title: &str,
    zenity_filters: &str,
    kdialog_filters: &str,
) -> Result<Option<Vec<String>>> {
    crate::native::backends::linux::file_dialog::choose_files(
        title,
        zenity_filters,
        kdialog_filters,
    )
}

/// 调用当前 Windows 或 macOS 目标的原生文件多选 adapter。
#[cfg(windows)]
pub(super) fn choose_native_files(title: &str, filters: &str) -> Result<Option<Vec<String>>> {
    crate::native::backends::windows::file_dialog::choose_files(title, filters)
}

/// 调用当前 macOS 目标的原生文件多选 adapter。
#[cfg(target_os = "macos")]
pub(super) fn choose_native_files(title: &str, filters: &str) -> Result<Option<Vec<String>>> {
    crate::native::backends::macos::platform::file_dialog::choose_files(title, filters)
}

/// 调用当前 Linux 目标的原生文件保存 adapter。
#[cfg(target_os = "linux")]
pub(super) fn choose_native_save_file(
    title: &str,
    zenity_filters: &str,
    kdialog_filters: &str,
) -> Result<Option<String>> {
    crate::native::backends::linux::file_dialog::choose_save_file(
        title,
        zenity_filters,
        kdialog_filters,
    )
}

/// 调用当前 Windows 目标的原生文件保存 adapter。
#[cfg(windows)]
pub(super) fn choose_native_save_file(title: &str, filters: &str) -> Result<Option<String>> {
    crate::native::backends::windows::file_dialog::choose_save_file(title, filters)
}

/// 调用当前 macOS 目标的原生文件保存 adapter。
#[cfg(target_os = "macos")]
pub(super) fn choose_native_save_file(title: &str, filters: &str) -> Result<Option<String>> {
    crate::native::backends::macos::platform::file_dialog::choose_save_file(title, filters)
}

/// 调用当前目标的原生目录选择 adapter。
#[cfg(windows)]
pub(super) fn choose_native_folder(title: &str) -> Result<Option<String>> {
    crate::native::backends::windows::file_dialog::choose_folder(title)
}

/// 调用当前 Linux 目标的原生目录选择 adapter。
#[cfg(target_os = "linux")]
pub(super) fn choose_native_folder(title: &str) -> Result<Option<String>> {
    crate::native::backends::linux::file_dialog::choose_folder(title)
}

/// 调用当前 macOS 目标的原生目录选择 adapter。
#[cfg(target_os = "macos")]
pub(super) fn choose_native_folder(title: &str) -> Result<Option<String>> {
    crate::native::backends::macos::platform::file_dialog::choose_folder(title)
}

/// 为当前编译目标创建并拥有唯一的平台聚合。
#[cfg(windows)]
pub(crate) fn create_platform_with_pending(
    options: PendingNativeOptions,
) -> Result<Box<dyn PlatformSystem>> {
    let pending_failures = options.into_pending_failures();
    Ok(Box::new(
        crate::native::backends::windows::platform::WindowsPlatform::new_with_pending(
            pending_failures,
        ),
    ))
}

/// 为当前编译目标创建并拥有唯一的平台聚合。
#[cfg(target_os = "linux")]
pub(crate) fn create_platform_with_pending(
    options: PendingNativeOptions,
) -> Result<Box<dyn PlatformSystem>> {
    let pending_failures = options.into_pending_failures();
    let platform = crate::native::backends::linux::platform::LinuxPlatform::new(pending_failures)?;
    Ok(Box::new(platform))
}

/// 为当前编译目标创建并拥有唯一的平台聚合。
#[cfg(target_os = "macos")]
pub(crate) fn create_platform_with_pending(
    options: PendingNativeOptions,
) -> Result<Box<dyn PlatformSystem>> {
    let pending_failures = options.into_pending_failures();
    Ok(Box::new(
        crate::native::backends::macos::platform::MacosPlatform::new(pending_failures),
    ))
}

/// 对尚未适配的编译目标返回稳定的平台错误。
#[cfg(not(any(windows, unix)))]
pub(crate) fn create_platform_with_pending(
    options: PendingNativeOptions,
) -> Result<Box<dyn PlatformSystem>> {
    let _pending_failures = options.into_pending_failures();
    Err(Error::new(
        Errc::PlatformError,
        unsupported_platform_message(),
    ))
}

// 跨平台不支持分支保留统一错误文案，供目标矩阵静态核对。
#[cfg_attr(any(windows, unix), allow(dead_code))]
fn unsupported_platform_message() -> String {
    "Unsupported platform: only Windows, Linux, and macOS are supported".to_string()
}

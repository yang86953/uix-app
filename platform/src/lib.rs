// ============================================================================
// platform/mod.rs — 平台层模块入口
//
// 平台层架构：
//   api.rs       — 统一 API 契约：所有 trait 定义
//   event.rs     — 事件类型（UiEvent 及其载荷）
//   presenter.rs — NullPresenter（空操作实现）
//   system_info.rs — 系统信息查询工具函数（probe_system_default_font）
//   linux/       — Linux（Wayland）平台实现
//   windows/     — Windows 平台实现
// ============================================================================

// ── 核心 API 契约 ──────────────────────────────────────────────
pub mod api;
pub use api::*;

// ── 共享类型与事件 ──────────────────────────────────────────────
pub mod event;

// ── 平台专属类型 ─────────────────────────────────────────────
pub mod types;

// ── 像素呈现 ───────────────────────────────────────────────────
pub mod presenter;

// ── 跨平台共享核心（状态/事件循环/窗口实现）────────────────
pub mod core;

// ── 平台工厂 ───────────────────────────────────────────────────

/// 创建当前平台对应的 Platform 实例。
///
/// 返回 `Err` 如果平台初始化失败（如 Linux 下无 Wayland 会话）。
#[cfg(windows)]
pub fn create_platform() -> Result<Box<dyn Platform>, uix_diag::Error> {
    Ok(Box::new(crate::windows::platform::WindowsPlatform::new()))
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn create_platform() -> Result<Box<dyn Platform>, uix_diag::Error> {
    let platform = crate::linux::platform::LinuxPlatform::new()?;
    Ok(Box::new(platform))
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
pub fn create_platform() -> Result<Box<dyn Platform>, uix_diag::Error> {
    compile_error!("Unsupported platform: only Windows and Linux are supported");
}

// ── GPU 图形上下文工厂 ───────────────────────────────────────────

/// 创建 GPU 图形上下文（IGraphicsContext）。
///
/// Linux 下通过 EGL + GLES 创建，Windows 下不支持 GPU 渲染。
/// `native_surface` 是平台原生 surface 指针（Linux: wl_surface, Windows: 未使用）。
#[cfg(all(unix, not(target_os = "macos")))]
pub fn create_gpu_context(
    native_surface: *mut std::ffi::c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, uix_diag::Error> {
    let egl = crate::linux::gpu::egl::EglContext::new(native_surface, width, height)?;
    Ok(Box::new(egl))
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
pub fn create_gpu_context(
    _native_surface: *mut std::ffi::c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, uix_diag::Error> {
    Err(uix_diag::Error::new(
        uix_diag::Errc::PlatformError,
        "GPU rendering is not supported on this platform".to_string(),
    ))
}

// ── 平台实现 ───────────────────────────────────────────────────
#[cfg(windows)]
pub mod windows;

#[cfg(all(unix, not(target_os = "macos")))]
pub mod linux;

// ── 系统信息便捷函数 ───────────────────────────────────────────

/// 探测系统可用空闲内存（字节），不需要完整的 Platform 实例。
/// 供大脑初始化等早期阶段使用。
#[cfg(all(unix, not(target_os = "macos")))]
pub fn available_memory_bytes() -> u64 {
    linux::system_info::LinuxSystemInfo::new().memory_info().available_bytes
}

#[cfg(windows)]
pub fn available_memory_bytes() -> u64 {
    windows::system_info::WindowsSystemInfo::new().memory_info().available_bytes
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
pub fn available_memory_bytes() -> u64 {
    512 * 1024 * 1024 // 回退：512 MB
}

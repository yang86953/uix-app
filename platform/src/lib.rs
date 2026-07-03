// ============================================================================
// platform/lib.rs — 平台层模块入口
//
// 平台层架构：
//   geometry.rs  — 几何类型（Point/Size/Rect/EdgeInsets）
//   error.rs     — 错误码与 Error 结构体
//   event.rs     — 事件类型（UiEvent 及其载荷）
//   log/         — 日志基础设施（+ Logger/Sink 高级日志）
//   presenter.rs — 像素呈现器（IPresenter/IGraphicsContext/NullPresenter）
//   types/       — 平台层数据类型与 API trait（key/input/console/display/system/status）
//   shared/      — 跨平台共享实现 + 窗口/事件 API trait（event_loop/window/platform）
//   diagnostic/  — 诊断系统（错误收集/崩溃处理/恢复策略/时间戳/中间件）
//   linux/       — Linux（Wayland）平台实现
//   windows/     — Windows 平台实现
// ============================================================================

// ── 稳定公开 API ─────────────────────────────────────────────
pub mod api;

// ── 共享基础类型 ──────────────────────────────────────────────
pub mod geometry;
pub mod error;

// ── 事件类型 ───────────────────────────────────────────────────
pub mod event;

// ── 事件订阅/发布总线 ─────────────────────────────────────────
pub mod event_bus;

// ── 日志基础设施 ──────────────────────────────────────────────
pub mod log;

// ── 像素呈现器与 API trait ────────────────────────────────────
pub mod presenter;

// ── 平台层数据类型与 API trait ────────────────────────────────
pub mod types;

// ── 跨平台共享实现与窗口/事件 API trait ──────────────────────
pub mod shared;

// ── 诊断系统（跨层使用）────────────────────────────────────
pub mod diagnostic;

// ── 业务服务（原 services crate，迁入 platform 层）────────────
pub mod file_service;
pub mod notification;
pub mod settings;

// ── 平台实现 ───────────────────────────────────────────────────
#[cfg(windows)]
pub mod windows;

#[cfg(all(unix, not(target_os = "macos")))]
pub mod linux;

// ── 便利重导出（统一通过 api 模块）─────────────────────────
pub use api::*;

// ── 平台工厂 ───────────────────────────────────────────────────

/// 创建当前平台对应的 Platform 实例。
///
/// 返回 `Err` 如果平台初始化失败（如 Linux 下无 Wayland 会话）。
#[cfg(windows)]
pub fn create_platform() -> Result<Box<dyn Platform>, Error> {
    Ok(Box::new(crate::windows::platform::WindowsPlatform::new()))
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn create_platform() -> Result<Box<dyn Platform>, Error> {
    let platform = crate::linux::platform::LinuxPlatform::new()?;
    Ok(Box::new(platform))
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
pub fn create_platform() -> Result<Box<dyn Platform>, Error> {
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
) -> Result<Box<dyn IGraphicsContext>, Error> {
    let egl = crate::linux::gpu::egl::EglContext::new(native_surface, width, height)?;
    Ok(Box::new(egl))
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
pub fn create_gpu_context(
    _native_surface: *mut std::ffi::c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(Error::new(
        Errc::PlatformError,
        "GPU rendering is not supported on this platform".to_string(),
    ))
}

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

// ── 测试框架（仅在启用 test-harness feature 时编译）────────────────
#[cfg(feature = "test-harness")]
pub mod test_harness;

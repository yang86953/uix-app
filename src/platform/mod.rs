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

// ── 平台工厂 ───────────────────────────────────────────────────

/// 创建当前平台对应的 Platform 实例。
///
/// 返回 `Err` 如果平台初始化失败（如 Linux 下无 Wayland 会话）。
#[cfg(windows)]
pub fn create_platform() -> Result<Box<dyn Platform>, crate::diag::Error> {
    Ok(Box::new(crate::platform::windows::platform::WindowsPlatform::new()))
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn create_platform() -> Result<Box<dyn Platform>, crate::diag::Error> {
    let platform = crate::platform::linux::platform::LinuxPlatform::new()?;
    Ok(Box::new(platform))
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
pub fn create_platform() -> Result<Box<dyn Platform>, crate::diag::Error> {
    compile_error!("Unsupported platform: only Windows and Linux are supported");
}

// ── 平台实现 ───────────────────────────────────────────────────
#[cfg(windows)]
pub mod windows;

#[cfg(all(unix, not(target_os = "macos")))]
pub mod linux;

// ============================================================================
// platform/mod.rs — 平台能力接口（聚合入口）
// ============================================================================

// ── 核心平台模块 ──────────────────────────────────────────────────
pub mod abstraction;
pub mod event;
pub mod types;

#[cfg(windows)]
pub mod windows;

#[cfg(all(unix, not(target_os = "macos")))]
pub mod linux;

// ── 像素呈现策略 ────────────────────────────────────────────────
pub mod presenter;

// ── 子系统 trait 模块 ─────────────────────────────────────────────
pub mod clipboard;
pub mod console;
pub mod cursor;
pub mod display;
pub mod file_dialog;
pub mod file_system;
pub mod input;
pub mod notification;
pub mod system_info;
pub mod timer;

// ── API 出口：所有公共 API 通过 api.rs 暴露 ──────────────────────
mod api;


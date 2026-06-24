// ============================================================================
// platform/linux/gpu/mod.rs — Linux GPU 图形上下文模块
//
// EglContext：基于 EGL + GLES 3.0 的 IGraphicsContext 实现，
// 对接 Wayland surface（通过 wl_egl_window）。
// ============================================================================

#![cfg(all(unix, not(target_os = "macos")))]

pub mod egl;
pub use egl::EglContext;

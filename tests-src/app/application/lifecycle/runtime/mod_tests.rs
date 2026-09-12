//! `src/app/application/lifecycle/runtime/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的自由 cfg(test) 项 ——

// 测试目标保留默认图形后端的 pending-window 便捷入口，供外部 GUI 测试按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(test)]
pub(crate) fn drain_pending_open_windows(
    platform: &mut dyn PlatformSystem,
    runtime: &AppRuntime,
    app_state: &AppState,
    container: &Container,
    theme: &Theme,
    on_window_start: Option<&Arc<dyn Fn(AppHandle) + Send + Sync>>,
    secondary_windows: &mut Vec<SecondaryWindowSession>,
) -> usize {
    drain_pending_open_windows_with_backend(
        platform,
        runtime,
        app_state,
        container,
        theme,
        GraphicsSelection::Automatic,
        RebuildRequest::default(),
        on_window_start,
        secondary_windows,
    )
}

// 测试目标保留无平台参数的 secondary-window 帧便捷入口，供外部 GUI 测试按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(test)]
pub(crate) fn drain_secondary_window_frames(
    secondary_windows: &mut [SecondaryWindowSession],
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    debug_mode: &Diagnostics,
    cursor_pos: &Cell<Point>,
    clock: &dyn AppClock,
) -> bool {
    drain_secondary_window_frames_impl(
        None,
        secondary_windows,
        font_service,
        image_service,
        theme,
        debug_mode,
        cursor_pos,
        clock,
    )
}
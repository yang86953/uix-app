//! `src/app/event_loop/event_loop.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的自由 cfg(test) 项 ——

#[allow(clippy::too_many_arguments)]
// 测试目标保留默认时钟的窗口循环入口，供外部 GUI 测试按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(test)]
pub(crate) fn run_window_session_loop<M, X, F>(
    platform: &mut dyn PlatformSystem,
    platform_window: &mut dyn PlatformWindow,
    session: &mut WindowSession,
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    debug_mode: &Diagnostics,
    cursor_pos: &Cell<Point>,
    metrics: Option<&Cell<RenderMetrics>>,
    map_event: M,
    on_exit: X,
    on_frame: F,
) -> i32
where
    M: Fn(&UiEvent) -> Option<SystemEvent>,
    X: Fn(&UiEvent) -> bool,
    F: Fn(&mut WidgetTree, &mut dyn RenderTarget, &mut dyn PlatformSystem),
{
    run_window_session_loop_with_system_theme(
        platform,
        platform_window,
        session,
        font_service,
        image_service,
        theme,
        None,
        debug_mode,
        cursor_pos,
        metrics,
        map_event,
        on_exit,
        on_frame,
    )
}

#[allow(clippy::too_many_arguments)]
// 测试目标保留系统主题兼容窗口循环入口，供外部 GUI 测试按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(test)]
pub(crate) fn run_window_session_loop_with_system_theme<M, X, F>(
    platform: &mut dyn PlatformSystem,
    platform_window: &mut dyn PlatformWindow,
    session: &mut WindowSession,
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    system_theme_tokens: Option<&DynTokens>,
    debug_mode: &Diagnostics,
    cursor_pos: &Cell<Point>,
    metrics: Option<&Cell<RenderMetrics>>,
    map_event: M,
    on_exit: X,
    on_frame: F,
) -> i32
where
    M: Fn(&UiEvent) -> Option<SystemEvent>,
    X: Fn(&UiEvent) -> bool,
    F: Fn(&mut WidgetTree, &mut dyn RenderTarget, &mut dyn PlatformSystem),
{
    run_window_session_loop_with_system_theme_and_tasks(
        platform,
        platform_window,
        session,
        font_service,
        image_service,
        theme,
        system_theme_tokens,
        debug_mode,
        cursor_pos,
        metrics,
        map_event,
        on_exit,
        |_, _| {},
        |_, _| {},
        || None,
        on_frame,
    )
}

#[allow(clippy::too_many_arguments)]
// 测试目标保留可注入时钟的窗口循环入口，供外部 GUI 测试按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(test)]
pub(crate) fn run_window_session_loop_with_clock<M, X, F>(
    platform: &mut dyn PlatformSystem,
    platform_window: &mut dyn PlatformWindow,
    session: &mut WindowSession,
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    clock: std::sync::Arc<dyn AppClock>,
    debug_mode: &Diagnostics,
    cursor_pos: &Cell<Point>,
    metrics: Option<&Cell<RenderMetrics>>,
    map_event: M,
    on_exit: X,
    on_frame: F,
) -> i32
where
    M: Fn(&UiEvent) -> Option<SystemEvent>,
    X: Fn(&UiEvent) -> bool,
    F: Fn(&mut WidgetTree, &mut dyn RenderTarget, &mut dyn PlatformSystem),
{
    run_window_session_loop_with_system_theme_and_clock(
        platform,
        platform_window,
        session,
        font_service,
        image_service,
        theme,
        None,
        clock,
        debug_mode,
        cursor_pos,
        metrics,
        map_event,
        on_exit,
        |_, _| {},
        |_, _| {},
        || None,
        on_frame,
    )
}
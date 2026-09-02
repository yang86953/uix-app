use crate::app::queues::active_work_registry::{ActiveWorkKind, ActiveWorkRegistry};
use crate::app::window::window_session::WindowTextInputState;
use crate::core::WindowId;
use crate::diagnostics::{Diagnostics, ReportOrigin};
use crate::platform::platform::PlatformSystem;
use crate::ui::WidgetTree;

/// Synchronize one window's declarative focus state with the process native IME.
///
/// The coordinator prevents a delayed blur from one window from stopping the
/// session that a different window has already activated in the same event batch.
pub(crate) fn sync_window_text_input(
    tree: &WidgetTree,
    active_work: &mut ActiveWorkRegistry,
    state: &mut WindowTextInputState,
    window_id: WindowId,
    native_window: *mut std::ffi::c_void,
    platform: &mut dyn PlatformSystem,
    diagnostics: &Diagnostics,
) {
    let requested = state.window_focused.then(|| {
        let target = tree.managers().focus.focused_widget()?;
        let client = tree.get(target)?.as_text_input()?;
        client
            .accepts_text_input()
            .then(|| (target, client.text_input_cursor_rect()))
    });
    let requested = requested.flatten();
    let requested_target = requested.map(|(target, _)| target);

    if state.ime_session != requested_target {
        if let Some(previous) = state.ime_session.take() {
            active_work.unregister(ActiveWorkKind::ImeSession(previous));
            if state.coordinator.active_window() == Some(window_id)
                && select_target(platform, window_id, native_window, diagnostics)
                && report_stop_error(platform.text_input().stop(), diagnostics)
            {
                state.coordinator.deactivate(window_id);
            }
        }
        state.cursor_rect = None;
    }

    let Some((target, cursor_rect)) = requested else {
        if state.coordinator.active_window() == Some(window_id)
            && select_target(platform, window_id, native_window, diagnostics)
            && report_stop_error(platform.text_input().stop(), diagnostics)
        {
            state.coordinator.deactivate(window_id);
        }
        return;
    };

    if state.coordinator.active_window() != Some(window_id) {
        if !select_target(platform, window_id, native_window, diagnostics) {
            return;
        }
        if let Err(error) = platform.text_input().start() {
            tracing::error!("IME session start failed: {}", error.short_what());
            // IME 激活失败直接放弃本次输入法会话，进入框架报告供宿主观察。
            diagnostics
                .report_with_origin(error, ReportOrigin::framework("ime", "start"));
            return;
        }
        state.coordinator.activate(window_id);
    }

    active_work.register_open(ActiveWorkKind::ImeSession(target));
    state.ime_session = Some(target);
    if cursor_rect.w <= 0.0 || cursor_rect.h <= 0.0 || state.cursor_rect == Some(cursor_rect) {
        return;
    }
    if let Err(error) = platform.text_input().set_cursor_rect(cursor_rect) {
        // 光标矩形随每次输入高频更新，经 observe_transient 冷却去重观察。
        diagnostics.observe_transient("ime", "cursor rect update failed", error.short_what());
    }
    state.cursor_rect = Some(cursor_rect);
}

fn select_target(
    platform: &mut dyn PlatformSystem,
    window_id: WindowId,
    native_window: *mut std::ffi::c_void,
    diagnostics: &Diagnostics,
) -> bool {
    if let Err(error) = platform
        .text_input()
        .set_target_window(window_id, native_window)
    {
        tracing::error!("IME target selection failed: {}", error.short_what());
        diagnostics.report_with_origin(error, ReportOrigin::framework("ime", "select_target"));
        return false;
    }
    true
}

fn report_stop_error(result: crate::core::Result<()>, diagnostics: &Diagnostics) -> bool {
    if let Err(error) = result {
        tracing::error!("IME session stop failed: {}", error.short_what());
        diagnostics.report_with_origin(error, ReportOrigin::framework("ime", "stop"));
        return false;
    }
    true
}

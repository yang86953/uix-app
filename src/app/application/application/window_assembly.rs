//! 主窗与副窗共用的窗口装配原语。
//!
//! 本模块收口两条创建路径中逐字重复的装配段：DI 回退解析、应用根工厂
//! 包装、会话资源接线与 Agent 窗口注册。装配顺序与语义以本模块为唯一
//! 权威；禁止在任一创建路径复制这些步骤，否则主/副窗行为会重新漂移。

use std::any::Any;

use super::*;

/// DI 未注册 `T` 时回退默认值并记录缺失，避免静默降级（保持回退行为）。
pub(super) fn resolve_or_default<T>(container: &Container) -> T
where
    T: Any + Send + Sync + Clone + Default,
{
    match container.resolve_clone::<T>() {
        Some(value) => value,
        None => {
            tracing::warn!(
                ty = %std::any::type_name::<T>(),
                "DI resolve failed, falling back to default"
            );
            T::default()
        }
    }
}

/// 按主窗/副窗统一顺序包装应用根：WidgetConfig → Locale → prepare_app_root。
///
/// 两个窗口来源（编译期根工厂 / `WindowRootFactory`）都经此包装，
/// 保证逐窗反馈浮层与默认值注入只有一套组装逻辑。
pub(super) fn wrap_app_root<R>(
    widget_config: &WidgetConfig,
    locale: &Locale,
    window_id: WindowId,
    feedback: Option<AppFeedbackState>,
    root: R,
) -> impl Fn() -> ViewNode + Send + Sync + 'static
where
    R: Fn() -> ViewNode + Send + Sync + 'static,
{
    let widget_config = widget_config.clone();
    let locale = locale.clone();
    move || {
        with_config(&widget_config, || {
            with_locale(&locale, || {
                prepare_app_root(root(), feedback.clone(), window_id)
            })
        })
    }
}

/// 会话资源接线与 Agent 窗口注册：主窗与副窗共用同一装配序列。
///
/// 调用方差异只允许出现在本序列之外（如主窗的 `register_session`、
/// 副窗的初始未聚焦写入）；序列内部步骤的增删必须同步两条创建路径。
pub(super) fn assemble_session_resources(
    session: &mut WindowSession,
    runtime: &AppRuntime,
    app_state: &AppState,
    app_timers: AppTimerQueue,
    main_thread_queue: MainThreadQueue,
    window_id: WindowId,
    title: &str,
    platform_window: &dyn PlatformWindow,
) {
    session.set_text_input_coordinator(runtime.text_input_coordinator());
    session.set_app_state(app_state.clone());
    session.set_app_timers(app_timers);
    session.set_main_thread_queue(main_thread_queue);
    if let Some(queue) = runtime.agent_command_queue(window_id) {
        session.set_agent_command_queue(queue);
    }
    session.set_agent_command_executor(runtime.agent_command_executor());
    session.set_agent_confirm_ui(runtime.agent_confirm_ui());
    let properties = platform_window.properties();
    if let Some(registration) = runtime.register_agent_window(
        window_id,
        title.to_owned(),
        platform_window.is_visible(),
        super::defaults::initially_agent_presentable(platform_window),
        // 焦点事实只随 WindowFocus/WindowBlur 事件更新，注册时按未聚焦处理。
        false,
        properties.width(),
        properties.height(),
        properties.is_maximized(),
        properties.is_minimized(),
        properties.is_fullscreen(),
    ) {
        // bind_agent_window 只返回 bool，无法区分「已绑定」与「窗口已关闭/
        // id 不匹配」等失败原因；改为 Result 会波及全部调用点与签名，
        // 本次仅记录日志，保留弱返回值契约。
        if !session.bind_agent_window(registration) {
            tracing::warn!(window_id = ?window_id, "agent window binding failed");
        }
    }
}

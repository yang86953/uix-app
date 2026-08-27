//! Application 组合根的默认构造与窗口启动辅助边界。

// 复用父 Module 已声明的应用类型、服务与平台契约。
use super::*;

// 只在组合根的最终责任边界记录检查式窗口操作失败。
pub(super) fn report_window_operation_error(
    // 附带调用方提供的窄操作上下文。
    context: &str,
    // 接收已完成调用的 typed Result，不接管资源 owner。
    result: crate::core::Result<()>,
) {
    // 成功操作不产生额外诊断。
    if let Err(error) = result {
        // 失败在最终责任边界记录一次结构化警告。
        tracing::warn!("{context}: {}", error.short_what());
    }
}

// 根据当前原生窗口事实判断 Agent 是否可以观察首帧。
pub(super) fn initially_agent_presentable(window: &dyn PlatformWindow) -> bool {
    // 读取同一窗口 owner 暴露的当前属性快照。
    let properties = window.properties();
    // 只有非零、非最小化且未遮挡的窗口才可对 Agent 呈现。
    properties.width() > 0
        && properties.height() > 0
        && !properties.is_minimized()
        && window.occlusion_state() != WindowOcclusionState::Occluded
}

// 默认值只组装 Application 的初始服务和状态，不创建原生资源。
impl Default for App {
    // 构造一个尚未进入窗口生命周期的应用组合根。
    fn default() -> Self {
        // 为根窗口建立应用级定时器队列。
        let app_timers = AppTimerQueue::new();
        // 为根窗口建立 owner-thread 工作队列。
        let main_thread_queue = MainThreadQueue::new();
        // 创建由 AppHandle 共享的会话存活事实。
        let handle_alive = Arc::new(AtomicBool::new(true));
        // 创建唯一的应用运行时 owner。
        let runtime = AppRuntime::new();
        // 预登记根窗口会话队列和存活句柄。
        runtime.register_session(
            WindowId::ROOT,
            app_timers.clone(),
            main_thread_queue.clone(),
            handle_alive.clone(),
        );
        // Application 组合根预注册所有窗口都必须拥有的默认语言。
        let mut container = Container::new();
        // 默认语言单例消除主窗和次窗首次解析时的预期失败告警。
        container.singleton(Locale::default());
        // 默认组件配置与语言共享同一应用级生命周期。
        container.singleton(WidgetConfig::default());

        // 把所有初始 owner 和配置收敛到唯一 App 值。
        Self {
            mode: AppMode::GUI,
            title: "UIX App".to_string(),
            size: (800, 600),
            custom_title_bar: false,
            theme: Theme::antd_light(),
            // 为普通 Rust App 保留 light 与 dark 内建名称。
            named_themes: NamedThemes::default(),
            follow_system_theme: false,
            app_state: AppState::new(),
            app_timers,
            main_thread_queue,
            runtime,
            debug_mode_override: None,
            handle_alive,
            root_factory: None,
            on_start: None,
            on_window_start: None,
            on_exit: None,
            cli: None,
            // 保存已经具备框架默认服务的应用容器。
            container,
            settings_path: None,
            graphics_backend: None,
            #[cfg(any(feature = "test-harness", feature = "agent-control"))]
            graphics_faults: GraphicsFaultSignal::default(),
            #[cfg(feature = "agent-control")]
            agent_control_enabled: false,
            agent_policy: AgentPolicy::default(),
            agent_confirm_ui: None,
            exit_code: 0,
        }
    }
}

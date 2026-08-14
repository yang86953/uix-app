//! 次要窗口创建与图形恢复。

use super::*;

pub(super) fn create_secondary_window(
    platform: &mut dyn Platform,
    runtime: &AppRuntime,
    app_state: &AppState,
    container: &Container,
    graphics_backend: GraphicsSelection,
    recovery_request: RebuildRequest,
    request: OpenWindowRequest,
) -> Option<SecondaryWindowSession> {
    let OpenWindowRequest {
        window_id,
        config,
        app_timers,
        main_thread_queue,
        alive,
    } = request;
    let WindowConfig {
        title,
        width,
        height,
        custom_title_bar,
        root,
    } = config;

    let mut platform_window = match platform
        .window_manager()
        .create_window(&title, width, height)
    {
        Ok(window) => window,
        Err(e) => {
            runtime.close_session(window_id);
            tracing::error!("open_window create_window failed: {}", e.short_what());
            return None;
        }
    };
    if platform_window.window_id() != window_id {
        let actual = platform_window.window_id();
        report_window_operation_error(
            "window_id mismatch cleanup close failed",
            platform_window.close(),
        );
        runtime.close_session(window_id);
        tracing::error!(
            "open_window window_id mismatch: reserved={}, native={}",
            window_id.raw(),
            actual.raw()
        );
        return None;
    }

    if custom_title_bar {
        if let Err(error) = configure_custom_title_bar(platform_window.as_mut(), width, height) {
            report_window_operation_error(
                "custom title bar failure cleanup close failed",
                platform_window.close(),
            );
            runtime.close_session(window_id);
            tracing::error!(
                "open_window custom title bar failed: {}",
                error.short_what()
            );
            return None;
        }
    }

    // 次窗与主窗使用同一自动居中错误分类。
    report_center_on_screen_result(
        "secondary center_on_screen failed",
        platform_window.center_on_screen(),
    );
    #[cfg(feature = "test-harness")]
    let graphics_faults = match runtime.graphics_fault_signal(window_id) {
        Some(signal) => signal,
        None => {
            report_window_operation_error(
                "secondary graphics test signal failure cleanup close failed",
                platform_window.close(),
            );
            runtime.close_session(window_id);
            return None;
        }
    };
    #[cfg(feature = "test-harness")]
    let preferred_engine = create_preferred_engine(
        platform_window.as_mut(),
        width,
        height,
        graphics_backend,
        runtime.diagnostics(),
        recovery_request.clone(),
        graphics_faults,
    );
    #[cfg(not(feature = "test-harness"))]
    let preferred_engine = create_preferred_engine(
        platform_window.as_mut(),
        width,
        height,
        graphics_backend,
        runtime.diagnostics(),
        recovery_request.clone(),
    );
    let engine = match preferred_engine {
        Some(engine) => engine,
        None => {
            report_window_operation_error(
                "secondary engine failure cleanup close failed",
                platform_window.close(),
            );
            runtime.close_session(window_id);
            return None;
        }
    };
    // 与主窗一致：首帧 present 成功后再 show，避免空窗白屏。

    // 副窗从 Application 容器取得同一个反馈 owner。
    let feedback = container.resolve_clone::<AppFeedbackState>();
    // DI 未注册 Locale 时回退默认值并记录缺失，避免静默降级（保持回退行为）。
    let locale = match container.resolve_clone::<Locale>() {
        Some(locale) => locale,
        None => {
            tracing::warn!(
                ty = %std::any::type_name::<Locale>(),
                "DI resolve failed, falling back to default"
            );
            Locale::default()
        }
    };
    // DI 未注册 ComponentConfig 时回退默认配置并记录缺失（保持回退行为）。
    let component_config = match container.resolve_clone::<ComponentConfig>() {
        Some(component_config) => component_config,
        None => {
            tracing::warn!(
                ty = %std::any::type_name::<ComponentConfig>(),
                "DI resolve failed, falling back to default"
            );
            ComponentConfig::default()
        }
    };
    let wrapped_root = move || {
        with_config(&component_config, || {
            with_locale(&locale, || {
                let root_node = root();
                // 副窗与主窗复用同一应用根默认值和逐窗反馈浮层组装入口。
                prepare_app_root(root_node, feedback.clone(), window_id)
            })
        })
    };
    let mut session =
        WindowSession::from_root_factory_for_window(window_id, wrapped_root, engine, width, height);
    session.set_text_input_coordinator(runtime.text_input_coordinator());
    // 新建副窗在收到自身原生焦点事件前保持未聚焦。
    session.set_window_focused(false);
    session.set_app_state(app_state.clone());
    session.set_app_timers(app_timers);
    session.set_main_thread_queue(main_thread_queue);
    if let Some(queue) = runtime.agent_command_queue(window_id) {
        session.set_agent_command_queue(queue);
    }
    session.set_agent_command_executor(runtime.agent_command_executor());
    if let Some(registration) = runtime.register_agent_window(
        window_id,
        title,
        platform_window.is_visible(),
        initially_agent_presentable(platform_window.as_ref()),
    ) {
        // bind_agent_window 只返回 bool，无法区分「已绑定」与「窗口已关闭/
        // id 不匹配」等失败原因；改为 Result 会波及全部调用点与签名，
        // 本次仅记录日志，保留弱返回值契约。
        if !session.bind_agent_window(registration) {
            tracing::warn!(window_id = ?window_id, "agent window binding failed");
        }
    }
    let handle = AppHandle::new(
        window_id,
        app_state.clone(),
        runtime.clone(),
        container.clone(),
        alive,
    );

    Some(SecondaryWindowSession {
        session,
        _window: platform_window,
        handle,
        driver: WindowDriver::new(width, height, true),
        last_frame: None,
    })
}

pub(super) fn recreate_exact_graphics_recipe(
    surface: NativeSurfaceHandle,
    width: i32,
    height: i32,
    recipe: GraphicsRecipe,
    pending_failures: &PendingFailureQueue,
) -> Result<Box<dyn RenderTarget>, Error> {
    // native factory 在返回前已经把兼容 context 收敛为 recipe owner。
    let owner =
        try_create_gpu_recipe_with_queue(recipe, surface, width, height, pending_failures.clone())?;
    // 恢复路径与首次 bootstrap 复用同一个 owner 装配入口。
    assemble_renderer(owner, width, height)
        .map(|renderer| Box::new(renderer) as Box<dyn RenderTarget>)
        .map_err(|failure| failure.into_error())
}

pub(super) fn create_software_recovery_engine(
    width: i32,
    height: i32,
) -> Result<Box<dyn RenderTarget>, Error> {
    let mut renderer = Renderer::cpu();
    renderer.initialize(width, height)?;
    Ok(Box::new(renderer))
}

pub(crate) fn graphics_recovery_rebuilder_with_pending(
    surface: NativeSurfaceHandle,
    requested: GraphicsSelection,
    selected_recipe: GraphicsRecipe,
    pending_failures: PendingFailureQueue,
) -> RenderTargetRebuilder {
    let candidates = gpu_recipe_candidates(requested);
    let mut current_recipe = selected_recipe;
    Box::new(move |action, width, height| match action {
        GraphicsRecoveryAction::RebuildSurface | GraphicsRecoveryAction::RebuildRecipe => {
            recreate_exact_graphics_recipe(
                surface,
                width,
                height,
                current_recipe,
                &pending_failures,
            )
        }
        GraphicsRecoveryAction::TryNextRecipe => {
            let start = candidates
                .iter()
                .position(|recipe| *recipe == current_recipe)
                .map(|index| index + 1)
                .unwrap_or(0);
            let mut last_error = Error::new(
                Errc::PlatformError,
                format!("graphics recovery: no next recipe after {current_recipe}"),
            );
            for candidate in candidates.iter().copied().skip(start) {
                match recreate_exact_graphics_recipe(
                    surface,
                    width,
                    height,
                    candidate,
                    &pending_failures,
                ) {
                    Ok(engine) => {
                        current_recipe = candidate;
                        return Ok(engine);
                    }
                    Err(error) => last_error = error,
                }
            }
            Err(last_error)
        }
        GraphicsRecoveryAction::UseSoftware => create_software_recovery_engine(width, height),
        GraphicsRecoveryAction::Abort | GraphicsRecoveryAction::AbortOutOfMemory => {
            Err(Error::new(
                Errc::InvalidState,
                format!("graphics recovery: forbidden action {action:?}"),
            ))
        }
    })
}

pub(crate) fn create_preferred_engine(
    platform_window: &mut dyn PlatformWindow,
    width: i32,
    height: i32,
    graphics_backend: GraphicsSelection,
    diagnostics: Diagnostics,
    recovery_request: RebuildRequest,
    #[cfg(feature = "test-harness")] graphics_faults: GraphicsFaultSignal,
) -> Option<Box<dyn RenderTarget>> {
    // SAFETY: `PlatformWindow` 在同步窗口会话全程拥有该 surface；图形启动与恢复
    // 均在同一事件循环线程执行，且 `NativeSurfaceHandle` 是 !Send + !Sync。
    let surface = unsafe { NativeSurfaceHandle::from_raw(platform_window.native_surface_ptr()) };
    let pending_failures = diagnostics.pending_failure_queue();
    match bootstrap_renderer_with_pending(
        surface,
        width,
        height,
        graphics_backend,
        pending_failures.clone(),
    ) {
        Ok(gpu) => {
            if gpu.report.failures.is_empty() {
                tracing::info!("GPU renderer initialized ({})", gpu.selected);
            } else {
                tracing::warn!(
                    "GPU renderer initialized after probe fallback; selected={}; failures=[{}]",
                    gpu.selected,
                    format_probe_failures(&gpu.report)
                );
            }
            let engine = RecoveryDriver::new(
                Box::new(gpu.renderer),
                graphics_recovery_rebuilder_with_pending(
                    surface,
                    graphics_backend,
                    gpu.selected_recipe,
                    pending_failures,
                ),
            )
            .with_extent(width, height)
            .with_rebuild_request(recovery_request);
            #[cfg(feature = "test-harness")]
            let engine = engine.with_test_fault_signal(graphics_faults);
            Some(Box::new(engine))
        }
        Err(report) => {
            tracing::warn!("{}", format_gpu_probe_fallback(graphics_backend, &report));

            let mut renderer = Renderer::cpu();
            match renderer.initialize(width, height) {
                Ok(()) => {
                    tracing::info!("CPU renderer initialized");
                    Some(Box::new(renderer))
                }
                Err(e) => {
                    let _ = renderer.try_shutdown();
                    tracing::error!("CPU Renderer 初始化失败: {}", e.what());
                    None
                }
            }
        }
    }
}

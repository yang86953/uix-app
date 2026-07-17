//! 应用运行时辅助：图形选择、副窗编排与平台事件映射。

use super::*;

pub(crate) fn resolve_graphics_backend(
    builder: Option<GraphicsBackend>,
    env_value: Option<&str>,
    settings: Option<&SettingsService>,
) -> GraphicsBackend {
    if let Some(backend) = builder {
        return backend;
    }

    if let Some(value) = env_value {
        if let Some(backend) = parse_graphics_backend_config(GRAPHICS_BACKEND_ENV, value) {
            return backend;
        }
    }

    if let Some(settings) = settings {
        for key in GRAPHICS_BACKEND_SETTING_KEYS {
            if let Some(value) = settings.get(key) {
                if let Some(backend) = parse_graphics_backend_config(key, &value) {
                    return backend;
                }
            }
        }
    }

    GraphicsBackend::Vulkan
}

fn parse_graphics_backend_config(source: &str, value: &str) -> Option<GraphicsBackend> {
    match value.parse::<GraphicsBackend>() {
        Ok(backend) => Some(backend),
        Err(err) => {
            crate::core::log::warn_fn(format!(
                "graphics backend config {source} ignored: {}",
                err.short_what()
            ));
            None
        }
    }
}

#[cfg(test)]
pub(crate) fn drain_pending_open_windows(
    platform: &mut dyn Platform,
    runtime: &AppRuntime,
    app_state: &AppState,
    container: &Container,
    on_window_start: Option<&Arc<dyn Fn(AppHandle) + Send + Sync>>,
    secondary_windows: &mut Vec<SecondaryWindowSession>,
) -> usize {
    drain_pending_open_windows_with_backend(
        platform,
        runtime,
        app_state,
        container,
        GraphicsBackend::Auto,
        on_window_start,
        secondary_windows,
    )
}

pub(crate) fn drain_pending_open_windows_with_backend(
    platform: &mut dyn Platform,
    runtime: &AppRuntime,
    app_state: &AppState,
    container: &Container,
    graphics_backend: GraphicsBackend,
    on_window_start: Option<&Arc<dyn Fn(AppHandle) + Send + Sync>>,
    secondary_windows: &mut Vec<SecondaryWindowSession>,
) -> usize {
    let mut created = 0;
    while let Some(request) = runtime.take_next_open_window() {
        if let Some(mut window) = create_secondary_window(
            platform,
            runtime,
            app_state,
            container,
            graphics_backend,
            request,
        ) {
            if let Some(callback) = on_window_start {
                callback(window.handle.clone());
            }
            window.drain_main_thread_work();
            secondary_windows.push(window);
            created += 1;
        }
    }
    created
}

pub(crate) fn drain_secondary_window_queues(
    secondary_windows: &mut [SecondaryWindowSession],
) -> bool {
    let mut drained = false;
    for window in secondary_windows {
        drained |= window.drain_main_thread_work();
    }
    drained
}

#[cfg(test)]
pub(crate) fn drain_secondary_window_frames(
    secondary_windows: &mut [SecondaryWindowSession],
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    debug_mode: &Cell<bool>,
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

pub(super) fn drain_secondary_window_frames_with_platform(
    platform: &mut dyn Platform,
    secondary_windows: &mut [SecondaryWindowSession],
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    debug_mode: &Cell<bool>,
    cursor_pos: &Cell<Point>,
    clock: &dyn AppClock,
) -> bool {
    drain_secondary_window_frames_impl(
        Some(platform),
        secondary_windows,
        font_service,
        image_service,
        theme,
        debug_mode,
        cursor_pos,
        clock,
    )
}

fn drain_secondary_window_frames_impl(
    platform: Option<&mut dyn Platform>,
    secondary_windows: &mut [SecondaryWindowSession],
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    debug_mode: &Cell<bool>,
    cursor_pos: &Cell<Point>,
    clock: &dyn AppClock,
) -> bool {
    let mut drained = false;
    let now = clock.now();
    match platform {
        Some(platform) => {
            for window in secondary_windows {
                if window.has_frame_work(now) {
                    drained |= window.drain_frame(
                        font_service,
                        image_service,
                        theme,
                        debug_mode,
                        cursor_pos,
                        clock,
                        Some(&mut *platform),
                    );
                }
            }
        }
        None => {
            for window in secondary_windows {
                if window.has_frame_work(now) {
                    drained |= window.drain_frame(
                        font_service,
                        image_service,
                        theme,
                        debug_mode,
                        cursor_pos,
                        clock,
                        None,
                    );
                }
            }
        }
    }
    drained
}

pub(crate) fn secondary_windows_next_deadline(
    secondary_windows: &mut [SecondaryWindowSession],
) -> Option<Instant> {
    secondary_windows
        .iter_mut()
        .filter_map(SecondaryWindowSession::next_deadline)
        .min()
}

pub(crate) fn dispatch_secondary_window_event(
    secondary_windows: &mut Vec<SecondaryWindowSession>,
    platform: &mut dyn Platform,
    event: &UiEvent,
) -> bool {
    let Some(window_id) = event.window_id else {
        return false;
    };
    let Some(index) = secondary_windows
        .iter()
        .position(|window| window.window_id() == window_id)
    else {
        return false;
    };

    if secondary_windows[index].handle_event(platform, event) {
        true
    } else {
        secondary_windows.remove(index).close();
        true
    }
}

pub(crate) fn dispatch_secondary_system_theme_changed(
    secondary_windows: &mut [SecondaryWindowSession],
    is_dark: bool,
) -> bool {
    let mut dispatched = false;
    for window in secondary_windows {
        window
            .session
            .parts_mut()
            .tree
            .dispatch_event(&SystemEvent::ThemeChanged { is_dark });
        dispatched = true;
    }
    dispatched
}

pub(crate) fn apply_runtime_theme_change(
    theme: &RefCell<Theme>,
    root_tree: &mut WidgetTree,
    secondary_windows: &mut [SecondaryWindowSession],
    next_theme: Theme,
) {
    let is_dark = next_theme.is_dark();
    *theme.borrow_mut() = next_theme;
    root_tree.dispatch_event(&SystemEvent::ThemeChanged { is_dark });
    dispatch_secondary_system_theme_changed(secondary_windows, is_dark);
}

fn create_secondary_window(
    platform: &mut dyn Platform,
    runtime: &AppRuntime,
    app_state: &AppState,
    container: &Container,
    graphics_backend: GraphicsBackend,
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
            crate::core::log::error_fn(format!(
                "open_window create_window failed: {}",
                e.short_what()
            ));
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
        crate::core::log::error_fn(format!(
            "open_window window_id mismatch: reserved={}, native={}",
            window_id.raw(),
            actual.raw()
        ));
        return None;
    }

    if custom_title_bar {
        if let Err(error) = configure_custom_title_bar(platform_window.as_mut(), width, height) {
            report_window_operation_error(
                "custom title bar failure cleanup close failed",
                platform_window.close(),
            );
            runtime.close_session(window_id);
            crate::core::log::error_fn(format!(
                "open_window custom title bar failed: {}",
                error.short_what()
            ));
            return None;
        }
    }

    report_window_operation_error(
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
        graphics_faults,
    );
    #[cfg(not(feature = "test-harness"))]
    let preferred_engine =
        create_preferred_engine(platform_window.as_mut(), width, height, graphics_backend);
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

    let notifications = container.resolve_clone::<AppNotificationState>();
    let locale = container.resolve_clone::<Locale>().unwrap_or_default();
    let component_config = container
        .resolve_clone::<ComponentConfig>()
        .unwrap_or_default();
    let wrapped_root = move || {
        with_config(&component_config, || {
            with_locale(&locale, || {
                let root_node = root();
                match notifications.clone() {
                    Some(state) => wrap_root_with_notification_overlay(root_node, state, window_id),
                    None => root_node,
                }
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
    if let Some(registration) = runtime.register_agent_window(
        window_id,
        title,
        platform_window.is_visible(),
        initially_agent_presentable(platform_window.as_ref()),
    ) {
        let _ = session.bind_agent_window(registration);
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

fn recreate_exact_graphics_recipe(
    surface: NativeSurfaceHandle,
    width: i32,
    height: i32,
    recipe: GraphicsRecipe,
) -> Result<Box<dyn GraphicsEngine>, Error> {
    let context = try_create_gpu_recipe(recipe, surface, width, height)?;
    assemble_graphics_engine(context, width, height).map_err(|failure| failure.into_error())
}

fn create_software_recovery_engine(
    width: i32,
    height: i32,
) -> Result<Box<dyn GraphicsEngine>, Error> {
    let mut engine = SoftwareEngine::new();
    engine.initialize(width, height)?;
    Ok(Box::new(engine))
}

pub(crate) fn graphics_recovery_rebuilder(
    surface: NativeSurfaceHandle,
    requested: GraphicsBackend,
    selected_recipe: GraphicsRecipe,
) -> GraphicsEngineRebuilder {
    let candidates = gpu_recipe_candidates(requested);
    let mut current_recipe = selected_recipe;
    Box::new(move |action, width, height| match action {
        RecoveryAction::RebuildSurface | RecoveryAction::RebuildRecipe => {
            recreate_exact_graphics_recipe(surface, width, height, current_recipe)
        }
        RecoveryAction::TryNextRecipe => {
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
                match recreate_exact_graphics_recipe(surface, width, height, candidate) {
                    Ok(engine) => {
                        current_recipe = candidate;
                        return Ok(engine);
                    }
                    Err(error) => last_error = error,
                }
            }
            Err(last_error)
        }
        RecoveryAction::UseSoftware => create_software_recovery_engine(width, height),
        RecoveryAction::Abort | RecoveryAction::AbortOutOfMemory => Err(Error::new(
            Errc::InvalidState,
            format!("graphics recovery: forbidden action {action:?}"),
        )),
    })
}

pub(super) fn create_preferred_engine(
    platform_window: &mut dyn PlatformWindow,
    width: i32,
    height: i32,
    graphics_backend: GraphicsBackend,
    #[cfg(feature = "test-harness")] graphics_faults: GraphicsFaultSignal,
) -> Option<Box<dyn GraphicsEngine>> {
    // SAFETY: `PlatformWindow` 在同步窗口会话全程拥有该 surface；图形启动与恢复
    // 均在同一事件循环线程执行，且 `NativeSurfaceHandle` 是 !Send + !Sync。
    let surface = unsafe { NativeSurfaceHandle::from_raw(platform_window.native_surface_ptr()) };
    match bootstrap_graphics_engine(surface, width, height, graphics_backend) {
        Ok(gpu) => {
            if gpu.report.failures.is_empty() {
                crate::core::log::info_fn(format!("GPU engine initialized ({})", gpu.selected));
            } else {
                crate::core::log::warn_fn(format!(
                    "GPU engine initialized after probe fallback; selected={}; failures=[{}]",
                    gpu.selected,
                    format_probe_failures(&gpu.report)
                ));
            }
            let engine = RecoveringGraphicsEngine::new(
                gpu.engine,
                graphics_recovery_rebuilder(surface, graphics_backend, gpu.selected_recipe),
            )
            .with_extent(width, height);
            #[cfg(feature = "test-harness")]
            let engine = engine.with_test_fault_signal(graphics_faults);
            Some(Box::new(engine))
        }
        Err(report) => {
            crate::core::log::warn_fn(format_gpu_probe_fallback(graphics_backend, &report));

            let mut engine = SoftwareEngine::new();
            match engine.initialize(width, height) {
                Ok(()) => {
                    crate::core::log::info_fn("CPU software engine initialized");
                    Some(Box::new(engine))
                }
                Err(e) => {
                    let _ = engine.try_shutdown();
                    crate::core::log::error_fn(format!("SoftwareEngine 初始化失败: {}", e.what()));
                    None
                }
            }
        }
    }
}

fn format_probe_failures(report: &ProbeReport) -> String {
    if report.failures.is_empty() {
        return "none".to_string();
    }

    report
        .failures
        .iter()
        .enumerate()
        .map(|(index, failure)| {
            format!(
                "failure[{index}]={{candidate={}, detail={:?}}}",
                failure.backend, failure.message
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub(crate) fn format_gpu_probe_fallback(request: GraphicsBackend, report: &ProbeReport) -> String {
    format!(
        "GPU probe exhausted; request={request}; platform={}; fallback=software_cpu; failures=[{}]",
        graphics_runtime_platform(),
        format_probe_failures(report)
    )
}

// ════════════════════════════════════════════════════════════════════════════
// UiEvent → SystemEvent 映射（唯一实现）
// ════════════════════════════════════════════════════════════════════════════

/// 将平台 `UiEvent` 转换为 `SystemEvent`。
pub fn map_ui_event(ev: &UiEvent) -> Option<SystemEvent> {
    match ev.type_ {
        UiEventType::PointerDown => {
            if let UiEventPayload::PointerButton(ref d) = ev.payload {
                Some(SystemEvent::PointerDown {
                    pos: d.pos,
                    button: d.btn,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::PointerDoubleClick => {
            if let UiEventPayload::PointerButton(ref d) = ev.payload {
                Some(SystemEvent::PointerDoubleClick {
                    pos: d.pos,
                    button: d.btn,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::PointerUp => {
            if let UiEventPayload::PointerButton(ref d) = ev.payload {
                Some(SystemEvent::PointerUp {
                    pos: d.pos,
                    button: d.btn,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::PointerMove => {
            if let UiEventPayload::PointerMove(ref d) = ev.payload {
                Some(SystemEvent::PointerMove {
                    pos: d.pos,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::Wheel => {
            if let UiEventPayload::Wheel(ref d) = ev.payload {
                Some(SystemEvent::Wheel {
                    pos: d.pos,
                    delta: Point::new(d.delta_x, d.delta_y),
                })
            } else {
                None
            }
        }
        UiEventType::KeyDown => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(SystemEvent::KeyDown {
                    key: d.key,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::KeyUp => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(SystemEvent::KeyUp {
                    key: d.key,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::Copy => Some(SystemEvent::Copy),
        UiEventType::Cut => Some(SystemEvent::Cut),
        UiEventType::Paste => {
            if let UiEventPayload::Clipboard(ref d) = ev.payload {
                Some(SystemEvent::Paste {
                    text: d.text.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::TextInput => {
            if let UiEventPayload::TextInput(ref d) = ev.payload {
                Some(SystemEvent::TextInput {
                    text: d.text.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::ImeCompositionStart => Some(SystemEvent::ImeCompositionStart),
        UiEventType::ImeCompositionUpdate => {
            if let UiEventPayload::ImeComposition(ref d) = ev.payload {
                Some(SystemEvent::ImeCompositionUpdate {
                    text: d.text.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::ImeCompositionEnd => {
            if let UiEventPayload::ImeComposition(ref d) = ev.payload {
                Some(SystemEvent::ImeCompositionEnd {
                    text: d.text.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::ThemeChanged => {
            if let UiEventPayload::ThemeChanged(ref d) = ev.payload {
                Some(SystemEvent::ThemeChanged { is_dark: d.is_dark })
            } else {
                None
            }
        }
        UiEventType::LocaleChanged => {
            if let UiEventPayload::LocaleChanged(ref d) = ev.payload {
                Some(SystemEvent::LocaleChanged {
                    locale: d.locale.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::WindowResize => {
            if let UiEventPayload::Resize(ref d) = ev.payload {
                Some(SystemEvent::Resize {
                    width: d.width as f32,
                    height: d.height as f32,
                })
            } else {
                None
            }
        }
        UiEventType::WindowMaximize => Some(SystemEvent::WindowMaximize),
        UiEventType::WindowMinimize => Some(SystemEvent::WindowMinimize),
        UiEventType::WindowRestore => Some(SystemEvent::WindowRestore),
        UiEventType::WindowFocus => Some(SystemEvent::WindowFocus),
        UiEventType::WindowBlur => Some(SystemEvent::WindowBlur),
        UiEventType::Timer => {
            if let UiEventPayload::Timer(ref d) = ev.payload {
                Some(SystemEvent::Timer { id: d.timer_id })
            } else {
                None
            }
        }
        UiEventType::FileDrop => {
            if let UiEventPayload::FileDrop(ref d) = ev.payload {
                Some(SystemEvent::FileDrop {
                    files: d.files.clone(),
                    position: d.position,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

//! 应用运行时辅助：图形选择、副窗编排与平台事件映射。

use super::*;
use crate::diagnostics::{Diagnostics, PendingFailureQueue};
use crate::draw::renderer::{GraphicsRecoveryAction, RebuildRequest};

/// 在应用 owner 线程边界排空原生回调失败。
///
/// 原生回调只能入队类型化错误。本函数特意由应用循环调用：只有在这里才允许
/// 执行上报或未来的领域恢复动作。已注册的恢复处理器先在此安全点运行，只有
/// 仍未处理的错误才进入最终上报，符合「报告不能恢复的」运行时保证。
pub(crate) fn drain_platform_pending_failures(
    platform: &mut dyn PlatformSystem,
    diagnostics: &Diagnostics,
) -> usize {
    let mut drained = 0;
    while let Some(error) = platform.take_pending_failure() {
        if let Some(unresolved) = diagnostics.attempt_recovery(error).into_error() {
            diagnostics.report(unresolved);
        }
        drained += 1;
    }
    drained
}

pub(crate) fn resolve_graphics_backend(
    builder: Option<GraphicsApi>,
    env_value: Option<&str>,
    settings: Option<&SettingsService>,
) -> GraphicsSelection {
    if let Some(backend) = builder {
        // 公开 builder 只产生显式具体 API 选择。
        return GraphicsSelection::Explicit(backend);
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

    // 未配置时才启用私有自动策略。
    GraphicsSelection::Automatic
}

// 让图形启动策略解析靠近它消费的环境变量、设置与优先级规则。
impl App {
    /// 按 builder、环境变量与设置的优先级解析私有启动策略。
    pub(crate) fn configured_graphics_backend(&self) -> GraphicsSelection {
        // 环境变量只在 builder 未显式选择时参与策略解析。
        let env_value = std::env::var(GRAPHICS_BACKEND_ENV).ok();
        // 把三种配置来源交给唯一优先级函数。
        resolve_graphics_backend(
            // 公开 builder 拥有最高优先级。
            self.graphics_backend,
            // 环境值保持借用直到本次解析结束。
            env_value.as_deref(),
            // 设置服务不存在时自然进入自动策略。
            self.container.resolve::<SettingsService>(),
        )
    }
}

fn parse_graphics_backend_config(source: &str, value: &str) -> Option<GraphicsSelection> {
    // 环境变量与设置值共享同一个私有策略解析器。
    match value.parse::<GraphicsSelection>() {
        Ok(backend) => Some(backend),
        Err(err) => {
            tracing::warn!(
                "graphics backend config {source} ignored: {}",
                err.short_what()
            );
            None
        }
    }
}

// 测试目标保留默认图形后端的 pending-window 便捷入口，供外部 GUI 测试按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(test)]
pub(crate) fn drain_pending_open_windows(
    platform: &mut dyn PlatformSystem,
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
        GraphicsSelection::Automatic,
        RebuildRequest::default(),
        on_window_start,
        secondary_windows,
    )
}

pub(crate) fn drain_pending_open_windows_with_backend(
    platform: &mut dyn PlatformSystem,
    runtime: &AppRuntime,
    app_state: &AppState,
    container: &Container,
    graphics_backend: GraphicsSelection,
    recovery_request: RebuildRequest,
    on_window_start: Option<&Arc<dyn Fn(AppHandle) + Send + Sync>>,
    secondary_windows: &mut Vec<SecondaryWindowSession>,
) -> usize {
    let mut created = 0;
    // 逐个取出待打开的副窗请求，创建成功后才登记到会话列表。
    while let Some(request) = runtime.take_next_open_window() {
        // 新窗口队列由紧随其后的唯一 WindowDriver 轮次按预算消费。
        if let Some(window) = create_secondary_window(
            platform,
            runtime,
            app_state,
            container,
            graphics_backend,
            recovery_request.clone(),
            request,
        ) {
            // 创建成功后回调通知外部（如测试）拿到窗口句柄。
            if let Some(callback) = on_window_start {
                callback(window.handle.clone());
            }
            secondary_windows.push(window);
            created += 1;
        }
    }
    created
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

#[allow(
    clippy::too_many_arguments,
    reason = "secondary window draining mirrors the application service boundary"
)]
pub(super) fn drain_secondary_window_frames_with_platform(
    platform: &mut dyn PlatformSystem,
    secondary_windows: &mut [SecondaryWindowSession],
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    debug_mode: &Diagnostics,
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

#[allow(
    clippy::too_many_arguments,
    reason = "secondary window draining mirrors the application service boundary"
)]
fn drain_secondary_window_frames_impl(
    platform: Option<&mut dyn PlatformSystem>,
    secondary_windows: &mut [SecondaryWindowSession],
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    debug_mode: &Diagnostics,
    cursor_pos: &Cell<Point>,
    clock: &dyn AppClock,
) -> bool {
    let mut drained = false;
    let now = clock.now();
    // 有平台句柄与无平台句柄共用同一帧排空逻辑，仅是否携带平台上下文不同。
    match platform {
        Some(platform) => {
            for window in secondary_windows {
                // 仅当窗口在当前时刻确有帧工作时才驱动一帧。
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
    platform: &mut dyn PlatformSystem,
    event: &UiEvent,
) -> bool {
    // 事件未标注窗口或目标窗口已不在会话列表时直接放弃。
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
        // 窗口自身判定事件已不可继续处理（如关闭请求），摘除并释放会话。
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
    // 先落盘主题值，再向主窗口树与所有副窗广播系统主题变更事件。
    *theme.borrow_mut() = next_theme;
    root_tree.dispatch_event(&SystemEvent::ThemeChanged { is_dark });
    dispatch_secondary_system_theme_changed(secondary_windows, is_dark);
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
            // 空候选报告使用 none，不能把自动策略伪装成设备 API。
            let candidate = failure
                .candidate
                .map(|api| api.to_string())
                .unwrap_or_else(|| "none".to_string());
            // 保留稳定的失败序号、候选身份与阶段细节。
            format!(
                "failure[{index}]={{candidate={}, detail={:?}}}",
                candidate, failure.message
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub(crate) fn format_gpu_probe_fallback(
    request: GraphicsSelection,
    report: &ProbeReport,
) -> String {
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
///
/// 每个分支校验事件携带的载荷类型与事件类型一致；载荷缺失或类型不符时
/// 返回 `None`（事件被丢弃，不产生系统事件）。
pub fn map_ui_event(ev: &UiEvent) -> Option<SystemEvent> {
    match ev.type_ {
        // ── 指针类事件：按下/双击/抬起携带位置、按键与修饰键 ──
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
        // 指针移动不携带按键信息，仅传递位置与修饰键。
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
        // 滚轮事件：把分开的 delta 分量合并为 Point。
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
        // ── 键盘事件：按下/抬起携带按键码与修饰键 ──
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
        // ── 剪贴板命令：复制/剪切无载荷，粘贴携带剪贴板文本 ──
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
        // ── 文本输入：直接文本与 IME 组合过程 ──
        UiEventType::TextInput => {
            if let UiEventPayload::TextInput(ref d) = ev.payload {
                Some(SystemEvent::TextInput {
                    text: d.text.clone(),
                })
            } else {
                None
            }
        }
        // IME 组合开始无载荷；更新与结束均携带当前组合文本。
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
        // ── 环境变更：主题深浅与区域设置 ──
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
        // ── 窗口状态：尺寸（像素转逻辑坐标）与最大化/最小化等 ──
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
        // 最大化/最小化/恢复/焦点得失均为无载荷信号，直接透传。
        UiEventType::WindowMaximize => Some(SystemEvent::WindowMaximize),
        UiEventType::WindowMinimize => Some(SystemEvent::WindowMinimize),
        UiEventType::WindowRestore => Some(SystemEvent::WindowRestore),
        UiEventType::WindowFocus => Some(SystemEvent::WindowFocus),
        UiEventType::WindowBlur => Some(SystemEvent::WindowBlur),
        // 计时器回调：透传 timer id 供应用定位到期定时器。
        UiEventType::Timer => {
            if let UiEventPayload::Timer(ref d) = ev.payload {
                Some(SystemEvent::Timer { id: d.timer_id })
            } else {
                None
            }
        }
        // 文件拖放：携带文件列表与落点坐标。
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
        // 其余未知事件类型统一丢弃，避免向系统事件空间泄漏。
        _ => None,
    }
}

mod create;

pub(crate) use self::create::*;

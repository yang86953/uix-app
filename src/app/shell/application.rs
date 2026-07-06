//! 应用入口 — 统一 GUI / CLI 生命周期。

use std::cell::{Cell, RefCell};
use std::sync::{atomic::AtomicBool, Arc};
use std::time::Duration;

use crate::app::app_handle::{AppHandle, WindowId};
use crate::app::app_timer::{AppTimerQueue, TimerHandle};
use crate::app::event_loop::run_window_session_loop_with_system_theme;
use crate::app::main_thread_queue::MainThreadQueue;
use crate::app::shell::cli::Cli;
use crate::app::shell::di::Container;
use crate::app::window_session::WindowSession;
use crate::core::Point;
use crate::data::SettingsService;
use crate::draw::font::font_service::FontService;
use crate::draw::image::ImageService;
use crate::draw::traits::GraphicsEngine;
use crate::draw::GpuEngine;
use crate::draw::SoftwareEngine;
use crate::native::traits::event::{UiEvent, UiEventPayload, UiEventType};
use crate::native::traits::window::PlatformWindow;
use crate::native::{create_gpu_context, create_platform};
use crate::ui::theme::{DesignTokens, DynTokens, Theme};
use crate::ui::traits::TokenProvider;
use crate::ui::view::ViewNode;
use crate::ui::{AppState, SystemEvent};

// ════════════════════════════════════════════════════════════════════════════
// 应用模式
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    #[default]
    GUI,
    CLI,
}

// ════════════════════════════════════════════════════════════════════════════
// App — 统一应用入口
// ════════════════════════════════════════════════════════════════════════════

/// 应用入口：GUI（View 根节点）或 CLI 模式。
pub struct App {
    mode: AppMode,
    title: String,
    size: (i32, i32),
    theme: Theme,
    follow_system_theme: bool,
    app_state: AppState,
    app_timers: AppTimerQueue,
    main_thread_queue: MainThreadQueue,
    handle_alive: Arc<AtomicBool>,
    root_factory: Option<Arc<dyn Fn() -> ViewNode + Send + Sync>>,
    on_start: Option<Box<dyn FnOnce(AppHandle) + Send>>,
    on_exit: Option<Box<dyn Fn(&UiEvent) -> bool>>,
    cli: Option<Cli>,
    container: Container,
    settings_path: Option<String>,
    exit_code: i32,
}

impl Default for App {
    fn default() -> Self {
        Self {
            mode: AppMode::GUI,
            title: "UIX App".to_string(),
            size: (800, 600),
            theme: Theme::antd_light(),
            follow_system_theme: false,
            app_state: AppState::new(),
            app_timers: AppTimerQueue::new(),
            main_thread_queue: MainThreadQueue::new(),
            handle_alive: Arc::new(AtomicBool::new(true)),
            root_factory: None,
            on_start: None,
            on_exit: None,
            cli: None,
            container: Container::new(),
            settings_path: None,
            exit_code: 0,
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置窗口标题。
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// 设置窗口初始尺寸。
    pub fn size(mut self, width: i32, height: i32) -> Self {
        self.size = (width, height);
        self
    }

    /// 设置主题（GUI + root 时生效）。
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// 设置是否在运行中跟随 OS 主题变化（默认 false）。
    pub fn follow_system_theme(mut self, follow: bool) -> Self {
        self.follow_system_theme = follow;
        self
    }

    /// 延迟一次执行 App 级回调；回调在主循环线程执行。
    pub fn run_after<F>(&self, delay: Duration, f: F) -> TimerHandle
    where
        F: FnOnce() + Send + 'static,
    {
        self.app_timers.run_after(delay, f)
    }

    /// 按固定间隔重复执行 App 级回调；`TimerHandle` drop/cancel 后停止。
    pub fn run_interval<F>(&self, interval: Duration, f: F) -> TimerHandle
    where
        F: FnMut() + Send + 'static,
    {
        self.app_timers.run_interval(interval, f)
    }

    /// 投递一次主线程回调；回调在当前窗口 session 的帧内 drain。
    pub fn post_to_ui<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.main_thread_queue.enqueue(f);
    }

    pub fn on_start<F>(mut self, f: F) -> Self
    where
        F: FnOnce(AppHandle) + Send + 'static,
    {
        self.on_start = Some(Box::new(f));
        self
    }

    /// 设置根 View（GUI 模式必需）。
    pub fn root<F>(mut self, build_root: F) -> Self
    where
        F: Fn() -> ViewNode + Send + Sync + 'static,
    {
        self.root_factory = Some(Arc::new(build_root));
        self
    }

    /// 退出条件：返回 `true` 时结束事件循环。
    pub fn on_exit<F: Fn(&UiEvent) -> bool + 'static>(mut self, f: F) -> Self {
        self.on_exit = Some(Box::new(f));
        self
    }

    /// 设置应用模式。
    pub fn mode(mut self, m: AppMode) -> Self {
        self.mode = m;
        self
    }

    /// 注册 CLI 命令处理器。
    pub fn cli(mut self, cli: Cli) -> Self {
        self.cli = Some(cli);
        self
    }

    /// Opt in to loading SettingsService once before `run()` enters its mode.
    pub fn settings(mut self, path: impl Into<String>) -> Self {
        self.settings_path = Some(path.into());
        self
    }

    pub(crate) fn app_handle(&self) -> AppHandle {
        AppHandle::new(
            WindowId::root(),
            self.app_state.clone(),
            self.app_timers.clone(),
            self.main_thread_queue.clone(),
            self.handle_alive.clone(),
        )
    }

    /// 注册全局单例。
    pub fn singleton<T: 'static + Send + Clone>(mut self, instance: T) -> Self {
        self.container.singleton(instance);
        self
    }

    // ── 查询 ──────────────────────────────────────────────────────

    pub fn current_mode(&self) -> AppMode {
        self.mode
    }

    pub fn window_title(&self) -> &str {
        &self.title
    }

    pub fn window_size(&self) -> (i32, i32) {
        self.size
    }

    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }

    pub fn container(&self) -> &Container {
        &self.container
    }

    pub fn app_state(&self) -> AppState {
        self.app_state.clone()
    }

    // ── 运行 ──────────────────────────────────────────────────────

    pub fn run(mut self) -> i32 {
        if !self.load_configured_settings() {
            return self.exit_code;
        }
        match self.mode {
            AppMode::CLI => self.run_cli(),
            AppMode::GUI => self.run_gui(),
        }
    }

    fn load_configured_settings(&mut self) -> bool {
        let Some(path) = self.settings_path.as_deref() else {
            return true;
        };

        let mut settings = SettingsService::new();
        if let Err(err) = settings.load(path) {
            crate::core::log::error_fn(format!("load settings failed: {}", err.short_what()));
            self.exit_code = 1;
            return false;
        }

        self.container.singleton(settings);
        true
    }

    fn run_cli(&mut self) -> i32 {
        if let Some(ref mut cli) = self.cli {
            let args = std::env::args().collect::<Vec<_>>();
            self.exit_code = cli.run(&args);
        } else {
            crate::core::log::info_fn("Running in CLI mode (no commands registered)");
        }
        self.exit_code
    }

    fn run_gui(mut self) -> i32 {
        let root_factory = match self.root_factory.take() {
            Some(factory) => factory,
            None => {
                crate::core::log::error_fn("GUI 模式须调用 .root() 设置根 View");
                return 1;
            }
        };

        let (w, h) = self.size;

        let mut platform = match create_platform() {
            Ok(p) => p,
            Err(e) => {
                crate::core::log::error_fn(format!("create_platform 失败: {:?}", e));
                return 1;
            }
        };

        let mut platform_window = match platform.window_manager().create_window(&self.title, w, h) {
            Ok(win) => win,
            Err(e) => {
                crate::core::log::error_fn(format!("create_window 失败: {:?}", e));
                return 1;
            }
        };
        platform_window.center_on_screen();
        platform_window.show();
        platform_window.raise();

        let engine = match create_preferred_engine(platform_window.as_mut(), w, h) {
            Some(engine) => engine,
            None => return 1,
        };

        let mut font_service = FontService::new();
        font_service.load_default_system_font(14.0, platform.system_info());
        let image_service = ImageService::new();

        let system_theme_tokens = if self.follow_system_theme {
            let tokens = Arc::new(DynTokens::new(if platform.display().is_dark_mode() {
                DesignTokens::antd_dark()
            } else {
                DesignTokens::antd_light()
            }));
            let provider: Arc<dyn TokenProvider> = tokens.clone();
            self.theme = Theme::from_arc(provider);
            Some(tokens)
        } else {
            None
        };

        let mut session = WindowSession::from_root_factory(move || root_factory(), engine, w, h);
        session.set_app_state(self.app_state.clone());
        session.set_app_timers(self.app_timers.clone());
        session.set_main_thread_queue(self.main_thread_queue.clone());
        self.handle_alive
            .store(true, std::sync::atomic::Ordering::Release);
        let app_handle = self.app_handle();
        if let Some(on_start) = self.on_start.take() {
            on_start(app_handle.clone());
        }

        let theme = RefCell::new(self.theme);
        let debug_mode = Cell::new(false);
        let cursor_pos = Cell::new(Point::new(0.0, 0.0));
        let on_exit = self
            .on_exit
            .unwrap_or_else(|| Box::new(|_: &UiEvent| false));

        run_window_session_loop_with_system_theme(
            &mut *platform,
            &mut *platform_window,
            &mut session,
            &font_service,
            &image_service,
            &theme,
            system_theme_tokens.as_deref(),
            &debug_mode,
            &cursor_pos,
            None,
            map_ui_event,
            |ev| on_exit(ev),
            |_, _, _| {},
        );

        app_handle.mark_closed();
        self.app_timers.cancel_all();
        self.main_thread_queue.clear();

        0
    }
}

fn create_preferred_engine(
    platform_window: &mut dyn PlatformWindow,
    width: i32,
    height: i32,
) -> Option<Box<dyn GraphicsEngine>> {
    let surface = platform_window.native_surface_ptr();
    match create_gpu_context(surface, width, height).and_then(GpuEngine::new) {
        Ok(mut engine) => match engine.initialize(width, height) {
            Ok(()) => {
                crate::core::log::info_fn("GPU engine initialized");
                return Some(Box::new(engine));
            }
            Err(e) => {
                engine.shutdown();
                crate::core::log::warn_fn(format!(
                    "GPU engine initialize failed, falling back to CPU: {}",
                    e.short_what()
                ));
            }
        },
        Err(e) => {
            crate::core::log::warn_fn(format!(
                "GPU engine unavailable, falling back to CPU: {}",
                e.short_what()
            ));
        }
    }

    let mut engine = SoftwareEngine::new();
    match engine.initialize(width, height) {
        Ok(()) => {
            crate::core::log::info_fn("CPU software engine initialized");
            Some(Box::new(engine))
        }
        Err(e) => {
            crate::core::log::error_fn(format!("SoftwareEngine 初始化失败: {}", e.short_what()));
            None
        }
    }
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

#[cfg(test)]
#[path = "../../tests/app/shell/application.rs"]
mod tests;

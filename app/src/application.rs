use crate::cli::Cli;
use crate::di::Container;
use crate::window::Window;
use uix_graphics::{GraphicsEngine, SoftwareEngine};
use uix_platform::create_platform;
use uix_platform::event::{UiEvent, UiEventPayload, UiEventType};
use std::cell::RefCell;

use uix_ui::theme::Theme;
use uix_ui::widget::{WidgetEvent, WidgetTree};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ════════════════════════════════════════════════════════════════════════════
// 应用模式
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    #[default]
    GUI,
    CLI,
}

/// 渲染策略 — 用户层选择，引擎层实现。
///
/// 在 `App` 层定义切换点，具体渲染方案由 `GraphicsEngine` 实现决定：
/// - `Cpu`    → `SoftwareEngine`（纯 CPU 光栅化）
/// - `Gpu`    → 纯 GPU 引擎（如 Direct2D / Vulkan）
/// - `Hybrid` → CPU + GPU 协作（如 CPU 布局 + GPU 绘制）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderStrategy {
    /// 纯 CPU 软件渲染（默认）
    #[default]
    Cpu,
    /// 纯 GPU 加速渲染
    Gpu,
    /// CPU + GPU 混合协作
    Hybrid,
}

// ════════════════════════════════════════════════════════════════════════════
// App — 应用入口
// ════════════════════════════════════════════════════════════════════════════

pub struct App {
    mode: AppMode,
    window: Option<Window>,
    cli: Option<Cli>,
    container: Container,
    running: Arc<AtomicBool>,
    exit_code: i32,
    window_title: String,
    /// 渲染策略（默认 Cpu）
    render_strategy: RenderStrategy,
    /// 自定义渲染引擎（可选，默认使用 SoftwareEngine）
    custom_engine: Option<Box<dyn GraphicsEngine>>,
    /// 应用主题（默认 Ant Design 亮色）
    pub theme: Theme,
}

impl Default for App {
    fn default() -> Self {
        Self {
            mode: AppMode::GUI,
            window: None,
            cli: None,
            container: Container::new(),
            running: Arc::new(AtomicBool::new(false)),
            exit_code: 0,
            window_title: "UIX App".to_string(),
            render_strategy: RenderStrategy::Cpu,
            custom_engine: None,
            theme: Theme::antd_light(),
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    // ── 配置方法（builder 模式）─────────────────────────────────────

    /// 设置窗口标题。
    pub fn title(&mut self, t: &str) -> &mut Self {
        self.window_title = t.to_string();
        self
    }

    /// 设置渲染策略。
    ///
    /// - `Cpu`    → `SoftwareEngine`（纯 CPU，默认）
    /// - `Gpu`    → 需要先 `.engine(...)` 传入 GPU 引擎
    /// - `Hybrid` → 需要先 `.engine(...)` 传入混合引擎
    pub fn render_strategy(&mut self, s: RenderStrategy) -> &mut Self {
        self.render_strategy = s;
        self
    }

    /// 设置自定义渲染引擎。
    ///
    /// 根据 `render_strategy` 传入对应的引擎实现：
    /// - `Cpu`    → 无需调用此方法，默认用 `SoftwareEngine`
    /// - `Gpu`    → 传入 GPU 引擎（如 `GpuEngine`）
    /// - `Hybrid` → 传入混合引擎（如 `HybridEngine`）
    pub fn engine(&mut self, engine: Box<dyn GraphicsEngine>) -> &mut Self {
        self.custom_engine = Some(engine);
        self
    }

    /// 设置应用模式。
    pub fn mode(&mut self, m: AppMode) -> &mut Self {
        self.mode = m;
        self
    }

    // ── 查询方法 ──────────────────────────────────────────────────

    pub fn current_mode(&self) -> AppMode {
        self.mode
    }

    pub fn current_strategy(&self) -> RenderStrategy {
        self.render_strategy
    }

    // ── 窗口生命周期 ──────────────────────────────────────────────

    /// 创建主窗口。用 `self.window_title` 作为标题。
    ///
    /// 返回 `Err` 如果平台初始化失败。
    pub fn create_window(
        &mut self,
        title: &str,
        width: i32,
        height: i32,
    ) -> Result<&mut Self, uix_diag::Error> {
        let platform = create_platform()?;
        let mut window = Window::new(platform);
        let t = if title.is_empty() {
            &self.window_title
        } else {
            title
        };
        if !window.create(t, width, height) {
            return Err(uix_diag::Error::new(
                uix_diag::Errc::WindowCreationFailed,
                "create_window: platform create_window failed",
            ));
        }
        self.window = Some(window);
        Ok(self)
    }

    // ── CLI ───────────────────────────────────────────────────────

    pub fn cli(&mut self, cli: Cli) -> &mut Self {
        self.cli = Some(cli);
        self
    }

    pub fn container(&mut self) -> &mut Container {
        &mut self.container
    }

    pub fn singleton<T: 'static + Send + Clone>(&mut self, instance: T) -> &mut Self {
        self.container.singleton(instance);
        self
    }

    // ── 运行 ──────────────────────────────────────────────────────

    pub fn run(&mut self) -> i32 {
        self.running.store(true, Ordering::SeqCst);

        match self.mode {
            AppMode::GUI => match self.window.as_mut() {
                Some(window) => {
                    let running = Arc::clone(&self.running);
                    window.run(move |platform| {
                        platform.event_loop().wait_event(&|event: &UiEvent| match event.type_ {
                            UiEventType::WindowClose => {
                                running.store(false, Ordering::SeqCst);
                                false
                            }
                            _ => true,
                        })
                    });
                    0
                }
                None => {
                    log::error!("App::run: no window created for GUI mode");
                    1
                }
            },
            AppMode::CLI => {
                if let Some(ref mut cli) = self.cli {
                    let args = std::env::args().collect::<Vec<_>>();
                    self.exit_code = cli.run(&args);
                } else {
                    log::info!("Running in CLI mode (no commands registered)");
                }
                self.exit_code
            }
        }
    }

    pub fn quit(&mut self, exit_code: i32) {
        self.exit_code = exit_code;
        self.running.store(false, Ordering::SeqCst);
        if let Some(ref mut window) = self.window {
            window.close();
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }

    pub fn window(&self) -> Option<&Window> {
        self.window.as_ref()
    }

    pub fn window_mut(&mut self) -> Option<&mut Window> {
        self.window.as_mut()
    }

    // ── Widget 渲染循环（框架处理全部样板代码）──────────────────────

    /// 根据 `render_strategy` 创建引擎。
    ///
    /// - `Cpu`    → 自动创建 `SoftwareEngine`
    /// - `Gpu`    → 需通过 `.engine(Box::new(GpuEngine::new(ctx, w, h)))` 传入
    /// - `Hybrid` → 同上，传入混合引擎实现
    fn build_engine(
        strategy: RenderStrategy,
        width: i32,
        height: i32,
        system_info: &dyn uix_platform::ISystemInfo,
    ) -> Option<Box<dyn GraphicsEngine>> {
        match strategy {
            RenderStrategy::Cpu => {
                let mut engine = SoftwareEngine::new();
                match engine.initialize(width, height, system_info) {
                    Ok(_) => {
                        log::info!("App: SoftwareEngine (CPU) initialized");
                        Some(Box::new(engine))
                    }
                    Err(e) => {
                        log::error!("App: SoftwareEngine init failed: {}", e.short_what());
                        None
                    }
                }
            }
            RenderStrategy::Gpu | RenderStrategy::Hybrid => {
                log::error!(
                    "App: {:?} strategy requires passing a custom engine via `.engine(...)`",
                    strategy
                );
                None
            }
        }
    }

    /// 取出或创建引擎。
    pub fn take_engine(&mut self, width: i32, height: i32, system_info: &dyn uix_platform::ISystemInfo) -> Option<Box<dyn GraphicsEngine>> {
        if let Some(engine) = self.custom_engine.take() {
            log::info!("App: using custom engine ({:?})", self.render_strategy);
            Some(engine)
        } else {
            Self::build_engine(self.render_strategy, width, height, system_info)
        }
    }

    /// 用默认策略运行 widget 渲染循环。
    ///
    /// `Cpu` 策略自动创建 `SoftwareEngine`。
    /// `Gpu` / `Hybrid` 策略需要先用 `.engine(...)` 传入自定义引擎。
    pub fn run_widget<M, X>(
        &mut self,
        tree: &mut WidgetTree,
        width: i32,
        height: i32,
        map_event: M,
        on_exit: X,
    ) -> i32
    where
        M: Fn(&UiEvent) -> Option<WidgetEvent>,
        X: Fn(&UiEvent) -> bool,
    {
        let platform = match uix_platform::create_platform() {
            Ok(p) => p,
            Err(e) => {
                log::error!("App::run_widget: platform creation failed: {}", e.short_what());
                return 1;
            }
        };
        let system_info = platform.system_info();
        let mut engine = match self.take_engine(width, height, system_info) {
            Some(e) => e,
            None => return 1,
        };

        let exit_code = self.run_widget_with(
            &mut *engine,
            tree,
            width,
            height,
            map_event,
            on_exit,
            |_, _, _| {},
        );
        engine.shutdown();
        exit_code
    }

    /// 用指定引擎运行 widget 渲染循环（App 不接管引擎生命周期）。
    pub fn run_widget_with<M, X, F>(
        &mut self,
        engine: &mut dyn GraphicsEngine,
        tree: &mut WidgetTree,
        width: i32,
        height: i32,
        map_event: M,
        on_exit: X,
        on_frame: F,
    ) -> i32
    where
        M: Fn(&UiEvent) -> Option<WidgetEvent>,
        X: Fn(&UiEvent) -> bool,
        F: Fn(&mut WidgetTree, &mut dyn GraphicsEngine, &mut dyn uix_platform::Platform),
    {
        // 使用 App 的 theme 字段创建运行时动态主题
        let theme_cell = RefCell::new(self.theme.clone());
        self.run_widget_with_tokens(
            engine,
            tree,
            width,
            height,
            &theme_cell,
            map_event,
            on_exit,
            on_frame,
        )
    }

    /// 与 `run_widget_with` 相同，但允许自定义 tokens（用于暗色/亮色切换）。
    pub fn run_widget_with_tokens<M, X, F>(
        &mut self,
        engine: &mut dyn GraphicsEngine,
        tree: &mut WidgetTree,
        width: i32,
        height: i32,
        theme: &RefCell<Theme>,
        map_event: M,
        on_exit: X,
        on_frame: F,
    ) -> i32
    where
        M: Fn(&UiEvent) -> Option<WidgetEvent>,
        X: Fn(&UiEvent) -> bool,
        F: Fn(&mut WidgetTree, &mut dyn GraphicsEngine, &mut dyn uix_platform::Platform),
    {
        if self.window.is_none() {
            if let Err(e) = self.create_window("", width, height) {
                log::error!("App::run_widget_with_tokens: {}", e.short_what());
                return 1;
            }
        }

        match self.window.as_mut() {
            Some(window) => {
                window.run_widget_loop(tree, engine, theme, map_event, on_exit, on_frame)
            }
            None => {
                log::error!("App::run_widget_with_tokens: window creation failed");
                1
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 默认事件映射 — 将平台 UiEvent 转换为 WidgetEvent
// ════════════════════════════════════════════════════════════════════════════

/// Default UiEvent → WidgetEvent mapper for most apps。
pub fn map_ui_event(ev: &UiEvent) -> Option<WidgetEvent> {
    match ev.type_ {
        UiEventType::MouseDown => {
            if let UiEventPayload::MouseButton(ref d) = ev.payload {
                log::debug!("map_ui_event: MouseDown pos=({}, {}) btn={:?}", d.pos.x, d.pos.y, d.btn);
                Some(WidgetEvent::MouseDown {
                    pos: d.pos,
                    button: d.btn,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::MouseUp => {
            if let UiEventPayload::MouseButton(ref d) = ev.payload {
                Some(WidgetEvent::MouseUp {
                    pos: d.pos,
                    button: d.btn,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::MouseMove => {
            if let UiEventPayload::MouseMove(ref d) = ev.payload {
                Some(WidgetEvent::MouseMove { pos: d.pos })
            } else {
                None
            }
        }
        UiEventType::MouseWheel => {
            if let UiEventPayload::MouseWheel(ref d) = ev.payload {
                Some(WidgetEvent::MouseWheel {
                    pos: d.pos,
                    delta: uix_core::Point::new(d.delta_x, d.delta_y),
                })
            } else {
                None
            }
        }
        UiEventType::KeyDown => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(WidgetEvent::KeyDown { key: d.key, mods: d.mods })
            } else {
                None
            }
        }
        UiEventType::KeyUp => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(WidgetEvent::KeyUp { key: d.key, mods: d.mods })
            } else {
                None
            }
        }
        UiEventType::KeyPress => {
            if let UiEventPayload::KeyPress(ref d) = ev.payload {
                Some(WidgetEvent::KeyPress {
                    text: d.text.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::WindowResize => {
            if let UiEventPayload::Resize(ref d) = ev.payload {
                Some(WidgetEvent::Resize {
                    width: d.width as f32,
                    height: d.height as f32,
                })
            } else {
                None
            }
        }
        UiEventType::WindowMaximize => {
            Some(WidgetEvent::WindowMaximize)
        }
        UiEventType::WindowMinimize => {
            Some(WidgetEvent::WindowMinimize)
        }
        UiEventType::WindowRestore => {
            Some(WidgetEvent::WindowRestore)
        }
        UiEventType::WindowFocus => {
            Some(WidgetEvent::WindowFocus)
        }
        UiEventType::WindowBlur => {
            Some(WidgetEvent::WindowBlur)
        }
        UiEventType::Timer => {
            if let UiEventPayload::Timer(ref d) = ev.payload {
                Some(WidgetEvent::Timer { id: d.timer_id })
            } else {
                None
            }
        }
        UiEventType::FileDrop => {
            if let UiEventPayload::FileDrop(ref d) = ev.payload {
                Some(WidgetEvent::FileDrop {
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

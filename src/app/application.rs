use crate::app::cli::Cli;
use crate::app::di::Container;
use crate::app::window::Window;
use crate::graphics::{GraphicsEngine, SoftwareEngine};
use crate::platform::create_platform;
use crate::platform::event::{UiEvent, UiEventPayload, UiEventType};
use crate::ui::theme::DesignTokens;
use crate::ui::widget::{WidgetEvent, WidgetTree};
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
    initialized: bool,
    window_title: String,
    /// 渲染策略（默认 Cpu）
    render_strategy: RenderStrategy,
    /// 自定义渲染引擎（可选，默认使用 SoftwareEngine）
    custom_engine: Option<Box<dyn GraphicsEngine>>,
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
            initialized: false,
            window_title: "UIX App".to_string(),
            render_strategy: RenderStrategy::Cpu,
            custom_engine: None,
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
    pub fn create_window(&mut self, title: &str, width: i32, height: i32) -> &mut Self {
        let platform = create_platform();
        let mut window = Window::new(platform);
        let t = if title.is_empty() { &self.window_title } else { title };
        window.create(t, width, height);
        self.window = Some(window);
        self
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
                        platform.wait_event(&|event: &UiEvent| {
                            match event.type_ {
                                UiEventType::WindowClose => {
                                    running.store(false, Ordering::SeqCst);
                                    false
                                }
                                _ => true,
                            }
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
    fn build_engine(strategy: RenderStrategy, width: i32, height: i32) -> Option<Box<dyn GraphicsEngine>> {
        match strategy {
            RenderStrategy::Cpu => {
                let mut engine = SoftwareEngine::new();
                match engine.initialize(width, height) {
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
    pub fn take_engine(&mut self, width: i32, height: i32) -> Option<Box<dyn GraphicsEngine>> {
        if let Some(engine) = self.custom_engine.take() {
            log::info!("App: using custom engine ({:?})", self.render_strategy);
            Some(engine)
        } else {
            Self::build_engine(self.render_strategy, width, height)
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
        let mut engine = match self.take_engine(width, height) {
            Some(e) => e,
            None => return 1,
        };

        let exit_code = self.run_widget_with(&mut *engine, tree, width, height, map_event, on_exit, |_, _| {});
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
        F: Fn(&mut WidgetTree, &mut dyn GraphicsEngine),
    {
        let tokens = DesignTokens::antd_light();

        if self.window.is_none() {
            self.create_window("", width, height);
        }

        match self.window.as_mut() {
            Some(window) => {
                window.run_widget_loop(tree, engine, &tokens, map_event, on_exit, on_frame)
            }
            None => {
                log::error!("App::run_widget_with: window creation failed");
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
                Some(WidgetEvent::MouseDown { pos: d.pos, button: d.btn })
            } else { None }
        }
        UiEventType::MouseUp => {
            if let UiEventPayload::MouseButton(ref d) = ev.payload {
                Some(WidgetEvent::MouseUp { pos: d.pos, button: d.btn })
            } else { None }
        }
        UiEventType::MouseMove => {
            if let UiEventPayload::MouseMove(ref d) = ev.payload {
                Some(WidgetEvent::MouseMove { pos: d.pos })
            } else { None }
        }
        UiEventType::MouseWheel => {
            if let UiEventPayload::MouseWheel(ref d) = ev.payload {
                Some(WidgetEvent::MouseWheel {
                    delta: crate::base::Point::new(d.delta_x, d.delta_y),
                })
            } else { None }
        }
        UiEventType::KeyDown => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(WidgetEvent::KeyDown { key: d.key })
            } else { None }
        }
        UiEventType::KeyUp => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(WidgetEvent::KeyUp { key: d.key })
            } else { None }
        }
        UiEventType::WindowResize => {
            if let UiEventPayload::Resize(ref d) = ev.payload {
                Some(WidgetEvent::Resize { width: d.width as f32, height: d.height as f32 })
            } else { None }
        }
        _ => None,
    }
}

// ============================================================================
// app/application.rs — 应用配置入口
//
// 职责边界（app 层只负责应用配置）：
//   - 应用模式（GUI / CLI）
//   - 窗口配置（标题、尺寸）
//   - CLI 命令注册与执行
//   - DI 容器注册
//   - 提供窗口访问（供 ui 层驱动渲染）
//
// 超出边界的（应在 ui 层）：
//   - 渲染引擎创建/管理
//   - 渲染策略选择
//   - 字体服务管理
//   - 主题管理
//   - 渲染循环驱动
// ============================================================================

use crate::runtime::cli::Cli;
use crate::runtime::di::Container;
use crate::runtime::window::Window;
use crate::platform::create_platform;
use crate::platform::api::event::{UiEvent, UiEventPayload, UiEventType};

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
// App — 应用配置入口
// ════════════════════════════════════════════════════════════════════════════

pub struct App {
    mode: AppMode,
    window: Option<Window>,
    cli: Option<Cli>,
    container: Container,
    exit_code: i32,
    window_title: String,
    window_size: (i32, i32),
}

impl Default for App {
    fn default() -> Self {
        Self {
            mode: AppMode::GUI,
            window: None,
            cli: None,
            container: Container::new(),
            exit_code: 0,
            window_title: "UIX App".to_string(),
            window_size: (800, 600),
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

    /// 设置窗口初始尺寸。
    pub fn size(&mut self, width: i32, height: i32) -> &mut Self {
        self.window_size = (width, height);
        self
    }

    /// 设置应用模式。
    pub fn mode(&mut self, m: AppMode) -> &mut Self {
        self.mode = m;
        self
    }

    /// 注册 CLI 命令。
    pub fn cli(&mut self, cli: Cli) -> &mut Self {
        self.cli = Some(cli);
        self
    }

    /// 获取 DI 容器。
    pub fn container(&mut self) -> &mut Container {
        &mut self.container
    }

    /// 注册全局单例。
    pub fn singleton<T: 'static + Send + Clone>(&mut self, instance: T) -> &mut Self {
        self.container.singleton(instance);
        self
    }

    // ── 查询方法 ──────────────────────────────────────────────────

    pub fn current_mode(&self) -> AppMode {
        self.mode
    }

    pub fn window_title(&self) -> &str {
        &self.window_title
    }

    pub fn window_size(&self) -> (i32, i32) {
        self.window_size
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

    // ── 窗口生命周期 ──────────────────────────────────────────────

    /// 创建主窗口。
    pub fn create_window(&mut self) -> Result<&mut Self, crate::platform::Error> {
        let platform = create_platform()?;
        let mut window = Window::new(platform);
        let (w, h) = self.window_size;
        if !window.create(&self.window_title, w, h) {
            return Err(crate::platform::Error::new(
                crate::platform::Errc::WindowCreationFailed,
                "create_window: platform create_window failed",
            ));
        }
        self.window = Some(window);
        Ok(self)
    }

    // ── 运行 ──────────────────────────────────────────────────────

    pub fn run(&mut self) -> i32 {
        match self.mode {
            AppMode::GUI => match self.window.as_mut() {
                Some(window) => {
                    window.run(|platform| {
                        platform.event_loop().wait_event(&|event| {
                            !matches!(event.type_, crate::platform::api::event::UiEventType::WindowClose)
                        })
                    });
                    0
                }
                None => {
                    crate::platform::log::error_fn("App::run: no window created for GUI mode");
                    1
                }
            },
            AppMode::CLI => {
                if let Some(ref mut cli) = self.cli {
                    let args = std::env::args().collect::<Vec<_>>();
                    self.exit_code = cli.run(&args);
                } else {
                    crate::platform::log::info_fn("Running in CLI mode (no commands registered)");
                }
                self.exit_code
            }
        }
    }

    pub fn quit(&mut self, exit_code: i32) {
        self.exit_code = exit_code;
        if let Some(ref mut window) = self.window {
            window.close();
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 默认事件映射 — 将平台 UiEvent 转换为 WidgetEvent
// ════════════════════════════════════════════════════════════════════════════

/// Default UiEvent → WidgetEvent mapper for most apps。
pub fn map_ui_event(ev: &UiEvent) -> Option<crate::widget::WidgetEvent> {
    match ev.type_ {
        UiEventType::MouseDown => {
            if let UiEventPayload::MouseButton(ref d) = ev.payload {
                Some(crate::widget::WidgetEvent::MouseDown {
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
                Some(crate::widget::WidgetEvent::MouseUp {
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
                Some(crate::widget::WidgetEvent::MouseMove {
                    pos: d.pos,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::MouseWheel => {
            if let UiEventPayload::MouseWheel(ref d) = ev.payload {
                Some(crate::widget::WidgetEvent::MouseWheel {
                    pos: d.pos,
                    delta: crate::platform::Point::new(d.delta_x, d.delta_y),
                })
            } else {
                None
            }
        }
        UiEventType::KeyDown => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(crate::widget::WidgetEvent::KeyDown {
                    key: d.key,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::KeyUp => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(crate::widget::WidgetEvent::KeyUp {
                    key: d.key,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::KeyPress => {
            if let UiEventPayload::KeyPress(ref d) = ev.payload {
                Some(crate::widget::WidgetEvent::KeyPress {
                    text: d.text.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::WindowResize => {
            if let UiEventPayload::Resize(ref d) = ev.payload {
                Some(crate::widget::WidgetEvent::Resize {
                    width: d.width as f32,
                    height: d.height as f32,
                })
            } else {
                None
            }
        }
        UiEventType::WindowMaximize => Some(crate::widget::WidgetEvent::WindowMaximize),
        UiEventType::WindowMinimize => Some(crate::widget::WidgetEvent::WindowMinimize),
        UiEventType::WindowRestore => Some(crate::widget::WidgetEvent::WindowRestore),
        UiEventType::WindowFocus => Some(crate::widget::WidgetEvent::WindowFocus),
        UiEventType::WindowBlur => Some(crate::widget::WidgetEvent::WindowBlur),
        UiEventType::Timer => {
            if let UiEventPayload::Timer(ref d) = ev.payload {
                Some(crate::widget::WidgetEvent::Timer { id: d.timer_id })
            } else {
                None
            }
        }
        UiEventType::FileDrop => {
            if let UiEventPayload::FileDrop(ref d) = ev.payload {
                Some(crate::widget::WidgetEvent::FileDrop {
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

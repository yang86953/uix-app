//! 应用入口 — 统一 GUI / CLI 生命周期。

use std::cell::{Cell, RefCell};

use crate::native::traits::event::{UiEvent, UiEventPayload, UiEventType};
use crate::native::{create_platform, Point};
use crate::draw::font::font_service::FontService;
use crate::draw::traits::GraphicsEngine;
use crate::draw::SoftwareEngine;
use crate::app::shell::cli::Cli;
use crate::app::shell::di::Container;
use crate::ui::view::adapter::ViewAdapter;
use crate::ui::view::{View, ViewNode};
use crate::app::event_loop::run_widget_loop;
use crate::ui::theme::Theme;
use crate::ui::{WidgetCore, WidgetEvent};

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
    root: Option<ViewNode>,
    on_exit: Option<Box<dyn Fn(&UiEvent) -> bool>>,
    cli: Option<Cli>,
    container: Container,
    exit_code: i32,
}

impl Default for App {
    fn default() -> Self {
        Self {
            mode: AppMode::GUI,
            title: "UIX App".to_string(),
            size: (800, 600),
            theme: Theme::antd_light(),
            root: None,
            on_exit: None,
            cli: None,
            container: Container::new(),
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

    /// 设置根 View（GUI 模式必需）。
    pub fn root(mut self, view: impl View) -> Self {
        crate::ui::state::begin_state_capture();
        self.root = Some(view.build());
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

    // ── 运行 ──────────────────────────────────────────────────────

    pub fn run(mut self) -> i32 {
        match self.mode {
            AppMode::CLI => self.run_cli(),
            AppMode::GUI => self.run_gui(),
        }
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
        let root_node = match self.root.take() {
            Some(node) => node,
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

        let mut engine = SoftwareEngine::new();
        if engine.initialize(w, h).is_err() {
            crate::core::log::error_fn("SoftwareEngine 初始化失败");
            return 1;
        }

        let mut font_service = FontService::new();
        font_service.load_default_system_font(14.0, platform.system_info());

        let mut tree = ViewAdapter::build_nodes(root_node);
        if let Some(root) = tree.root_mut() {
            root.set_frame(crate::native::Rect::new(0.0, 0.0, w as f32, h as f32));
        }
        tree.layout();
        tree.mark_full_frame_dirty();

        let theme = RefCell::new(self.theme);
        let debug_mode = Cell::new(false);
        let cursor_pos = Cell::new(Point::new(0.0, 0.0));
        let on_exit = self
            .on_exit
            .unwrap_or_else(|| Box::new(|_: &UiEvent| false));

        run_widget_loop(
            &mut *platform,
            &mut *platform_window,
            &mut engine,
            &mut tree,
            &font_service,
            &theme,
            &debug_mode,
            &cursor_pos,
            None,
            map_ui_event,
            |ev| on_exit(ev),
            |_, _, _| {},
        );

        0
    }
}

// ════════════════════════════════════════════════════════════════════════════
// UiEvent → WidgetEvent 映射（唯一实现）
// ════════════════════════════════════════════════════════════════════════════

/// 将平台 `UiEvent` 转换为 `WidgetEvent`。
pub fn map_ui_event(ev: &UiEvent) -> Option<WidgetEvent> {
    match ev.type_ {
        UiEventType::MouseDown => {
            if let UiEventPayload::MouseButton(ref d) = ev.payload {
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
                Some(WidgetEvent::MouseMove {
                    pos: d.pos,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::MouseWheel => {
            if let UiEventPayload::MouseWheel(ref d) = ev.payload {
                Some(WidgetEvent::MouseWheel {
                    pos: d.pos,
                    delta: Point::new(d.delta_x, d.delta_y),
                })
            } else {
                None
            }
        }
        UiEventType::KeyDown => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(WidgetEvent::KeyDown {
                    key: d.key,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::KeyUp => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(WidgetEvent::KeyUp {
                    key: d.key,
                    mods: d.mods,
                })
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
        UiEventType::WindowMaximize => Some(WidgetEvent::WindowMaximize),
        UiEventType::WindowMinimize => Some(WidgetEvent::WindowMinimize),
        UiEventType::WindowRestore => Some(WidgetEvent::WindowRestore),
        UiEventType::WindowFocus => Some(WidgetEvent::WindowFocus),
        UiEventType::WindowBlur => Some(WidgetEvent::WindowBlur),
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

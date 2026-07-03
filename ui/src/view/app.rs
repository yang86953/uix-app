//! 简化 App 入口。
//!
//! 提供链式 API 一键启动 UI 应用：
//!
//! ```ignore
//! uix::ui::view::App::new()
//!     .title("我的应用")
//!     .size(1024, 768)
//!     .root(my_view)
//!     .run();
//! ```
//!
//! 用户不需要接触 `create_platform`、`SoftwareEngine`、`FontService`、
//! `run_widget_loop` 等内部 API。

use crate::render_loop::run_widget_loop;
use crate::theme::Theme;
use crate::view::adapter::ViewAdapter;
use crate::view::{View, ViewNode};
use crate::widget::{WidgetCore, WidgetEvent};
use std::cell::{Cell, RefCell};
use uix_graphics::font_service::FontService;
use uix_graphics::{GraphicsEngine, SoftwareEngine};
use uix_platform::event::{UiEvent, UiEventPayload, UiEventType};
use uix_platform::{create_platform, Point};

/// 默认的 UiEvent → WidgetEvent 映射。
fn default_map_event(ev: &UiEvent) -> Option<WidgetEvent> {
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
                    delta: uix_platform::Point::new(d.delta_x, d.delta_y),
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

/// 简化 UI 应用入口。
///
/// 通过链式 Builder API 配置并启动应用：
///
/// ```ignore
/// App::new()
///     .title("Hello")
///     .size(800, 600)
///     .root(my_view)
///     .run();
/// ```
pub struct App {
    title: String,
    size: (i32, i32),
    theme: Theme,
    root: Option<ViewNode>,
    on_exit: Option<Box<dyn Fn(&UiEvent) -> bool>>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            title: "UIX App".to_string(),
            size: (800, 600),
            theme: Theme::antd_light(),
            root: None,
            on_exit: None,
        }
    }
}

impl App {
    /// 创建新 App 实例。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置窗口标题。
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// 设置窗口初始尺寸（宽度, 高度）。
    pub fn size(mut self, width: i32, height: i32) -> Self {
        self.size = (width, height);
        self
    }

    /// 设置主题。
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// 设置根 View。
    ///
    /// 接受实现了 `View` trait 的类型（所有组合子函数如 `column()`、`button()` 的返回值）。
    pub fn root(mut self, view: impl View) -> Self {
        crate::state::begin_state_capture();
        let node = view.build();
        self.root = Some(node);
        self
    }

    /// 设置退出条件回调。返回 `true` 时退出事件循环。
    pub fn on_exit<F: Fn(&UiEvent) -> bool + 'static>(mut self, f: F) -> Self {
        self.on_exit = Some(Box::new(f));
        self
    }

    /// 启动应用。
    ///
    /// 内部完成：创建平台、窗口、图形引擎、字体服务，展开 View 树为 WidgetTree，
    /// 然后进入渲染事件循环。用户不需要了解任何内部细节。
    pub fn run(mut self) {
        let (w, h) = self.size;
        let root_node = match self.root.take() {
            Some(node) => node,
            None => {
                uix_platform::log::error_fn("App::run() 前必须调用 .root() 设置根 View");
                return;
            }
        };

        // ── 1. 创建平台 ──
        let mut platform = match create_platform() {
            Ok(p) => p,
            Err(e) => {
                uix_platform::log::error_fn(format!("create_platform 失败: {:?}", e));
                return;
            }
        };

        // ── 2. 创建窗口 ──
        let mut platform_window = match platform.window_manager().create_window(&self.title, w, h) {
            Ok(w) => w,
            Err(e) => {
                uix_platform::log::error_fn(format!("create_window 失败: {:?}", e));
                return;
            }
        };
        platform_window.center_on_screen();
        platform_window.show();
        platform_window.raise();

        // ── 3. 创建图形引擎 ──
        let mut engine: Box<dyn GraphicsEngine> = {
            let mut se = SoftwareEngine::new();
            let init_result = se.initialize(w, h);
            if init_result.is_err() {
                uix_platform::log::error_fn("SoftwareEngine 初始化失败");
                return;
            }
            Box::new(se)
        };

        // ── 4. 创建字体服务 ──
        let mut font_service = FontService::new();
        font_service.load_default_system_font(14.0, platform.system_info());

        // ── 5. 展开 ViewNode 为 WidgetTree ──
        let mut tree = ViewAdapter::build_nodes(root_node);
        if let Some(root) = tree.root_mut() {
            root.set_frame(uix_platform::Rect::new(0.0, 0.0, w as f32, h as f32));
        }
        tree.layout();
        tree.mark_full_frame_dirty();

        // ── 6. 状态管理 ──
        let theme = RefCell::new(self.theme);
        let debug_mode = Cell::new(false);
        let cursor_pos = Cell::new(Point::new(0.0, 0.0));

        let on_exit = self
            .on_exit
            .unwrap_or_else(|| Box::new(|_: &UiEvent| false));

        // ── 7. 进入渲染事件循环 ──
        run_widget_loop(
            &mut *platform,
            &mut *platform_window,
            &mut *engine,
            &mut tree,
            &font_service,
            &theme,
            &debug_mode,
            &cursor_pos,
            None,
            default_map_event,
            |ev| on_exit(ev),
            |_, _, _| {},
        );
    }
}

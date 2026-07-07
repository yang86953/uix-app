//! 组件库演示运行时 — 平台、引擎、字体与事件循环。
//!
//! 本模块展示**高级用法**：直接操作 `WidgetTree` + `run_widget_loop`，
//! 适用于多页面切换、主题重建、GPU 引擎等 `App::new()` 尚未封装的场景。
//! 入门开发请优先使用 `simplified` 演示中的 `prelude` + `App` 路径。

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use uix::core::log::{debug_fn, error_fn, info_fn, warn_fn};
use uix::draw::GpuEngine;
use uix::native::create_gpu_context;
use uix::native::traits::event::{UiEvent, UiEventPayload, UiEventType};
use uix::native::traits::platform::Platform;
use uix::native::traits::window::PlatformWindow;
use uix::prelude::*;
use uix::ui::core::widget::WidgetTree;
use uix::ui::widgets::icon::init_lucide_font;

use super::{rebuild_for_theme, switch_page, DemoState};

/// 创建图形引擎（`--gpu` 时尝试 GPU，失败回退 CPU）。
pub fn create_engine(
    use_gpu: bool,
    platform_window: &dyn PlatformWindow,
    width: i32,
    height: i32,
) -> Box<dyn GraphicsEngine> {
    if use_gpu {
        let surface_ptr = platform_window.native_surface_ptr();
        if surface_ptr.is_null() {
            error_fn("GPU: 无法获取 surface 指针，回退 CPU");
        } else {
            match create_gpu_context(surface_ptr, width, height) {
                Ok(ctx) => match GpuEngine::new(ctx) {
                    Ok(mut engine) => {
                        if let Err(err) = engine.initialize(width, height) {
                            error_fn(format!("GPU 引擎初始化失败: {}", err.short_what()));
                        } else {
                            info_fn("GPU: GpuEngine 就绪");
                            return Box::new(engine);
                        }
                    }
                    Err(e) => {
                        warn_fn(format!(
                            "GPU: GpuEngine 创建失败({}), 回退 CPU",
                            e.short_what()
                        ));
                    }
                },
                Err(e) => {
                    warn_fn(format!("GPU: 上下文创建失败({}), 回退 CPU", e.short_what()));
                }
            }
        }
    }

    let mut engine = SoftwareEngine::new();
    if let Err(e) = engine.initialize(width, height) {
        error_fn(format!("CPU 引擎初始化失败: {}", e.short_what()));
    }
    Box::new(engine)
}

/// 加载 Lucide 图标字体与系统默认文本字体。
pub fn create_font_service(platform: &dyn Platform) -> FontService {
    let mut font_service = FontService::new();
    if let Ok(ttf) = std::fs::read("assets/fonts/lucide.ttf") {
        init_lucide_font(&ttf, &mut font_service);
    } else {
        warn_fn("Lucide font not found — icons will be blank");
    }
    font_service.load_default_system_font(14.0, platform.system_info());
    font_service
}

/// 运行组件库演示事件循环。
pub(super) fn run_event_loop(
    platform: &mut dyn Platform,
    platform_window: &mut dyn PlatformWindow,
    engine: &mut dyn GraphicsEngine,
    tree: &mut WidgetTree,
    font_service: &FontService,
    image_service: &ImageService,
    state: &DemoState,
    dyn_tokens: Arc<DynTokens>,
) -> i32 {
    let theme_cell = RefCell::new(Theme::from_arc(dyn_tokens.clone()));
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::new(0.0, 0.0));

    run_widget_loop(
        platform,
        platform_window,
        engine,
        tree,
        font_service,
        image_service,
        &theme_cell,
        &debug_mode,
        &cursor_pos,
        None,
        map_ui_event,
        |ev| {
            matches!(
                ev,
                UiEvent {
                    type_: UiEventType::KeyDown,
                    payload: UiEventPayload::Key(ref d),
                    ..
                } if d.key == KeyCode::Escape
            )
        },
        move |tree: &mut WidgetTree, eng: &mut dyn GraphicsEngine, _platform: &mut dyn Platform| {
            let mut new_dark = false;
            tree.find_by_type_and_modify::<ThemeToggle>(|w| new_dark = w.dark.get());

            if new_dark != state.dark_mode.get() {
                debug_fn(format!("on_frame: 主题切换 dark={}", new_dark));
                state.dark_mode.set(new_dark);
                dyn_tokens.set_mode(new_dark);
                let nav_active = rebuild_for_theme(tree, eng, &dyn_tokens, state);
                *state.nav_active.borrow_mut() = nav_active;
                return;
            }

            let active = state.nav_active.borrow().get();
            if active != state.prev_active.get() {
                switch_page(tree, state, active);
            }
        },
    )
}

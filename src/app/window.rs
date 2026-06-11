// ============================================================================
// app/window.rs — 基于 Platform 的事件驱动窗口
//
// 核心设计：
//   - 组合 `Box<dyn Platform>`，将窗口管理职责委托给平台层
//   - 不重复维护窗口状态（尺寸、最小化等），全部通过 Platform trait 获取
//   - 提供 run() 事件循环 + 帧回调机制，调用者负责渲染管线
// ============================================================================

use std::cell::Cell;

use crate::graphics::frame_graph::FrameGraph;
use crate::graphics::frame_graph::pass;
use crate::graphics::frame_graph::resource;
use crate::graphics::{DirtyRegion, GraphicsEngine};
use crate::platform::event::{UiEvent, UiEventPayload, UiEventType};
use crate::platform::Platform;
use crate::ui::render_context::RenderContext;
use std::cell::RefCell;

use crate::ui::theme::Theme;
use crate::ui::widget::{WidgetEvent, WidgetTree};
use std::time::Instant;

/// Event-driven application window.
///
/// Wraps a `dyn Platform` and provides a high-level event loop.
/// Does **not** duplicate platform window state — delegates all window
/// management (dimensions, minimize, maximize, etc.) to the inner `Platform`.
pub struct Window {
    platform: Box<dyn Platform>,
    running: bool,
    exit_code: i32,
}

impl Window {
    /// Create a new Window from a platform implementation.
    pub fn new(platform: Box<dyn Platform>) -> Self {
        Self {
            platform,
            running: false,
            exit_code: 0,
        }
    }

    // ── 平台访问器 ──────────────────────────────────────────────────

    /// Access the underlying platform (read-only).
    pub fn platform(&self) -> &dyn Platform {
        self.platform.as_ref()
    }

    /// Access the underlying platform (mutable).
    pub fn platform_mut(&mut self) -> &mut dyn Platform {
        self.platform.as_mut()
    }

    // ── 窗口生命周期（委托给 platform）────────────────────────────

    /// Create the native window, center it on screen, show it, and raise.
    /// Returns `true` on success.
    pub fn create(&mut self, title: &str, width: i32, height: i32) -> bool {
        if !self.platform.create_window(title, width, height) {
            log::error!("Window::create: platform failed to create window");
            return false;
        }
        // Auto-complete window initialization: center → show → raise
        self.platform.center_on_screen();
        self.platform.show();
        self.platform.raise();
        log::info!(
            "Window created and shown ({}x{}, title='{}')",
            width, height, title
        );
        true
    }

    /// Show the window (called automatically by `create()`).
    pub fn show(&mut self) {
        self.platform.show();
    }

    /// Close the window and signal the event loop to exit.
    pub fn close(&mut self) {
        self.running = false;
    }

    /// Check whether the event loop is still running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    // ── 事件循环 ────────────────────────────────────────────────────

    /// Run the event loop with full widget integration.
    ///
    /// Encapsulates the complete frame cycle so every platform shares the
    /// same rendering loop — event collection, animation driving via
    /// `tree.update(dt)`, layout, rendering, presentation, and frame rate
    /// control. Platform code only needs to provide `poll_event`,
    /// `wait_event` and `present_pixels`.
    ///
    /// **Event loop strategy:**
    /// - `tree.update(dt)` returns `true` while any widget is animating.
    /// - Animating → `poll_event` (non-blocking), capped at 60fps.
    /// - Idle → `wait_event` (blocking, zero CPU when idle).
    ///
    /// # Parameters
    /// - `tree` — widget tree to update and render each frame.
    /// - `engine` — graphics engine for rendering.
    /// - `theme` — runtime-switchable theme (`RefCell<Theme>`, call `theme.borrow().tokens()` each frame).
    /// - `map_event` — converts a platform `UiEvent` into a `WidgetEvent`
    ///   (or `None` to ignore). Platform-specific details like key codes
    ///   and mouse button mappings go here.
    /// - `on_exit` — optional extra exit conditions (return `false` to
    ///   keep running, `true` to exit).
    pub fn run_widget_loop<M, X, F>(
        &mut self,
        tree: &mut WidgetTree,
        engine: &mut dyn GraphicsEngine,
        theme: &RefCell<Theme>,
        map_event: M,
        on_exit: X,
        on_frame: F,
    ) -> i32
    where
        M: Fn(&UiEvent) -> Option<WidgetEvent>,
        X: Fn(&UiEvent) -> bool,
        F: Fn(&mut WidgetTree, &mut dyn GraphicsEngine),
    {
        self.running = true;
        let running_flag = Cell::new(true);
        let pending_events = std::cell::RefCell::new(Vec::<UiEvent>::new());
        let mut first_frame = true;
        let mut rendered_first_frame = false;
        let mut last_frame = Instant::now();
        let mut keep_polling = false;
        const FRAME_TIME: f32 = 1.0 / 60.0;

        // ── 帧图初始化 ──────────────────────────────────────────────
        let mut fg = FrameGraph::new();
        let _main_color = fg.register_texture(
            "main_color",
            engine.width() as u32,
            engine.height() as u32,
        );
        let dirty_flag = fg.register_buffer("dirty_flag", 1);
        // 仅声明依赖的 Pass：编译时追踪 dirty_flag → main_color 的消费链
        // 无执行闭包，渲染通过 execute_with 的外部回调完成
        fg.add_pass_node(pass::PassNode::new_structural(
            resource::PassId(0),
            "RenderUI",
            vec![dirty_flag],
            vec![_main_color],
        ));

        let collect = |ev: &UiEvent| {
            match ev.type_ {
                UiEventType::WindowClose => {
                    running_flag.set(false);
                    return false;
                }
                _ => {
                    if on_exit(ev) {
                        running_flag.set(false);
                        return false;
                    }
                }
            }
            pending_events.borrow_mut().push(ev.clone());
            true
        };

        while running_flag.get() {
            // ── Event collection ─────────────────────────────────
            let mut woke = false;
            if first_frame || keep_polling {
                self.platform.poll_event(&collect);
                first_frame = false;
            } else {
                let alive = self.platform.wait_event(&collect);
                if !alive {
                    break;
                }
                self.platform.poll_event(&collect);
                last_frame = Instant::now();
                woke = true;
            }

            // ── Dispatch events → widget tree ────────────────────
            for ev in pending_events.borrow_mut().drain(..) {
                if let UiEventType::WindowResize = ev.type_ {
                    if let UiEventPayload::Resize(ref d) = ev.payload {
                        if d.width > 0 && d.height > 0 {
                            // Resize the engine BEFORE widget dispatch so
                            // the pixel buffer matches the window dimensions.
                            engine.resize(d.width, d.height);
                        }
                        // Always dispatch Resize to the widget tree,
                        // even for (0,0) — the tree handler ignores
                        // invalid dimensions internally.
                        if let Some(we) = map_event(&ev) {
                            tree.dispatch_event(&we);
                        }
                    }
                } else if let Some(we) = map_event(&ev) {
                    tree.dispatch_event(&we);
                }
            }

            // ── Advance animations (dt capped to prevent jump) ───
            let now = Instant::now();
            let dt = (now - last_frame).as_secs_f32().min(0.05);
            last_frame = now;
            keep_polling = tree.update(dt) || woke;

            // ── 逐帧回调（应用层注入：如导航→滚动联动） ────────
            on_frame(tree, engine);

            // ── 帧图：标记脏状态 ──────────────────────────────────
            // keep_polling 作为额外安全网：有动画在跑（如滚动惯性）时确保渲染，
            // 即使 tree.dirty_region() 因某些原因未被标记。
            if !rendered_first_frame || tree.dirty_region().clear_required || keep_polling {
                fg.mark_resource_dirty(dirty_flag);
            }

            // ── 帧图：编译（自动裁剪：输入未变 + 输出未被消费 → 跳过） ─
            let plan = fg.compile();

            // ── 帧图：执行（仅在 zero_frame_cost = false 时渲染） ──
            if !plan.zero_frame_cost {
                let region = if !rendered_first_frame || tree.dirty_region().full_frame {
                    DirtyRegion::full()
                } else {
                    tree.dirty_region().clone()
                };
                // 像素缓冲滚动：提前 drain 以便判断是否需要全 surface damage
                let scroll_deltas = tree.drain_scroll_deltas();
                // 脏区域坐标（用于平台层局部 damage，减轻合成器负担）
                // 当有像素缓冲滚动时，scroll_region 移位了整个视口内容，
                // 必须上报全 surface damage，否则合成器只更新 strip 区域，
                // 导致移位部分残留旧帧像素（"旧内容残留"）
                let dirty = if region.full_frame || !scroll_deltas.is_empty() {
                    None
                } else {
                    Some((
                        region.rect.x as i32,
                        region.rect.y as i32,
                        region.rect.w as i32,
                        region.rect.h as i32,
                    ))
                };

                // 通过外部回调执行渲染（避免 PassFn 的 'static 限制）
                fg.execute_with(engine, &plan, &mut |_pid, eng, _res| {
                    for &(viewport, _dx, dy) in &scroll_deltas {
                        eng.scroll_region(viewport, dy);
                    }
                    eng.begin_frame(&region);
                    tree.layout();
                    let theme_guard = theme.borrow();
                    let tokens = theme_guard.tokens();
                    let mut rctx = RenderContext::new(
                        eng,
                        crate::graphics::FontHandle::default(),
                        tokens,
                    );
                    tree.render_tree(&mut rctx);
                    eng.end_frame(&region);
                    tree.reset_dirty();
                    Ok(())
                })
                .map_err(|e| {
                    log::error!("FrameGraph execute failed: {}", e.short_what());
                })
                .ok();

                // 首帧：两次呈现确保 DWM 合成表面已建立
                if !rendered_first_frame {
                    self.platform
                        .present_pixels(engine.pixels(), engine.width(), engine.height(), dirty);
                }
                self.platform
                    .present_pixels(engine.pixels(), engine.width(), engine.height(), dirty);
                rendered_first_frame = true;
            }

            // ── 60fps frame cap when animating ───────────────────
            if keep_polling {
                let frame_elapsed = last_frame.elapsed().as_secs_f32();
                if frame_elapsed < FRAME_TIME {
                    std::thread::sleep(std::time::Duration::from_secs_f32(
                        FRAME_TIME - frame_elapsed,
                    ));
                }
            }
        }

        self.running = false;
        log::info!("Widget event loop ended");
        self.exit_code
    }

    /// Run the raw event loop (blocks until exit).
    ///
    /// For each iteration, calls `frame_fn` which receives `&mut dyn Platform`
    /// and returns `true` to continue or `false` to exit the loop.
    ///
    /// The caller is responsible for:
    /// - Polling or waiting for platform events via `platform.poll_event()`
    ///   or `platform.wait_event()`
    /// - Rendering via `GraphicsEngine`
    /// - Presenting the pixel buffer
    ///
    /// No frame rate capping is applied — rendering only happens when
    /// `frame_fn` chooses to do so. Use `wait_event()` inside `frame_fn`
    /// to block until events arrive (0 CPU when idle).
    pub fn run<F>(&mut self, mut frame_fn: F) -> i32
    where
        F: FnMut(&mut dyn Platform) -> bool,
    {
        self.running = true;

        log::info!(
            "Window event loop started ({}x{})",
            self.platform.width(),
            self.platform.height()
        );

        while self.running {
            let should_continue = frame_fn(self.platform.as_mut());
            if !should_continue {
                self.running = false;
            }
        }

        self.running = false;
        log::info!("Window event loop ended");
        self.exit_code
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if self.running {
            // Ensure the native window is destroyed
            self.platform.destroy_window();
        }
        self.running = false;
    }
}

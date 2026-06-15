// ============================================================================
// app/window.rs — 基于 Platform 的事件驱动窗口
//
// 核心设计：
//   - 组合 `Box<dyn Platform>`，将窗口管理职责委托给平台层
//   - 不重复维护窗口状态（尺寸、最小化等），全部通过 Platform trait 获取
//   - 提供 run() 事件循环 + 帧回调机制，渲染委托给引擎
// ============================================================================

use std::cell::Cell;

use crate::graphics::{GraphicsEngine, RenderOutcome};
use crate::platform::event::{UiEvent, UiEventPayload, UiEventType};
use crate::platform::Platform;
use std::cell::RefCell;

use crate::ui::theme::Theme;
use crate::ui::widget::{WidgetEvent, WidgetTree};
use std::time::Instant;

/// Event-driven application window.
pub struct Window {
    platform: Box<dyn Platform>,
    running: bool,
    exit_code: i32,
}

impl Window {
    pub fn new(platform: Box<dyn Platform>) -> Self {
        Self {
            platform,
            running: false,
            exit_code: 0,
        }
    }

    pub fn platform(&self) -> &dyn Platform {
        self.platform.as_ref()
    }
    pub fn platform_mut(&mut self) -> &mut dyn Platform {
        self.platform.as_mut()
    }

    pub fn create(&mut self, title: &str, width: i32, height: i32) -> bool {
        if let Err(e) = self.platform.create_window(title, width, height) {
            log::error!("Window::create: platform failed: {}", e.short_what());
            return false;
        }
        self.platform.center_on_screen();
        self.platform.show();
        self.platform.raise();
        log::info!(
            "Window created and shown ({}x{}, title='{}')",
            width,
            height,
            title
        );
        true
    }

    pub fn show(&mut self) {
        self.platform.show();
    }
    pub fn close(&mut self) {
        self.running = false;
    }
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Run the event loop with full widget integration.
    ///
    /// Window 负责：事件收集、分发、动画推进、帧率控制、呈现。
    /// **渲染全部委托给引擎的 render_frame()。**
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

            for ev in pending_events.borrow_mut().drain(..) {
                if let UiEventType::WindowResize = ev.type_ {
                    if let UiEventPayload::Resize(ref d) = ev.payload {
                        if d.width > 0 && d.height > 0 {
                            engine.resize(d.width, d.height);
                        }
                        if let Some(we) = map_event(&ev) {
                            tree.dispatch_event(&we);
                        }
                    }
                } else if let Some(we) = map_event(&ev) {
                    tree.dispatch_event(&we);
                }
            }

            let now = Instant::now();
            let dt = (now - last_frame).as_secs_f32().min(0.05);
            last_frame = now;
            keep_polling = tree.update(dt) || woke;

            on_frame(tree, engine);

<<<<<<< Updated upstream
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
=======
            match engine.render_frame(tree, theme, !rendered_first_frame, keep_polling) {
                RenderOutcome::Present(damage) => {
                    let dirty = if rendered_first_frame { damage } else { None };
                    if !rendered_first_frame {
                        self.platform.present_pixels(
                            engine.pixels(),
                            engine.width(),
                            engine.height(),
                            None,
                        );
>>>>>>> Stashed changes
                    }
                    self.platform.present_pixels(
                        engine.pixels(),
                        engine.width(),
                        engine.height(),
                        dirty,
                    );
                    rendered_first_frame = true;
                }
                RenderOutcome::Idle => {}
            }

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
            if !frame_fn(self.platform.as_mut()) {
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
            self.platform.destroy_window();
        }
        self.running = false;
    }
}

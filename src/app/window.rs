// ============================================================================
// app/window.rs — 基于 Platform 的事件驱动窗口
//
// 核心设计：
//   - 组合 `Box<dyn Platform>`，将窗口管理职责委托给平台层
//   - 不重复维护窗口状态（尺寸、最小化等），全部通过 Platform trait 获取
//   - 提供 run() 事件循环 + 帧回调机制，渲染委托给引擎
//   - FrameGraph（帧图） 驱动渲染 Pass 编排：自动裁剪未变化的 Pass
// ============================================================================

use std::cell::Cell;
use std::cell::RefCell;
use std::time::Instant;

use crate::base::Rect;
use crate::graphics::frame_graph::resource::ResourceId;
use crate::graphics::frame_graph::FrameGraph;
use crate::ui::layer::LayerTree;
use crate::graphics::{DirtyRegion, GraphicsEngine, RenderOutcome};
use crate::platform::event::{UiEvent, UiEventPayload, UiEventType};
use crate::platform::Platform;

use crate::ui::theme::Theme;
use crate::ui::widget::{WidgetCore, WidgetEvent, WidgetTree};

/// Event-driven application window.
///
/// 持有 FrameGraph（帧图）驱动渲染 Pass 编排。
pub struct Window {
    platform: Box<dyn Platform>,
    running: bool,
    exit_code: i32,
    /// 帧图——渲染 Pass 编排器。
    frame_graph: FrameGraph,
    /// 主帧缓冲资源 ID（FrameGraph 资源注册）。
    main_color_res: ResourceId,
    /// 首帧已渲染标志。
    rendered_first: bool,
    /// 上次渲染时的 WidgetTree 版本号（用于检测结构变更）。
    last_tree_version: u64,
    /// 调试模式开关（F12 切换），开启后在 overlay 层绘制调试边框和信息。
    debug_mode: Cell<bool>,
    /// Geometry Pass 的 ID（注册一次，复用）。
    geom_pass_id: Option<crate::graphics::frame_graph::resource::PassId>,
    /// Overlay Pass 的 ID（注册一次，复用）。
    over_pass_id: Option<crate::graphics::frame_graph::resource::PassId>,
}

impl Window {
    pub fn new(platform: Box<dyn Platform>) -> Self {
        let mut fg = FrameGraph::new();
        let main_color = fg.register_texture("MainColor", 800, 600);
        Self {
            platform,
            running: false,
            exit_code: 0,
            frame_graph: fg,
            main_color_res: main_color,
            rendered_first: false,
            last_tree_version: 0,
            debug_mode: Cell::new(false),
            geom_pass_id: None,
            over_pass_id: None,
        }
    }

    pub fn platform(&self) -> &dyn Platform {
        self.platform.as_ref()
    }
    pub fn platform_mut(&mut self) -> &mut dyn Platform {
        self.platform.as_mut()
    }

    pub fn create(&mut self, title: &str, width: i32, height: i32) -> bool {
        if let Err(e) = self.platform.window_manager().create_window(title, width, height) {
            log::error!("Window::create: platform failed: {}", e.short_what());
            return false;
        }
        self.platform.window_manager().center_on_screen();
        self.platform.window_manager().show();
        self.platform.window_manager().raise();
        log::info!(
            "Window created and shown ({}x{}, title='{}')",
            width,
            height,
            title
        );
        true
    }

    pub fn show(&mut self) {
        self.platform.window_manager().show();
    }
    pub fn close(&mut self) {
        self.running = false;
        self.platform.window_manager().destroy_window();
    }
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Run the event loop with full widget integration.
    ///
    /// Window 负责：事件收集、分发、动画推进、帧率控制、呈现。
    /// **渲染由 FrameGraph（帧图）驱动，engine.render_frame() 不再直接调用。**
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
        F: Fn(&mut WidgetTree, &mut dyn GraphicsEngine, &mut dyn Platform),
    {
        self.running = true;
        let running_flag = Cell::new(true);
        let pending_events = std::cell::RefCell::new(Vec::<UiEvent>::new());
        let mut first_frame = true;
        let mut rendered_first_frame = false;
        let mut last_frame = Instant::now();
        let mut keep_polling = false;
        const FRAME_TIME: f32 = 1.0 / 120.0;
        let mut layer_tree = LayerTree::new();

        let collect = |ev: &UiEvent| {
            match ev.type_ {
                UiEventType::WindowClose => {
                    log::debug!("collect: WindowClose -> exit");
                    running_flag.set(false);
                    return false;
                }
                _ => {
                    if on_exit(ev) {
                        log::debug!("collect: on_exit -> exit");
                        running_flag.set(false);
                        return false;
                    }
                }
            }
            log::trace!("collect: {:?}", ev.type_);
            pending_events.borrow_mut().push(ev.clone());
            true
        };

        while running_flag.get() {
            let mut woke = false;
            if first_frame || keep_polling {
                if !self.platform.event_loop().poll_event(&collect) {
                    break;
                }
                first_frame = false;
            } else {
                let alive = self.platform.event_loop().wait_event(&collect);
                if !alive {
                    break;
                }
                self.platform.event_loop().poll_event(&collect);
                last_frame = Instant::now();
                woke = true;
            }

            // ═══════════════════════════════════════════════════════════
            // [TIMING] 帧耗时统计
            // ═══════════════════════════════════════════════════════════
            #[cfg(debug_assertions)]
            let _frame_t0 = std::time::Instant::now();
            #[cfg(debug_assertions)]
            let mut _last_tmark = _frame_t0;
            #[cfg(debug_assertions)]
            macro_rules! _tmark {
                ($label:expr) => {{
                    let elapsed = _frame_t0.elapsed();
                    let since_last = _last_tmark.elapsed();
                    if elapsed.as_secs_f32() > 0.1 {
                        log::debug!("[TIMING] {}: total={:.1}s  step={:.1}s", $label, elapsed.as_secs_f32(), since_last.as_secs_f32());
                    }
                    _last_tmark = std::time::Instant::now();
                }};
            }
            #[cfg(not(debug_assertions))]
            macro_rules! _tmark {
                ($label:expr) => {{}};
            }

            // ═══════════════════════════════════════════════════════════
            // 1. 事件处理
            //    - WindowResize 驱动 engine.resize()（引擎缓冲须先于布局更新）
            //      这是 Window 层的正当编排职责，不是层混叠
            //    - WindowMaximize/Minimize/Restore 调用对应平台方法并
            //      记录窗口状态变更（widget 树可监听 WidgetEvent 获取通知）
            //    - 其余事件通过 map_event 映射后分发给 widget 树
            // ═══════════════════════════════════════════════════════════
            _tmark!("events");
            for ev in pending_events.borrow_mut().drain(..) {
                    if let UiEventType::WindowResize = ev.type_ {
                    if let UiEventPayload::Resize(ref d) = ev.payload {
                        if d.width > 0 && d.height > 0 {
                            engine.resize(d.width, d.height);
                        }
                    }
                }
                // 调试模式切换：F12 键
                if let UiEventType::KeyDown = ev.type_ {
                    use crate::base::KeyCode;
                    if let UiEventPayload::Key(ref data) = ev.payload {
                        if data.key == KeyCode::F12 {
                            let new_val = !self.debug_mode.get();
                            self.debug_mode.set(new_val);
                            log::info!("[Debug] 调试模式 {}", if new_val { "开启" } else { "关闭" });
                            continue;
                        }
                    }
                }
                if let Some(we) = map_event(&ev) {
                    log::trace!("dispatch: {:?}", we);
                    tree.dispatch_event(&we);
                }
            }

            // ═══════════════════════════════════════════════════════════
            // 2. 动画推进 + 布局
            //    resize 事件已更新 root frame → layout 自动传播至子节点
            // ═══════════════════════════════════════════════════════════
            let now = Instant::now();
            let dt = (now - last_frame).as_secs_f32().min(0.05);
            last_frame = now;
            let t0 = Instant::now();
            keep_polling = tree.update(dt) || woke;
            let t1 = Instant::now();
            if t1 - t0 > std::time::Duration::from_millis(100) {
                log::warn!("EventLoop: tree.update took {}ms", (t1 - t0).as_millis());
            }
            tree.layout();
            let t2 = Instant::now();
            if t2 - t1 > std::time::Duration::from_millis(100) {
                log::warn!("EventLoop: tree.layout took {}ms", (t2 - t1).as_millis());
            }

            // ═══════════════════════════════════════════════════════════
            // 3. 引擎尺寸 ↔ root frame 同步保障
            //    引擎尺寸是窗口真实尺寸的事实来源。当 resize 事件因平台差异
            //    未送达时（如无边框窗口拖拽缩放），此步确保根节点对齐。
            // ═══════════════════════════════════════════════════════════
            let need_relayout = tree.root_id().and_then(|rid| tree.get(rid)).map_or(false, |root| {
                let engine_w = engine.width() as f32;
                let engine_h = engine.height() as f32;
                let rf = root.frame();
                (rf.w - engine_w).abs() > 0.5 || (rf.h - engine_h).abs() > 0.5
            });
            if need_relayout {
                if let Some(rid) = tree.root_id() {
                    if let Some(root_mut) = tree.get_mut(rid) {
                        root_mut.set_frame(Rect::new(
                            0.0, 0.0,
                            engine.width() as f32,
                            engine.height() as f32,
                        ));
                    }
                }
                tree.mark_full_frame_dirty();
                tree.layout();
                // 递增 tree_version 触发 LayerTree 重建，确保 Picture 节点使用新 bounds
                tree.tree_version += 1;
            }

            let t_frame = Instant::now();
            on_frame(tree, engine, self.platform.as_mut());
            let t3 = Instant::now();
            if t3 - t_frame > std::time::Duration::from_millis(100) {
                log::warn!("EventLoop: on_frame took {}ms", (t3 - t_frame).as_millis());
            }
            // ── FrameGraph 驱动渲染（替代 engine.render_frame） ────
            let dirty_region = tree.dirty_region();
            let first_render = !rendered_first_frame;
            let need_render = first_render
                || !self.rendered_first
                || dirty_region.full_frame
                || dirty_region.clear_required
                || keep_polling;

            _tmark!("pre-render");

            let outcome = if !need_render {
                RenderOutcome::Idle
            } else {
                let region = if first_render || dirty_region.full_frame {
                    DirtyRegion::full()
                } else {
                    dirty_region.clone()
                };

                let scroll_deltas = tree.drain_scroll_deltas();

                let damage: Option<(i32, i32, i32, i32)> = if region.full_frame {
                    None
                } else {
                    let bounds = region.bounds();
                    Some((
                        bounds.x as i32,
                        bounds.y as i32,
                        bounds.w as i32,
                        bounds.h as i32,
                    ))
                };

                // 滚动偏移（先于清理，避免滚动携带清除后的透明像素）
                for &(viewport, dx, dy) in &scroll_deltas {
                    engine.scroll_region(viewport, dx, dy);
                }

                // ── 构建帧图 Pass（仅首次注册，复用 PassId 使 Compiler 版本追踪生效）──
                if self.geom_pass_id.is_none() {
                    let gid = self.frame_graph.add_pass("Geometry", |b| {
                        b.writes(&[self.main_color_res])
                    });
                    let oid = self.frame_graph.add_pass("Overlay", |b| {
                        b.reads(&[self.main_color_res])
                            .writes(&[self.main_color_res])
                    });
                    self.geom_pass_id = Some(gid);
                    self.over_pass_id = Some(oid);
                }
                let geom_pid = self.geom_pass_id.unwrap();
                let over_pid = self.over_pass_id.unwrap();

                let plan = self.frame_graph.compile();

                if plan.zero_frame_cost {
                    RenderOutcome::Idle
                } else {
                    // LayerTree 构建：检测 widget 树结构变化
                    let cur_version = tree.tree_version();
                    if self.last_tree_version != cur_version {
                        layer_tree.build(tree);
                        // 释放未被新树复用的旧离屏缓冲
                        layer_tree.sweep_orphaned_offscreens(engine);
                        self.last_tree_version = cur_version;
                    }
                    layer_tree.update_dirty(tree);

                    // 预处理：收集所有需写入的资源（避免 passes() 和 mark_resource_dirty 的借用冲突）
                    let pass_info: std::collections::HashMap<_, _> =
                        self.frame_graph.passes().iter()
                            .map(|p| (p.id, p.writes.clone()))
                            .collect();
                    let resources_to_bump: Vec<ResourceId> = plan.execution_order.iter()
                        .filter_map(|&pid| pass_info.get(&pid))
                        .flat_map(|writes| writes.iter().copied())
                        .collect();

                    // 按 FrameGraph 执行计划遍历 Pass
                    for &pid in &plan.execution_order {
                        if pid == geom_pid {
                            // ── Pass 1: Geometry ──
                            let t_render = Instant::now();
                            engine.begin_frame(&region);
                            let lt_ref = theme.borrow();
                            let tokens = lt_ref.tokens();
                            let lt_font = engine.font_service().loaded_font_handle;
                            layer_tree.render(engine, tree, tokens, lt_font);
                            drop(lt_ref);
                            engine.end_frame(&region);
                            if t_render.elapsed() > std::time::Duration::from_millis(100) {
                                log::warn!("EventLoop: Geometry pass took {}ms", t_render.elapsed().as_millis());
                            }
                        } else if pid == over_pid {
                            // ── Pass 2: Overlay ──
                            let t_over = Instant::now();
                            let overlay_region = DirtyRegion::empty();
                            engine.begin_frame(&overlay_region);
                            let theme_ref = theme.borrow();
                            let tokens = theme_ref.tokens();
                            let lt_font = engine.font_service().loaded_font_handle;
                            layer_tree.render_overlays(engine, tree, tokens, lt_font, self.debug_mode.get());
                            drop(theme_ref);
                            engine.end_frame(&overlay_region);
                            if t_over.elapsed() > std::time::Duration::from_millis(100) {
                                log::warn!("EventLoop: Overlay pass took {}ms", t_over.elapsed().as_millis());
                            }
                        }
                    }

                    // 提升写入资源版本（FrameGraph 版本追踪，用于后续帧的 Pass 裁剪）
                    for res in resources_to_bump {
                        self.frame_graph.mark_resource_dirty(res);
                    }

                    tree.reset_dirty();
                    self.rendered_first = true;
                    RenderOutcome::Present(damage)
                }
            };

            // ── 呈现 ──
            match outcome {
                RenderOutcome::Present(damage) => {
                    let t_present = Instant::now();
                    let dirty = if rendered_first_frame { damage } else { None };
                    let _ = self.platform.presenter().present(
                        engine.pixels(),
                        engine.width(),
                        engine.height(),
                        dirty,
                    );
                    rendered_first_frame = true;
                    if t_present.elapsed() > std::time::Duration::from_millis(100) {
                        log::warn!("EventLoop: present took {}ms", t_present.elapsed().as_millis());
                    }
                }
                RenderOutcome::Idle => {}
            }

            log::debug!("EventLoop: iteration end, dt={:.1}ms", last_frame.elapsed().as_secs_f32() * 1000.0);

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
            self.platform.window_properties().width(),
            self.platform.window_properties().height()
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
            self.platform.window_manager().destroy_window();
        }
        self.running = false;
    }
}

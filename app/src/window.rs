// ============================================================================
// app/window.rs — 基于 Platform 的事件驱动窗口
//
// 核心设计：
//   - 组合 `Box<dyn Platform>`（共享资源） + `Box<dyn PlatformWindow>`（窗口操作）
//   - 多窗口架构：每个 Window 持有独立的 PlatformWindow，共享同一个 Platform
//   - 不重复维护窗口状态（尺寸、最小化等），全部通过 PlatformWindow trait 获取
//   - 提供 run() 事件循环 + 帧回调机制，渲染委托给引擎
//   - FrameGraph（帧图） 驱动渲染 Pass 编排：自动裁剪未变化的 Pass
// ============================================================================

use std::cell::Cell;
use std::cell::RefCell;
use std::time::{Duration, Instant};

use uix_core::{Point, Rect};
use uix_graphics::frame_graph::resource::ResourceId;
use uix_graphics::frame_graph::FrameGraph;
use uix_ui::layer::LayerTree;
use uix_graphics::{DirtyRegion, GraphicsEngine, RenderOutcome};
use uix_platform::event::{UiEvent, UiEventPayload, UiEventType};
use uix_platform::{IClipboard, Platform, PlatformWindow};

use uix_ui::theme::Theme;
use uix_ui::widget::{WidgetCore, WidgetEvent, WidgetTree};

/// Event-driven application window.
///
/// 持有 FrameGraph（帧图）驱动渲染 Pass 编排。
/// `platform` 为共享平台资源（事件循环/显示器/剪贴板），
/// `window` 为当前窗口操作（呈现/属性/显隐）。
pub struct Window {
    platform: Box<dyn Platform>,
    window: Option<Box<dyn PlatformWindow>>,
    running: bool,
    exit_code: i32,
    /// 帧图——渲染 Pass 编排器。
    frame_graph: FrameGraph,
    /// 主帧缓冲资源 ID（FrameGraph 资源注册）。
    main_color_res: ResourceId,
    /// 窗口初始尺寸（用于最大化还原时恢复）
    initial_size: (i32, i32),
    /// 首帧已渲染标志。
    rendered_first: bool,
    /// 上次渲染时的 WidgetTree 版本号（用于检测结构变更）。
    last_tree_version: u64,
    /// 调试模式开关（F12 切换），开启后在 overlay 层绘制调试边框和信息。
    debug_mode: Cell<bool>,
    /// 当前光标位置（用于调试模式悬浮高亮）。
    cursor_pos: Cell<Point>,
    /// Geometry Pass 的 ID（注册一次，复用）。
    geom_pass_id: Option<uix_graphics::frame_graph::resource::PassId>,
    /// Overlay Pass 的 ID（注册一次，复用）。
    over_pass_id: Option<uix_graphics::frame_graph::resource::PassId>,
}

impl Window {
    pub fn new(platform: Box<dyn Platform>) -> Self {
        let mut fg = FrameGraph::new();
        let main_color = fg.register_texture("MainColor", 800, 600);
        let debug_mode = std::env::var("UIX_DEBUG").is_ok();
        if debug_mode {
            log::info!("[Debug] UIX_DEBUG 环境变量已设置，调试模式默认开启");
        }
        Self {
            platform,
            window: None,
            running: false,
            exit_code: 0,
            frame_graph: fg,
            main_color_res: main_color,
            initial_size: (800, 600),
            rendered_first: false,
            last_tree_version: 0,
            debug_mode: Cell::new(debug_mode),
            cursor_pos: Cell::new(Point::new(0.0, 0.0)),
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
    /// 获取当前窗口句柄（直接访问内部 Option）。
    /// 如需借用，使用 as_deref/as_deref_mut 从 Option<Box<...>> 转换。
    pub fn window_box(&self) -> &Option<Box<dyn PlatformWindow>> {
        &self.window
    }

    /// 创建平台窗口，存储 PlatformWindow 句柄。
    pub fn create(&mut self, title: &str, width: i32, height: i32) -> bool {
        self.initial_size = (width, height);
        match self.platform.window_manager().create_window(title, width, height) {
            Ok(mut w) => {
                w.center_on_screen();
                w.show();
                w.raise();
                log::info!(
                    "Window created and shown ({}x{}, title='{}')",
                    width, height, title
                );
                self.window = Some(w);
                true
            }
            Err(e) => {
                log::error!("Window::create: platform failed: {}", e.short_what());
                false
            }
        }
    }

    pub fn show(&mut self) {
        if let Some(ref mut w) = self.window { w.show(); }
    }
    pub fn close(&mut self) {
        self.running = false;
        if let Some(ref mut w) = self.window { w.close(); }
    }
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// 设置调试模式。
    pub fn set_debug_mode(&self, mode: bool) {
        self.debug_mode.set(mode);
        log::info!("[Debug] 调试模式 {}", if mode { "开启" } else { "关闭" });
    }

    /// 查询当前是否处于调试模式。
    pub fn debug_mode(&self) -> bool {
        self.debug_mode.get()
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

        // 注入剪贴板指针（拆分为 data+vtable 以断开 borrow），
        // 使 widget 可通过 Ctrl+C 复制文本。
        {
            let c: &mut dyn IClipboard = self.platform.clipboard();
            let wide: *mut dyn IClipboard = c;
            let parts: (usize, usize) = unsafe { std::mem::transmute(wide) };
            uix_ui::clipboard::set_clipboard_parts(parts.0, parts.1);
        }

        let running_flag = Cell::new(true);
        let pending_events = std::cell::RefCell::new(Vec::<UiEvent>::new());
        let mut first_frame = true;
        let mut rendered_first_frame = false;
        let mut last_frame = Instant::now();
        let mut keep_polling = false;
        let mut idle_count: u32 = 0;
        let mut window_visible = true;
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
            // ── 事件收集 ──
            if keep_polling {
                // 动画帧：阻塞等待（Linux: frame callback; Windows: MsgWait 超时）
                if !self.platform.event_loop().wait_event(&collect) { break; }
                self.platform.event_loop().poll_event(&collect);
            } else if first_frame {
                if !self.platform.event_loop().poll_event(&collect) { break; }
                first_frame = false;
            } else {
                // 空闲：阻塞等待，长时间无事件后加长超时节流
                if idle_count > 3 {
                    self.platform.event_loop().wait_timeout(
                        std::time::Duration::from_millis(100), &collect);
                } else {
                    if !self.platform.event_loop().wait_event(&collect) { break; }
                }
                self.platform.event_loop().poll_event(&collect);
            }

            #[cfg(debug_assertions)]
            let _frame_t0 = std::time::Instant::now();
            #[cfg(debug_assertions)]
            let mut _last_tmark = _frame_t0;
            #[cfg(debug_assertions)]
            macro_rules! _tmark {
                ($label:expr) => {{
                    let elapsed = _frame_t0.elapsed();
                    let since_last = _last_tmark.elapsed();
                    if since_last.as_secs_f32() > 0.1 {
                        log::debug!("[TIMING] {}: total={:.1}s  step={:.1}s", $label, elapsed.as_secs_f32(), since_last.as_secs_f32());
                    }
                    _last_tmark = std::time::Instant::now();
                }};
            }
            #[cfg(not(debug_assertions))]
            macro_rules! _tmark {
                ($label:expr) => {{}};
            }

            _tmark!("events");
            let had_events = !pending_events.borrow().is_empty();
            for ev in pending_events.borrow_mut().drain(..) {
                log::trace!("[EventLoop] process ev={:?}", ev.type_);
                if let UiEventType::WindowResize = ev.type_ {
                    if let UiEventPayload::Resize(ref d) = ev.payload {
                        if d.width > 0 && d.height > 0 {
                            engine.resize(d.width, d.height);
                            if let Some(ref mut w) = self.window {
                                w.resize_notify(d.width, d.height);
                            }
                        }
                    }
                }
                if let UiEventType::WindowMaximize = ev.type_ {
                    if engine.width() != self.initial_size.0 || engine.height() != self.initial_size.1 {
                        // 引擎已通过 resize 事件调整了尺寸
                    } else {
                        let info = self.platform.display().info(0);
                        let w = info.bounds.w as i32;
                        let h = info.bounds.h as i32;
                        if w > 0 && h > 0 {
                            log::debug!("[Window] Maximize fallback resize to {}x{}", w, h);
                            engine.resize(w, h);
                            if let Some(ref mut win_ref) = self.window {
                                win_ref.resize_notify(w, h);
                            }
                        }
                    }
                }
                if let UiEventType::WindowRestore = ev.type_ {
                    let (restore_w, restore_h) = self.initial_size;
                    log::debug!("[Window] Restore resize to {}x{}", restore_w, restore_h);
                    engine.resize(restore_w, restore_h);
                    if let Some(ref mut w) = self.window {
                        w.resize_notify(restore_w, restore_h);
                    }
                    window_visible = true;
                    log::debug!("[Visible] 窗口恢复可见");
                }
                if let UiEventType::WindowMinimize = ev.type_ {
                    window_visible = false;
                    log::debug!("[Visible] 窗口最小化，暂停渲染");
                }
                if let UiEventType::MouseMove = ev.type_ {
                    if let UiEventPayload::MouseMove(ref data) = ev.payload {
                        self.cursor_pos.set(data.pos);
                    }
                }
                if let UiEventType::KeyDown = ev.type_ {
                    use uix_core::KeyCode;
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
            // 有事件到达时重置空闲计数，保持响应速度
            if had_events {
                idle_count = 0;
            }

            let now = Instant::now();
            let dt = (now - last_frame).as_secs_f32().min(0.05);
            last_frame = now;
            let t0 = Instant::now();
            // keep_polling 仅反映 widget 是否需要持续更新（动画等），
            // woke 不参与此判断以避免空闲时陷入 60fps 轮询循环。
            keep_polling = tree.update(dt);
            let t1 = Instant::now();
            if t1 - t0 > std::time::Duration::from_millis(100) {
                log::warn!("EventLoop: tree.update took {}ms", (t1 - t0).as_millis());
            }

            // 仅在有事件、持续更新、或首帧时才 layout/on_frame。
            // 窗口最小化/隐藏时跳过。
            let needs_work = window_visible && (had_events || keep_polling || idle_count == 0);
            if needs_work {
                tree.layout();
                let t2 = Instant::now();
            if t2 - t1 > std::time::Duration::from_millis(100) {
                log::warn!("EventLoop: tree.layout took {}ms", (t2 - t1).as_millis());
            }

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
                tree.tree_version += 1;
            }

            let t_frame = Instant::now();
            on_frame(tree, engine, self.platform.as_mut());
            let t3 = Instant::now();
            if t3 - t_frame > std::time::Duration::from_millis(100) {
                log::warn!("EventLoop: on_frame took {}ms", (t3 - t_frame).as_millis());
            }
            } // needs_work

            let dirty_region = tree.dirty_region();
            let first_render = !rendered_first_frame;
            // 窗口最小化/隐藏时暂停渲染和动画
            let need_render = window_visible && (
                first_render
                || !self.rendered_first
                || !dirty_region.is_empty()
                || keep_polling
            );

            _tmark!("pre-render");

            log::debug!("[EventLoop] need_render={} first_render={} rendered_first={} full_frame={} clear={} keep_polling={} ev_count={}",
                need_render, first_render, rendered_first_frame,
                dirty_region.full_frame, dirty_region.clear_required, keep_polling,
                pending_events.borrow().len());

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
                        bounds.x as i32, bounds.y as i32,
                        bounds.w as i32, bounds.h as i32,
                    ))
                };

                for &(viewport, dx, dy) in &scroll_deltas {
                    engine.scroll_region(viewport, dx, dy);
                }

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
                let geom_pid = match self.geom_pass_id {
                    Some(id) => id,
                    None => {
                        log::error!("[EventLoop] geom_pass_id not initialized");
                        return 1;
                    }
                };
                let over_pid = match self.over_pass_id {
                    Some(id) => id,
                    None => {
                        log::error!("[EventLoop] over_pass_id not initialized");
                        return 1;
                    }
                };

                let plan = self.frame_graph.compile();

                if plan.zero_frame_cost {
                    RenderOutcome::Idle
                } else {
                    let cur_version = tree.tree_version();
                    if self.last_tree_version != cur_version {
                        layer_tree.build(tree);
                        layer_tree.sweep_orphaned_offscreens(engine);
                        self.last_tree_version = cur_version;
                    }
                    layer_tree.update_dirty(tree);

                    let pass_info: std::collections::HashMap<_, _> =
                        self.frame_graph.passes().iter()
                            .map(|p| (p.id, p.writes.clone()))
                            .collect();
                    let resources_to_bump: Vec<ResourceId> = plan.execution_order.iter()
                        .filter_map(|&pid| pass_info.get(&pid))
                        .flat_map(|writes| writes.iter().copied())
                        .collect();

                    for &pid in &plan.execution_order {
                        if pid == geom_pid {
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
                            let t_over = Instant::now();
                            let overlay_region = DirtyRegion::empty();
                            engine.begin_frame(&overlay_region);
                            let theme_ref = theme.borrow();
                            let tokens = theme_ref.tokens();
                            let lt_font = engine.font_service().loaded_font_handle;
                            let hover_pos = if self.debug_mode.get() { Some(self.cursor_pos.get()) } else { None };
                            layer_tree.render_overlays(engine, tree, tokens, lt_font, self.debug_mode.get(), hover_pos, &region);
                            drop(theme_ref);
                            engine.end_frame(&overlay_region);
                            if t_over.elapsed() > std::time::Duration::from_millis(100) {
                                log::warn!("EventLoop: Overlay pass took {}ms", t_over.elapsed().as_millis());
                            }
                        }
                    }

                    for res in resources_to_bump {
                        self.frame_graph.mark_resource_dirty(res);
                    }

                    tree.reset_dirty();
                    self.rendered_first = true;
                    RenderOutcome::Present(damage)
                }
            };

            match outcome {
                RenderOutcome::Present(damage) => {
                    idle_count = 0;
                    let t_present = Instant::now();
                    let dirty = if rendered_first_frame { damage } else { None };
                    if let Some(ref mut w) = self.window {
                        if let Err(e) = w.presenter().present(
                            engine.pixels(),
                            engine.width(),
                            engine.height(),
                            dirty,
                        ) {
                            log::error!("[EventLoop] present failed: {}", e.short_what());
                        }
                    }
                    rendered_first_frame = true;
                    if t_present.elapsed() > std::time::Duration::from_millis(100) {
                        log::warn!("EventLoop: present took {}ms", t_present.elapsed().as_millis());
                    }
                }
                RenderOutcome::Idle => {
                    idle_count = idle_count.saturating_add(1);
                }
            }

            log::debug!("EventLoop: iteration end, dt={:.1}ms", last_frame.elapsed().as_secs_f32() * 1000.0);

            // 动画帧由平台帧节奏驱动（Linux: compositor frame callback; Windows: MsgWait 超时）
            if keep_polling {
                if !self.platform.event_loop().wait_event(&collect) { break; }
                self.platform.event_loop().poll_event(&collect);
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
        if let Some(ref w) = self.window {
            log::info!(
                "Window event loop started ({}x{})",
                w.properties().width(),
                w.properties().height()
            );
        }
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
            if let Some(ref mut w) = self.window { w.close(); }
        }
        self.running = false;
    }
}

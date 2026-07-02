//! Render Loop — Widget 渲染事件循环。
//!
//! # 事件流
//!
//! ```text
//! OS 事件 → IEventLoop 轮询 → 内联 collect closure → pending_events
//!                                                       ↓
//!              事件处理 → WidgetTree.dispatch_event() → bus.publish(ev) → 外部订阅者
//! ```
//!
//! RenderLoop 内部使用内联 closure 收集事件到 pending_events 队列，
//! 处理完毕后通过 `bus.publish()` 广播到平台层 EventBus，供其他层订阅。
//!
//! 外部层可通过 `platform.event_bus().subscribe(...)` 直接订阅事件，
//! 无需侵入 render loop。
//!
//! # 增量渲染管线
//!
//! 每帧渲染遵循「只在变动处绘制」原则：
//!
//! ```text
//! tree.update(dt)
//!   ├─ on_update() 推进动画/滚动
//!   ├─ dirty_rect() 计算变化区域 → 加入 dirty_region
//!   ├─ scroll_delta() 收集滚动增量 → dirty.scroll_deltas
//!   └─ 返回 keep_polling（有动画/滚动进行中）
//!       ↓
//! tree.layout() —— 仅 dirty_traverse 遍历脏子树
//!       ↓
//! Geometry Pass:
//!   ├─ drain_scroll_deltas() → canvas.scroll_region() 移动已有像素
//!   ├─ begin_frame(DirtyRects) — 只清除脏区域
//!   ├─ layer_tree.render() — 裁剪到脏区域，只绘制相交 widget
//!   └─ end_frame()
//!       ↓
//! Overlay Pass:
//!   ├─ begin_frame(Overlay) — 不清除，叠加绘制
//!   └─ layer_tree.render_overlays() — 仅脏区域内 post_render
//!       ↓
//! tree.reset_dirty() → 准备下一帧
//! ```
//!
//! # 滚动优化（像素移动）
//!
//! ScrollView 滚动时不触发全帧重绘：
//! 1. `scroll_delta` 返回帧间偏移量 (dx, dy)
//! 2. `drain_scroll_deltas` 在 begin_frame 前消费这些偏移
//! 3. `canvas.scroll_region(viewport, dx, dy)` memmove 像素缓冲
//! 4. `dirty_rect` 只返回新暴露的 strip 区域
//! 5. `DirtyRects(strip)` 只清除并重绘 strip
//! 6. 视口内非 strip 区域的内容通过像素移动保留，无需重绘
//!
//! # FrameGraph 裁剪
//!
//! FrameGraph 追踪资源版本号。当输入未变化且输出无消费时，
//! Pass 被自动裁剪（culled），`zero_frame_cost = true` 跳过整帧渲染。
//! 动画/滚动持续时 `mark_resource_dirty` 通知 FrameGraph 资源有变化，
//! 防止 Pass 被误裁剪。

use std::cell::{Cell, RefCell};
use std::time::Instant;

use uix_graphics::frame_graph::resource::{PassId, ResourceId};
use uix_graphics::frame_graph::FrameGraph;
use uix_graphics::{DirtyRegion, GraphicsEngine, RenderOutcome, UpdateStrategy};
use uix_platform::event::{UiEvent, UiEventPayload, UiEventType};
use uix_platform::{Platform, PlatformWindow, Point, Rect};

use crate::clipboard;
use crate::layer::LayerTree;
use crate::theme::Theme;
use crate::widget::{WidgetCore, WidgetEvent, WidgetTree};
use uix_graphics::font_service::FontService;

/// 运行完整的 widget 渲染事件循环。
#[allow(clippy::too_many_arguments)]
pub fn run_widget_loop<M, X, F>(
    platform: &mut dyn Platform,
    platform_window: &mut dyn PlatformWindow,
    engine: &mut dyn GraphicsEngine,
    tree: &mut WidgetTree,
    font_service: &FontService,
    theme: &RefCell<Theme>,
    debug_mode: &Cell<bool>,
    cursor_pos: &Cell<Point>,
    map_event: M,
    on_exit: X,
    on_frame: F,
) -> i32
where
    M: Fn(&UiEvent) -> Option<WidgetEvent>,
    X: Fn(&UiEvent) -> bool,
    F: Fn(&mut WidgetTree, &mut dyn GraphicsEngine, &mut dyn Platform),
{
    let mut frame_graph = FrameGraph::new();
    let main_color_res = frame_graph.register_texture("MainColor", 800, 600);

    // 从 platform 提前取出 event_bus 原始指针——避免 collect closure 中双重可变借用。
    // Safety: platform 在 closure 的整个生命周期内存活且不被别名访问。
    let bus_ptr: *mut dyn Platform = platform as *mut dyn Platform;

    let pending_events = RefCell::new(Vec::<UiEvent>::new());
    let mut first_frame = true;
    let mut rendered_first = false;
    let mut last_frame = Instant::now();
    let mut keep_polling = false;
    let mut idle_count: u32 = 0;
    let mut window_visible = true;
    let mut layer_tree = LayerTree::new();
    let mut last_tree_version: u64 = 0;
    let mut geom_pass_id: Option<PassId> = None;
    let mut over_pass_id: Option<PassId> = None;
    let mut initial_size = (
        platform_window.properties().width(),
        platform_window.properties().height(),
    );

    // 注入剪贴板指针
    {
        let c: &mut dyn uix_platform::IClipboard = platform.clipboard();
        let wide: *mut dyn uix_platform::IClipboard = c;
        let parts: (usize, usize) = unsafe { std::mem::transmute(wide) };
        clipboard::set_clipboard_parts(parts.0, parts.1);
    }

    let running = Cell::new(true);

    // 内联事件收集 closure
    let collect = |ev: &UiEvent| {
        match ev.type_ {
            UiEventType::WindowClose => {
                running.set(false);
                return false;
            }
            _ => {
                if on_exit(ev) {
                    running.set(false);
                    return false;
                }
            }
        }
        pending_events.borrow_mut().push(ev.clone());
        true
    };

    while running.get() {
        platform.text_input().start();

        // ── 事件轮询（内联 collect closure）──
        if keep_polling {
            if !platform.event_loop().wait_event(&collect) {
                break;
            }
            platform.event_loop().poll_event(&collect);
        } else if first_frame {
            if !platform.event_loop().poll_event(&collect) {
                break;
            }
            first_frame = false;
        } else {
            if idle_count > 3 {
                platform.event_loop().wait_timeout(
                    std::time::Duration::from_millis(100),
                    &collect,
                );
            } else {
                if !platform.event_loop().wait_event(&collect) {
                    break;
                }
            }
            platform.event_loop().poll_event(&collect);
        }

        let had_events = !pending_events.borrow().is_empty();

        // MouseMove 是最高频事件（每次鼠标滑动都触发），但它不需要触发 layout——
        // layout 只在树结构变化、resize 或需要重新计算大小时才需要。
        // MouseMove 只更新光标位置和 hover 状态，不改变树结构。
        // 分离 had_layout_event 可避免每帧无用 layout + 全帧渲染拖慢滚动。
        let mut had_layout_event = false;

        for ev in pending_events.borrow_mut().drain(..) {
            let is_layout_event = !matches!(ev.type_, UiEventType::MouseMove);
            if is_layout_event {
                had_layout_event = true;
            }

            // ── 窗口事件处理 ──
            match ev.type_ {
                UiEventType::WindowResize => {
                    if let UiEventPayload::Resize(ref d) = ev.payload {
                        if d.width > 0 && d.height > 0 {
                            engine.resize(d.width, d.height);
                            platform_window.resize_notify(d.width, d.height);
                            initial_size = (d.width, d.height);
                        }
                    }
                }
                UiEventType::WindowMaximize => {
                    if engine.canvas_2d().width() != initial_size.0
                        || engine.canvas_2d().height() != initial_size.1
                    {
                        // 已通过 resize 事件调整
                    } else {
                        let info = platform.display().info(0);
                        let w = info.bounds.w as i32;
                        let h = info.bounds.h as i32;
                        if w > 0 && h > 0 {
                            engine.resize(w, h);
                            platform_window.resize_notify(w, h);
                        }
                    }
                }
                UiEventType::WindowRestore => {
                    let (rw, rh) = initial_size;
                    engine.resize(rw, rh);
                    platform_window.resize_notify(rw, rh);
                    window_visible = true;
                }
                UiEventType::WindowMinimize => {
                    window_visible = false;
                }
                UiEventType::MouseMove => {
                    if let UiEventPayload::MouseMove(ref data) = ev.payload {
                        cursor_pos.set(data.pos);
                    }
                }
                UiEventType::KeyDown => {
                    use uix_platform::KeyCode;
                    if let UiEventPayload::Key(ref data) = ev.payload {
                        if data.key == KeyCode::F12 {
                            let new_val = !debug_mode.get();
                            debug_mode.set(new_val);
                            continue;
                        }
                    }
                }
                _ => {}
            }

            // ── 映射为 WidgetEvent 并分发 ──
            if let Some(we) = map_event(&ev) {
                tree.dispatch_event(&we);
            }

            // ── 广播到 EventBus（供外部订阅者）──
            unsafe {
                (*bus_ptr).event_bus().publish(&ev);
            }
        }
        if had_events {
            idle_count = 0;
        }

        // ── 帧推进 ──
        let now = Instant::now();
        let dt = (now - last_frame).as_secs_f64().min(0.05);
        last_frame = now;
        keep_polling = tree.update(dt);

        // 仅在以下情况触发 layout：
        // - 有布局事件（resize/点击/键盘等，排除 MouseMove）
        // - 有动画/滚动运行（keep_polling）
        // - 首帧（尚未 rendered_first）
        // MouseMove 不改变树结构，无需 layout；
        // 其带来的 hover 视觉变化通过 mark_dirty() 直接设置脏矩形，
        // 由下方的 need_render 独立处理渲染。
        let needs_work = window_visible && (had_layout_event || keep_polling || !rendered_first);

        if needs_work {
            let before_version = tree.tree_version();
            tree.layout();

            // 根 frame 与引擎画布尺寸同步（on_frame 前后均需检查，
            // 因为 on_frame 可能改变树结构导致根 frame 失配）
            sync_root_frame_to_engine(tree, engine);
            on_frame(tree, engine, platform);
            sync_root_frame_to_engine(tree, engine);

            // on_frame 可能重建整棵树（主题切换），此时必须强制全帧渲染。
            // 重建后 mark_full_frame_dirty 已设置全帧脏区域，
            // 但 FrameGraph culling 可能因版本追踪将 Pass 裁剪掉，
            // 导致画面停留在重建前的帧。强制刷新确保重建生效。
            if tree.tree_version() != before_version {
                // ⚠️  重建后必须重新 layout：set_root 只设置了根节点的 frame，
                // 所有子节点的 frame 为 Rect::zero()，不 layout 则组件位置混乱。
                // sync_root_frame_to_engine 之后调用 layout 确保根 frame 已正确同步到画布。
                tree.layout();
                tree.mark_full_frame_dirty();
            }
        }

        let dirty_region = tree.dirty_region();
        let need_render = window_visible
            && (!rendered_first || !dirty_region.is_empty() || keep_polling);

        let outcome = if !need_render {
            RenderOutcome::Idle
        } else {
            let region = if !rendered_first || dirty_region.full_frame {
                DirtyRegion::full()
            } else {
                dirty_region.clone()
            };

            if geom_pass_id.is_none() {
                let gid = frame_graph.add_pass("Geometry", |b| b.writes(&[main_color_res]));
                let oid = frame_graph.add_pass("Overlay", |b| {
                    b.reads(&[main_color_res]).writes(&[main_color_res])
                });
                geom_pass_id = Some(gid);
                over_pass_id = Some(oid);
            }

            // 通知 FrameGraph 资源有变化，防止 pass 被裁剪。
            // 必须同时覆盖动画（keep_polling）和脏区域（鼠标/键盘事件导致的 hover 变化等）
            // 两个场景，否则 FrameGraph 认为资源版本未变而裁剪 Pass，
            // 导致脏区域的视觉更新（hover 高亮、ripple、焦点框等）丢失。
            frame_graph.mark_resource_dirty(main_color_res);

            let plan = frame_graph.compile();

            if plan.zero_frame_cost {
                // FrameGraph 认为本轮无需渲染，脏数据已被消费，清理后返回空闲。
                tree.reset_dirty();
                RenderOutcome::Idle
            } else {
                let cur_version = tree.tree_version();
                if last_tree_version != cur_version {
                    layer_tree.build(tree);
                    layer_tree.sweep_orphaned_offscreens(engine);
                    last_tree_version = cur_version;
                }
                layer_tree.update_dirty(tree);

                // ── 滚动偏移像素移动（提前执行，确保 present 前像素已移位）──
                // scroll_region memmove 修改了视口内全部像素（非仅 strip），
                // 必须在 present 前将变化提交到 DIB/窗口，否则非 strip 区域
                // 残留旧帧像素，导致滚动画面撕裂。
                let scroll_move = tree.drain_scroll_region_move();
                if let Some((frame, dx, dy)) = scroll_move {
                    engine.canvas_2d().scroll_region(frame, dx, dy);
                }

                // damage rect：滚动时将 ScrollView 视口纳入 damage，
                // 确保 present 提交 memmove 引起的全视口像素变化。
                let damage: Option<(i32, i32, i32, i32)> = if region.full_frame {
                    None
                } else {
                    let bounds = if let Some((frame, _, _)) = scroll_move {
                        region.bounds().union(&frame)
                    } else {
                        region.bounds()
                    };
                    Some((
                        (bounds.x - 1.0).max(0.0) as i32,
                        (bounds.y - 1.0).max(0.0) as i32,
                        (bounds.w + 2.0) as i32,
                        (bounds.h + 2.0) as i32,
                    ))
                };

                let pass_info: std::collections::HashMap<_, _> = frame_graph
                    .passes()
                    .iter()
                    .map(|p| (p.id, p.writes.clone()))
                    .collect();
                let resources_to_bump: Vec<ResourceId> = plan
                    .execution_order
                    .iter()
                    .filter_map(|&pid| pass_info.get(&pid))
                    .flat_map(|writes| writes.iter().copied())
                    .collect();

                for &pid in &plan.execution_order {
                    if Some(pid) == geom_pass_id {
                        // 选择更新策略：首次帧或全帧脏时用 FullRedraw，
                        // 增量帧用 DirtyRects（只清除并重绘脏区域）
                        let strategy = if !rendered_first || region.full_frame {
                            UpdateStrategy::FullRedraw
                        } else {
                            UpdateStrategy::DirtyRects(region.rects().to_vec())
                        };
                        engine.begin_frame(strategy);

                        let lt_ref = theme.borrow();
                        let tokens = lt_ref.tokens();
                        let lt_font = font_service.loaded_font_handle;
                        layer_tree.render(engine, tree, tokens, lt_font, font_service);
                        drop(lt_ref);
                        engine.end_frame();
                    } else if Some(pid) == over_pass_id {
                        // 使用 Overlay 策略——不清除画布，在 Geometry Pass
                        // 的渲染结果上叠加 overlay 内容。FullRedraw 会清空画布
                        // 导致几何渲染内容被擦除（黑屏）。
                        //
                        // clear_required 表示是否有实际脏区域需要清空后重绘。
                        // 当 clear_required=false 时（如仅动画触发的渲染），
                        // 传空 vec 避免不必要的 overlay 清空。
                        let overlay_rects: Vec<Rect> = if region.clear_required {
                            if region.full_frame {
                                let w = engine.canvas_2d().width() as f32;
                                let h = engine.canvas_2d().height() as f32;
                                vec![Rect::new(0.0, 0.0, w, h)]
                            } else {
                                let mut rects = region.rects().to_vec();
                                // 滚动时将 ScrollView 视口帧纳入 overlay 区域，
                                // 否则 Overlay clip 限制在 strip 内，
                                // 导致滚动条等视口内非 strip 的 overlay 内容被截掉。
                                if let Some((frame, _, _)) = scroll_move {
                                    rects.push(frame);
                                }
                                rects
                            }
                        } else {
                            Vec::new()
                        };
                        engine.begin_frame(UpdateStrategy::Overlay(overlay_rects));
                        let theme_ref = theme.borrow();
                        let tokens = theme_ref.tokens();
                        let lt_font = font_service.loaded_font_handle;
                        let hover_pos =
                            if debug_mode.get() { Some(cursor_pos.get()) } else { None };
                        layer_tree.render_overlays(
                            engine,
                            tree,
                            tokens,
                            lt_font,
                            font_service,
                            debug_mode.get(),
                            hover_pos,
                            &region,
                        );
                        drop(theme_ref);
                        engine.end_frame();
                    }
                }

                for res in resources_to_bump {
                    frame_graph.mark_resource_dirty(res);
                }

                tree.reset_dirty();
                rendered_first = true;
                RenderOutcome::Present(damage)
            }
        };

        match outcome {
            RenderOutcome::Present(damage) => {
                idle_count = 0;
                // damage 始终为 None（全帧输出），rendered_first 仅用于 region 计算
                let canvas = engine.canvas_2d();
                let cw = canvas.width();
                let ch = canvas.height();
                if let Err(e) = platform_window
                    .presenter()
                    .present(canvas.pixels_mut(), cw, ch, damage)
                {
                    uix_platform::log::error_fn(format!(
                        "[EventLoop] present failed: {}",
                        e.short_what()
                    ));
                }
            }
            RenderOutcome::Idle => {
                idle_count = idle_count.saturating_add(1);
            }
        }

        if keep_polling {
            if !platform.event_loop().wait_event(&collect) {
                break;
            }
            platform.event_loop().poll_event(&collect);
        }
    }

    0
}

/// 检查并同步根 widget 的 frame 到引擎画布尺寸。
/// 当根 frame 与画布尺寸不匹配时，自动调整并触发重排。
fn sync_root_frame_to_engine(tree: &mut WidgetTree, engine: &mut dyn GraphicsEngine) {
    let need_sync = tree
        .root_id()
        .and_then(|rid| tree.get(rid))
        .is_some_and(|root| {
            let ew = engine.canvas_2d().width() as f32;
            let eh = engine.canvas_2d().height() as f32;
            let rf = root.frame();
            (rf.w - ew).abs() > 0.5 || (rf.h - eh).abs() > 0.5
        });
    if need_sync {
        if let Some(rid) = tree.root_id() {
            if let Some(root_mut) = tree.get_mut(rid) {
                root_mut.set_frame(Rect::new(
                    0.0,
                    0.0,
                    engine.canvas_2d().width() as f32,
                    engine.canvas_2d().height() as f32,
                ));
            }
        }
        tree.mark_full_frame_dirty();
        tree.layout();
        // 不递增 tree_version——树结构未改变，LayerTree 无需重建。
        // mark_full_frame_dirty + layout() 已确保脏区域和布局正确。
    }
}

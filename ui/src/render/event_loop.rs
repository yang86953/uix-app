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
//!   ├─ AnimationRegistry 仅 tick 活跃动画节点
//!   ├─ dirty_rect() 计算变化区域 → InvalidationQueue + dirty_region
//!   └─ 动画结束 → 注册表清空，恢复 0 帧
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
//! 无 invalidation 且无脏区域时返回 `RenderOutcome::Idle`（0 帧）。

use std::cell::{Cell, RefCell};
use std::time::Instant;

use uix_graphics::pipeline::{InvalidationSource, RenderMetrics};
use uix_graphics::{Color, DirtyRegion, GraphicsEngine, RenderOutcome, UpdateStrategy};
use uix_platform::event::{UiEvent, UiEventPayload, UiEventType};
use uix_platform::{Platform, PlatformWindow, Point, Rect};

use crate::clipboard;
use super::debug::DebugRenderService;
use super::text::TextRenderService;
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
    metrics: Option<&Cell<RenderMetrics>>,
    map_event: M,
    on_exit: X,
    on_frame: F,
) -> i32
where
    M: Fn(&UiEvent) -> Option<WidgetEvent>,
    X: Fn(&UiEvent) -> bool,
    F: Fn(&mut WidgetTree, &mut dyn GraphicsEngine, &mut dyn Platform),
{
    // 从 platform 提前取出 event_bus 原始指针——避免 collect closure 中双重可变借用。
    // Safety: platform 在 closure 的整个生命周期内存活且不被别名访问。
    let bus_ptr: *mut dyn Platform = platform as *mut dyn Platform;

    let pending_events = RefCell::new(Vec::<UiEvent>::new());
    tree.bind_invalidation();

    let mut first_frame = true;
    let mut rendered_first = false;
    let mut last_frame = Instant::now();
    let mut idle_count: u32 = 0;
    let mut window_visible = true;
    let mut layer_tree = LayerTree::new();
    let mut last_tree_version: u64 = 0;
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
        let animations_active = tree.animations_active();
        if animations_active {
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
                platform
                    .event_loop()
                    .wait_timeout(std::time::Duration::from_millis(100), &collect);
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
        let _ = tree.update(dt);

        // 仅在以下情况触发 layout：
        // - 有布局事件（resize/点击/键盘等，排除 MouseMove）
        // - 首帧（尚未 rendered_first）
        let needs_work = window_visible && (had_layout_event || !rendered_first);

        if needs_work {
            let before_version = tree.tree_version();
            tree.layout();
            record_layout(metrics);

            // 根 frame 与引擎画布尺寸同步（on_frame 前后均需检查，
            // 因为 on_frame 可能改变树结构导致根 frame 失配）
            sync_root_frame_to_engine(tree, engine);
            on_frame(tree, engine, platform);
            sync_root_frame_to_engine(tree, engine);

            // on_frame 可能重建整棵树（主题切换），此时必须强制全帧渲染。
            if tree.tree_version() != before_version {
                // ⚠️  重建后必须重新 layout：set_root 只设置了根节点的 frame，
                // 所有子节点的 frame 为 Rect::zero()，不 layout 则组件位置混乱。
                // sync_root_frame_to_engine 之后调用 layout 确保根 frame 已正确同步到画布。
                tree.layout();
                record_layout(metrics);
                tree.mark_full_frame_dirty();
            }
        }

        let dirty_region = tree.dirty_region();
        let need_render =
            window_visible && (!rendered_first || tree.has_render_work());
        let engine_capabilities = engine.capabilities();

        let inv_source = classify_invalidation(rendered_first, had_layout_event, &dirty_region);

        let (outcome, outcome_source) = if !need_render {
            (RenderOutcome::Idle, InvalidationSource::None)
        } else {
            let region = if !rendered_first
                || dirty_region.full_frame
                || !engine_capabilities.supports_partial_redraw()
            {
                DirtyRegion::full()
            } else {
                dirty_region.clone()
            };

            let cur_version = tree.tree_version();
            if last_tree_version != cur_version {
                layer_tree.build(tree);
                layer_tree.sweep_orphaned_offscreens(engine);
                last_tree_version = cur_version;
            }
            layer_tree.update_dirty(tree);

            let scroll_move = tree.drain_scroll_region_move();
            if let Some((frame, dx, dy)) = scroll_move {
                engine.canvas_2d().scroll_region(frame, dx, dy);
            }

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

            // Geometry Pass
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
            record_paint(metrics);
            drop(lt_ref);
            engine.end_frame();

            // Overlay Pass
            let overlay_rects: Vec<Rect> = if region.clear_required {
                if region.full_frame {
                    let w = engine.canvas_2d().width() as f32;
                    let h = engine.canvas_2d().height() as f32;
                    vec![Rect::new(0.0, 0.0, w, h)]
                } else {
                    let mut rects = region.rects().to_vec();
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
            let hover_pos = if debug_mode.get() {
                Some(cursor_pos.get())
            } else {
                None
            };
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
            if debug_mode.get() {
                if let Some(m) = metrics {
                    let canvas = engine.canvas_2d();
                    let sw = canvas.width();
                    let hud = DebugRenderService::new(true);
                    hud.draw_telemetry_hud(canvas, &m.get(), sw);
                    let lines = DebugRenderService::telemetry_hud_lines(&m.get());
                    let mut text_svc =
                        TextRenderService::new(lt_font, font_service, 300.0);
                    let panel_x = sw as f32 - 214.0;
                    for (i, line) in lines.iter().enumerate() {
                        text_svc.draw_text(
                            canvas,
                            line,
                            Point::new(panel_x, 12.0 + i as f32 * 14.0),
                            Color::from_rgba(220, 220, 220, 255),
                            11.0,
                        );
                    }
                }
            }

            tree.reset_dirty();
            rendered_first = true;
            (RenderOutcome::Present(damage), inv_source)
        };

        match outcome {
            RenderOutcome::Present(damage) => {
                record_present(metrics, outcome_source);
                idle_count = 0;
                if engine_capabilities.uses_external_presenter() {
                    let canvas = engine.canvas_2d();
                    let cw = canvas.width();
                    let ch = canvas.height();
                    if let Err(e) =
                        platform_window
                            .presenter()
                            .present(canvas.pixels_mut(), cw, ch, damage)
                    {
                        uix_platform::log::error_fn(format!(
                            "[EventLoop] present failed: {}",
                            e.short_what()
                        ));
                    }
                }
            }
            RenderOutcome::Idle => {
                record_idle(metrics, outcome_source);
                idle_count = idle_count.saturating_add(1);
            }
        }

        if tree.animations_active() {
            if !platform.event_loop().wait_event(&collect) {
                break;
            }
            platform.event_loop().poll_event(&collect);
        }
    }

    0
}

fn classify_invalidation(
    rendered_first: bool,
    had_layout_event: bool,
    dirty_region: &DirtyRegion,
) -> InvalidationSource {
    if !rendered_first {
        return InvalidationSource::FirstFrame;
    }
    if !dirty_region.is_empty() {
        return InvalidationSource::DirtyRegion;
    }
    if had_layout_event {
        return InvalidationSource::LayoutEvent;
    }
    InvalidationSource::None
}

fn record_layout(metrics: Option<&Cell<RenderMetrics>>) {
    if let Some(m) = metrics {
        let mut stats = m.get();
        stats.record_layout();
        m.set(stats);
    }
}

fn record_paint(metrics: Option<&Cell<RenderMetrics>>) {
    if let Some(m) = metrics {
        let mut stats = m.get();
        stats.record_paint();
        m.set(stats);
    }
}

fn record_present(metrics: Option<&Cell<RenderMetrics>>, source: InvalidationSource) {
    if let Some(m) = metrics {
        let mut stats = m.get();
        stats.record_present(source);
        m.set(stats);
    }
}

fn record_idle(metrics: Option<&Cell<RenderMetrics>>, source: InvalidationSource) {
    if let Some(m) = metrics {
        let mut stats = m.get();
        stats.record_idle_with_source(source);
        m.set(stats);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::container::Container;
    use std::cell::Cell;
    use uix_graphics::pipeline::{InvalidationSource, RenderMetrics};
    use uix_graphics::types::DirtyRegion;
    use uix_graphics::NullEngine;

    #[test]
    fn classify_invalidation_first_frame() {
        let region = DirtyRegion::default();
        assert_eq!(
            classify_invalidation(false, false, &region),
            InvalidationSource::FirstFrame
        );
    }

    #[test]
    fn classify_invalidation_dirty_region() {
        let mut region = DirtyRegion::default();
        region.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(
            classify_invalidation(true, false, &region),
            InvalidationSource::DirtyRegion
        );
    }

    #[test]
    fn metrics_recording_helpers() {
        let m = Cell::new(RenderMetrics::default());
        record_layout(Some(&m));
        record_paint(Some(&m));
        record_present(Some(&m), InvalidationSource::FirstFrame);
        let stats = m.get();
        assert_eq!(stats.layout_calls, 1);
        assert_eq!(stats.paint_calls, 1);
        assert_eq!(stats.present_calls, 1);
        assert_eq!(stats.last_invalidation, InvalidationSource::FirstFrame);
    }

    /// sync_root_frame_to_engine 当根 frame 不匹配时自动同步（缩小）到引擎画布尺寸
    #[test]
    fn sync_root_frame_mismatch() {
        let mut tree = WidgetTree::new();
        let rid = tree.set_root(Box::new(Container::new()));
        // 设置根 frame 为 800x600
        if let Some(root) = tree.get_mut(rid) {
            root.set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
        }
        let mut engine = NullEngine::new();
        // NullEngine 的 canvas_2d() 返回 NoopCanvas2D，surface_size() 为 (0,0)
        sync_root_frame_to_engine(&mut tree, &mut engine);
        let root = tree.get(rid).unwrap();
        // 同步后根 frame 宽度/高度已被缩小（引擎 canvas 0x0, layout 后取最小 1x1）
        assert!(root.frame().w < 800.0);
        assert!(root.frame().h < 600.0);
    }

    /// sync_root_frame_to_engine 当尺寸匹配时不触发改变
    #[test]
    fn sync_root_frame_already_matched() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(Container::new()));
        // 根 frame 已是 (0,0,0,0)，与 NullEngine 的 canvas (0,0) 匹配
        let mut engine = NullEngine::new();
        sync_root_frame_to_engine(&mut tree, &mut engine);
        if let Some(rid) = tree.root_id() {
            let root = tree.get(rid).unwrap();
            assert_eq!(root.frame(), Rect::new(0.0, 0.0, 0.0, 0.0));
        }
    }
}

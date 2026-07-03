//! Render Loop — OS 事件 + Widget 调度；渲染段委托 graphics FrameRenderer。

use std::cell::{Cell, RefCell};
use std::time::Instant;

use uix_graphics::font_service::FontService;
use uix_graphics::pipeline::{FrameRenderInput, FrameRenderer, InvalidationSource, RenderMetrics};
use uix_graphics::painting::ThemeSnapshot;
use uix_graphics::{GraphicsEngine, RenderOutcome};
use uix_platform::event::{UiEvent, UiEventPayload, UiEventType};
use uix_platform::{Platform, PlatformWindow, Point, Rect};

use crate::clipboard;
use crate::theme::Theme;
use crate::widget::{WidgetCore, WidgetEvent, WidgetTree};

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
    let bus_ptr: *mut dyn Platform = platform as *mut dyn Platform;

    let pending_events = RefCell::new(Vec::<UiEvent>::new());
    tree.bind_invalidation();

    let mut first_frame = true;
    let mut rendered_first = false;
    let mut last_frame = Instant::now();
    let mut idle_count: u32 = 0;
    let mut window_visible = true;
    let mut frame_renderer = FrameRenderer::new();
    let mut initial_size = (
        platform_window.properties().width(),
        platform_window.properties().height(),
    );

    {
        let c: &mut dyn uix_platform::IClipboard = platform.clipboard();
        let wide: *mut dyn uix_platform::IClipboard = c;
        let parts: (usize, usize) = unsafe { std::mem::transmute(wide) };
        clipboard::set_clipboard_parts(parts.0, parts.1);
    }

    let running = Cell::new(true);

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
        let mut had_layout_event = false;

        for ev in pending_events.borrow_mut().drain(..) {
            let is_layout_event = !matches!(ev.type_, UiEventType::MouseMove);
            if is_layout_event {
                had_layout_event = true;
            }

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
                            debug_mode.set(!debug_mode.get());
                            continue;
                        }
                    }
                }
                _ => {}
            }

            if let Some(we) = map_event(&ev) {
                tree.dispatch_event(&we);
            }

            unsafe {
                (*bus_ptr).event_bus().publish(&ev);
            }
        }
        if had_events {
            idle_count = 0;
        }

        let now = Instant::now();
        let dt = (now - last_frame).as_secs_f64().min(0.05);
        last_frame = now;
        let _ = tree.update(dt);

        let needs_work = window_visible && (had_layout_event || !rendered_first);

        if needs_work {
            let before_version = tree.tree_version();
            tree.layout();
            record_layout(metrics);

            sync_root_frame_to_engine(tree, engine);
            on_frame(tree, engine, platform);
            sync_root_frame_to_engine(tree, engine);

            if tree.tree_version() != before_version {
                tree.layout();
                record_layout(metrics);
                tree.mark_full_frame_dirty();
            }
        }

        let dirty_region = tree.dirty_region().clone();
        let need_render = window_visible && (!rendered_first || tree.has_render_work());
        let engine_capabilities = engine.capabilities();

        let (outcome, outcome_source) = if !need_render {
            (RenderOutcome::Idle, InvalidationSource::None)
        } else {
            let theme_ref = theme.borrow();
            let snapshot = ThemeSnapshot::new(theme_ref.tokens());
            let scroll_move = tree.drain_scroll_region_move();
            let hover_pos = if debug_mode.get() {
                Some(cursor_pos.get())
            } else {
                None
            };
            let metrics_ref = metrics.map(|m| m.get());
            let frame_out = frame_renderer.render_frame(
                engine,
                tree,
                FrameRenderInput {
                    rendered_first,
                    dirty_region: &dirty_region,
                    tree_version: tree.tree_version(),
                    scroll_move,
                    theme: snapshot,
                    font: font_service.loaded_font_handle,
                    font_service,
                    debug_mode: debug_mode.get(),
                    hover_pos,
                    metrics: metrics_ref.as_ref(),
                },
            );
            tree.reset_dirty();
            rendered_first = true;
            (frame_out.outcome, frame_out.inv_source)
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

fn record_layout(metrics: Option<&Cell<RenderMetrics>>) {
    if let Some(m) = metrics {
        let mut stats = m.get();
        stats.record_layout();
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::container::Container;
    use std::cell::Cell;
    use uix_graphics::NullEngine;

    #[test]
    fn sync_root_frame_mismatch() {
        let mut tree = WidgetTree::new();
        let rid = tree.set_root(Box::new(Container::new()));
        if let Some(root) = tree.get_mut(rid) {
            root.set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
        }
        let mut engine = NullEngine::new();
        sync_root_frame_to_engine(&mut tree, &mut engine);
        let root = tree.get(rid).unwrap();
        assert!(root.frame().w < 800.0);
        assert!(root.frame().h < 600.0);
    }

    #[test]
    fn sync_root_frame_already_matched() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(Container::new()));
        let mut engine = NullEngine::new();
        sync_root_frame_to_engine(&mut tree, &mut engine);
        if let Some(rid) = tree.root_id() {
            let root = tree.get(rid).unwrap();
            assert_eq!(root.frame(), Rect::new(0.0, 0.0, 0.0, 0.0));
        }
    }
}

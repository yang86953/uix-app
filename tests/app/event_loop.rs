//! app 域 — event_loop 集成测试（State 变更 → present）。
//!
//! 模拟 `run_widget_loop` 单帧体：layout → need_render → FrameRenderer → reset_dirty。
//!
//! ```bash
//! cargo test --features test-harness -p uix event_loop
//! ```

#![cfg(feature = "test-harness")]

use std::cell::Cell;

use uix::draw::font::font_service::FontService;
use uix::draw::image::ImageService;
use uix::draw::pipeline::{FrameRenderInput, FrameRenderer, InvalidationSource, RenderMetrics};
use uix::draw::painting::ThemeSnapshot;
use uix::draw::{GraphicsEngine, NullEngine, RenderOutcome};
use uix::native::Rect;
use uix::prelude::*;
use uix::ui::view::ViewAdapter;

/// 执行一帧渲染（与 `event_loop` 中 `FrameRenderer::render_frame` 路径对齐）。
fn render_frame_tick(
    tree: &mut WidgetTree,
    engine: &mut NullEngine,
    frame_renderer: &mut FrameRenderer,
    rendered_first: &mut bool,
    font_service: &FontService,
    image_service: &ImageService,
    theme: &Theme,
    metrics: &Cell<RenderMetrics>,
) -> RenderOutcome {
    let need_render = !*rendered_first || tree.has_render_work();
    if !need_render {
        return RenderOutcome::Idle;
    }

    if !*rendered_first {
        tree.mark_full_frame_dirty();
        tree.layout();
    }

    let dirty_region = tree.dirty_region();
    let scroll_move = tree.drain_scroll_region_move();
    let theme_snap = ThemeSnapshot::new(theme.tokens());
    let metrics_snapshot = metrics.get();
    let out = frame_renderer.render_frame(
        engine,
        tree,
        FrameRenderInput {
            rendered_first: *rendered_first,
            dirty_region: &dirty_region,
            tree_version: tree.tree_version(),
            scroll_move,
            theme: theme_snap,
            font: font_service.loaded_font_handle,
            font_service,
            image_service,
            debug_mode: false,
            hover_pos: None,
            metrics: Some(&metrics_snapshot),
        },
    );
    *rendered_first = true;

    if matches!(out.outcome, RenderOutcome::Present(_)) {
        let mut m = metrics.get();
        m.record_present(out.inv_source);
        metrics.set(m);
    }

    tree.reset_dirty();
    out.outcome
}

#[test]
fn state_change_triggers_second_present() {
    let count = State::new(0);
    let label_count = count.clone();
    let section = column([dynamic_label(move || format!("Count: {}", label_count.get()))]);

    let mut tree = ViewAdapter::build_nodes(section);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, 200.0, 100.0));
    }
    tree.bind_invalidation();

    let mut engine = NullEngine::new();
    engine.initialize(200, 100).expect("engine init");

    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = Theme::default();
    let metrics = Cell::new(RenderMetrics::default());
    let mut frame_renderer = FrameRenderer::new();
    let mut rendered_first = false;

    // 帧 1：首帧 present
    let o1 = render_frame_tick(
        &mut tree,
        &mut engine,
        &mut frame_renderer,
        &mut rendered_first,
        &font_service,
        &image_service,
        &theme,
        &metrics,
    );
    assert!(matches!(o1, RenderOutcome::Present(_)));
    assert_eq!(metrics.get().present_calls, 1);

    // 帧 2：Counter +1 → 精确 Paint 失效 → 第二次 present
    count.set(1);
    assert!(tree.has_render_work());

    tree.update(0.016);
    let o2 = render_frame_tick(
        &mut tree,
        &mut engine,
        &mut frame_renderer,
        &mut rendered_first,
        &font_service,
        &image_service,
        &theme,
        &metrics,
    );
    assert!(matches!(o2, RenderOutcome::Present(_)));
    assert!(
        metrics.get().present_calls >= 2,
        "State 变更应触发第二次 present"
    );
    assert_ne!(
        metrics.get().last_invalidation,
        InvalidationSource::None,
        "第二次 present 应有明确失效来源"
    );
}

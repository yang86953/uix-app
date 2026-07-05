//! 帧渲染调度 — 从 UI event_loop 迁入的渲染段（Phase 3）。

use crate::native::{Point, Rect};

use crate::draw::backend::DamageRegion;
use crate::draw::compositor::{LayerTree, RenderObjectTree, ScenePaint};
use crate::draw::debug::DebugRenderService;
use crate::draw::font::font_service::FontService;
use crate::draw::font::text::TextRenderService;
use crate::draw::image::ImageService;
use crate::draw::painting::ThemeSnapshot;
use crate::draw::pipeline::{InvalidationSource, RenderMetrics};
use crate::draw::primitives::types::DirtyRegion;
use crate::draw::traits::{GraphicsEngine, UpdateStrategy};
use crate::draw::{Color, FontHandle, RenderOutcome};

/// 单帧渲染输入。
pub struct FrameRenderInput<'a> {
    pub rendered_first: bool,
    pub dirty_region: &'a DirtyRegion,
    pub tree_version: u64,
    pub scroll_move: Option<(Rect, f32, f32)>,
    pub theme: ThemeSnapshot<'a>,
    pub font: FontHandle,
    pub font_service: &'a FontService,
    pub image_service: &'a ImageService,
    pub debug_mode: bool,
    pub hover_pos: Option<Point>,
    pub metrics: Option<&'a RenderMetrics>,
}

/// 单帧渲染输出。
pub struct FrameRenderOutput {
    pub outcome: RenderOutcome,
    pub inv_source: InvalidationSource,
    pub tree_version: u64,
}

/// 帧渲染器 — 持有 LayerTree 与合成状态。
pub struct FrameRenderer {
    layer_tree: LayerTree,
    render_object_tree: RenderObjectTree,
    last_tree_version: u64,
}

impl FrameRenderer {
    pub fn new() -> Self {
        Self {
            layer_tree: LayerTree::new(),
            render_object_tree: RenderObjectTree::new(),
            last_tree_version: 0,
        }
    }

    pub fn render_object_tree(&self) -> &RenderObjectTree {
        &self.render_object_tree
    }

    pub fn layer_tree(&self) -> &LayerTree {
        &self.layer_tree
    }

    pub fn layer_tree_mut(&mut self) -> &mut LayerTree {
        &mut self.layer_tree
    }

    /// 执行单 Pass 渲染（Content + AfterChildren + 焦点环）；返回 Present damage 与 invalidation 来源。
    pub fn render_frame<S: ScenePaint>(
        &mut self,
        engine: &mut dyn GraphicsEngine,
        scene: &S,
        input: FrameRenderInput<'_>,
    ) -> FrameRenderOutput {
        let caps = engine.capabilities();
        let region = if !input.rendered_first
            || input.dirty_region.full_frame
            || input.dirty_region.is_empty()
            || !caps.supports_partial_redraw()
        {
            DirtyRegion::full()
        } else {
            input.dirty_region.clone()
        };

        let cur_version = scene.tree_version();
        if self.last_tree_version != cur_version {
            self.layer_tree.build(scene);
            self.layer_tree.sweep_orphaned_offscreens(engine);
            self.last_tree_version = cur_version;
        }
        self.layer_tree.update_dirty(scene);
        self.render_object_tree.sync(scene);

        if let Some((frame, dx, dy)) = input.scroll_move {
            engine.canvas_2d().scroll_region(frame, dx, dy);
        }

        let damage = compute_damage(&region, input.scroll_move, input.rendered_first);

        let strategy = if !input.rendered_first || region.full_frame {
            UpdateStrategy::FullRedraw
        } else {
            UpdateStrategy::DirtyRects(region.rects().to_vec())
        };
        let begin_outcome = engine.begin_frame(strategy);
        if begin_outcome == RenderOutcome::Idle {
            return FrameRenderOutput {
                outcome: RenderOutcome::Idle,
                inv_source: InvalidationSource::None,
                tree_version: cur_version,
            };
        }
        // 首帧绕过 DisplayList 缓存，避免空缓存重放导致侧栏等节点漏绘
        let render_objects = if input.rendered_first {
            Some(&mut self.render_object_tree)
        } else {
            None
        };
        self.layer_tree.render(
            engine,
            scene,
            &region,
            &input.theme,
            input.font,
            input.font_service,
            input.image_service,
            input.debug_mode,
            input.hover_pos,
            render_objects,
        );
        engine.end_frame(&damage);

        if input.debug_mode {
            draw_debug_telemetry(engine, input.metrics, input.font, input.font_service);
        }

        let inv_source = classify_invalidation(input.rendered_first, &region);

        FrameRenderOutput {
            outcome: RenderOutcome::Present(damage),
            inv_source,
            tree_version: cur_version,
        }
    }
}

impl Default for FrameRenderer {
    fn default() -> Self {
        Self::new()
    }
}

fn compute_damage(
    region: &DirtyRegion,
    scroll_move: Option<(Rect, f32, f32)>,
    rendered_first: bool,
) -> DamageRegion {
    if !rendered_first || region.full_frame {
        return DamageRegion::full();
    }
    let mut rects: Vec<Rect> = region
        .rects()
        .iter()
        .filter(|r| r.w > 0.0 && r.h > 0.0)
        .map(pad_damage_rect)
        .collect();
    if let Some((frame, _, _)) = scroll_move {
        if frame.w > 0.0 && frame.h > 0.0 {
            rects.push(pad_damage_rect(&frame));
        }
    }
    if rects.is_empty() {
        DamageRegion::full()
    } else {
        DamageRegion::partial(rects)
    }
}

fn pad_damage_rect(r: &Rect) -> Rect {
    Rect::new(
        (r.x - 1.0).max(0.0),
        (r.y - 1.0).max(0.0),
        r.w + 2.0,
        r.h + 2.0,
    )
}

fn draw_debug_telemetry(
    engine: &mut dyn GraphicsEngine,
    metrics: Option<&RenderMetrics>,
    font: FontHandle,
    font_service: &FontService,
) {
    let Some(m) = metrics else {
        return;
    };
    let canvas = engine.canvas_2d();
    let sw = canvas.width();
    let hud = DebugRenderService::new(true);
    hud.draw_telemetry_hud(canvas, m, sw);
    let lines = DebugRenderService::telemetry_hud_lines(m);
    let mut text_svc = TextRenderService::new(font, font_service, 300.0);
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

fn classify_invalidation(rendered_first: bool, dirty_region: &DirtyRegion) -> InvalidationSource {
    if !rendered_first {
        return InvalidationSource::FirstFrame;
    }
    if !dirty_region.is_empty() {
        return InvalidationSource::DirtyRegion;
    }
    InvalidationSource::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::null_engine::NullEngine;
    use crate::draw::painting::PaintContext;

    struct EmptyScene {
        region: DirtyRegion,
    }

    impl EmptyScene {
        fn new() -> Self {
            Self {
                region: DirtyRegion::empty(),
            }
        }
    }

    impl ScenePaint for EmptyScene {
        fn root_id(&self) -> Option<crate::draw::pipeline::NodeId> {
            None
        }
        fn tree_version(&self) -> u64 {
            0
        }
        fn dirty_region(&self) -> DirtyRegion {
            self.region.clone()
        }
        fn node_visible(&self, _: crate::draw::pipeline::NodeId) -> bool {
            false
        }
        fn node_frame(&self, _: crate::draw::pipeline::NodeId) -> Rect {
            Rect::zero()
        }
        fn node_dirty(&self, _: crate::draw::pipeline::NodeId) -> bool {
            false
        }
        fn node_z_index(&self, _: crate::draw::pipeline::NodeId) -> i32 {
            0
        }
        fn node_children(
            &self,
            _: crate::draw::pipeline::NodeId,
        ) -> &[crate::draw::pipeline::NodeId] {
            &[]
        }
        fn children_clip(&self, _: crate::draw::pipeline::NodeId, _: Rect) -> Option<Rect> {
            None
        }
        fn dirty_rect(&self, _: crate::draw::pipeline::NodeId, frame: Rect) -> Rect {
            frame
        }
        fn scroll_offset(&self, _: crate::draw::pipeline::NodeId) -> Option<(f32, f32)> {
            None
        }
        fn focused_node(&self) -> Option<crate::draw::pipeline::NodeId> {
            None
        }
        fn node_focusable(&self, _: crate::draw::pipeline::NodeId) -> bool {
            false
        }
        fn hit_test(&self, _: Point) -> Option<crate::draw::pipeline::NodeId> {
            None
        }
        fn parent(
            &self,
            _: crate::draw::pipeline::NodeId,
        ) -> Option<crate::draw::pipeline::NodeId> {
            None
        }
        fn paint(&self, _: crate::draw::pipeline::NodeId, _: Rect, _: &mut PaintContext<'_>) {}
    }

    struct MockTokens;

    impl crate::draw::painting::IColorTokens for MockTokens {
        fn color_primary(&self) -> Color {
            Color::blue()
        }
        fn color_primary_hover(&self) -> Color {
            Color::blue()
        }
        fn color_primary_active(&self) -> Color {
            Color::blue()
        }
        fn color_primary_bg(&self) -> Color {
            Color::blue()
        }
        fn color_primary_border(&self) -> Color {
            Color::blue()
        }
        fn color_bg_container(&self) -> Color {
            Color::white()
        }
        fn color_bg_elevated(&self) -> Color {
            Color::white()
        }
        fn color_bg_raised(&self) -> Color {
            Color::white()
        }
        fn color_bg_overlay(&self) -> Color {
            Color::white()
        }
        fn color_bg_layout(&self) -> Color {
            Color::white()
        }
        fn color_bg_spotlight(&self) -> Color {
            Color::white()
        }
        fn color_bg_mask(&self) -> Color {
            Color::white()
        }
        fn color_border(&self) -> Color {
            Color::black()
        }
        fn color_border_secondary(&self) -> Color {
            Color::black()
        }
        fn color_fill(&self) -> Color {
            Color::black()
        }
        fn color_fill_secondary(&self) -> Color {
            Color::black()
        }
        fn color_fill_tertiary(&self) -> Color {
            Color::black()
        }
        fn color_fill_quaternary(&self) -> Color {
            Color::black()
        }
        fn color_text(&self) -> Color {
            Color::black()
        }
        fn color_text_secondary(&self) -> Color {
            Color::black()
        }
        fn color_text_tertiary(&self) -> Color {
            Color::black()
        }
        fn color_text_quaternary(&self) -> Color {
            Color::black()
        }
        fn color_white(&self) -> Color {
            Color::white()
        }
        fn color_black(&self) -> Color {
            Color::black()
        }
        fn color_shadow(&self) -> Color {
            Color::black()
        }
        fn color_shadow_secondary(&self) -> Color {
            Color::black()
        }
        fn color_success(&self) -> Color {
            Color::green()
        }
        fn color_success_bg(&self) -> Color {
            Color::green()
        }
        fn color_success_border(&self) -> Color {
            Color::green()
        }
        fn color_warning(&self) -> Color {
            Color::from_rgb(255, 255, 0)
        }
        fn color_warning_bg(&self) -> Color {
            Color::from_rgb(255, 255, 0)
        }
        fn color_warning_border(&self) -> Color {
            Color::from_rgb(255, 255, 0)
        }
        fn color_error(&self) -> Color {
            Color::red()
        }
        fn color_error_bg(&self) -> Color {
            Color::red()
        }
        fn color_error_border(&self) -> Color {
            Color::red()
        }
        fn color_info(&self) -> Color {
            Color::blue()
        }
        fn color_info_bg(&self) -> Color {
            Color::blue()
        }
        fn color_info_border(&self) -> Color {
            Color::blue()
        }
        fn color_link(&self) -> Color {
            Color::blue()
        }
        fn color_link_hover(&self) -> Color {
            Color::blue()
        }
        fn color_link_active(&self) -> Color {
            Color::blue()
        }
    }

    impl crate::draw::painting::ITypographyTokens for MockTokens {
        fn font_family(&self) -> &str {
            "sans"
        }
    }

    impl crate::draw::painting::IBoxShadowTokens for MockTokens {
        fn box_shadow(&self) -> crate::draw::painting::ShadowToken {
            crate::draw::painting::ShadowToken::none()
        }
        fn box_shadow_secondary(&self) -> crate::draw::painting::ShadowToken {
            crate::draw::painting::ShadowToken::none()
        }
    }

    impl crate::draw::painting::ISpacingTokens for MockTokens {}

    impl crate::draw::painting::ThemeTokens for MockTokens {}

    #[test]
    fn first_frame_expands_partial_dirty_to_full_paint_region() {
        struct PartialDirtyScene {
            region: DirtyRegion,
        }

        impl PartialDirtyScene {
            fn new() -> Self {
                let mut region = DirtyRegion::empty();
                region.add_rect(Rect::new(0.0, 0.0, 50.0, 50.0));
                Self { region }
            }
        }

        impl ScenePaint for PartialDirtyScene {
            fn root_id(&self) -> Option<crate::draw::pipeline::NodeId> {
                Some(1)
            }
            fn tree_version(&self) -> u64 {
                1
            }
            fn dirty_region(&self) -> DirtyRegion {
                self.region.clone()
            }
            fn node_visible(&self, _: crate::draw::pipeline::NodeId) -> bool {
                true
            }
            fn node_frame(&self, id: crate::draw::pipeline::NodeId) -> Rect {
                match id {
                    1 => Rect::new(0.0, 0.0, 100.0, 100.0),
                    2 => Rect::new(200.0, 200.0, 30.0, 30.0),
                    _ => Rect::zero(),
                }
            }
            fn node_dirty(&self, _: crate::draw::pipeline::NodeId) -> bool {
                false
            }
            fn node_z_index(&self, _: crate::draw::pipeline::NodeId) -> i32 {
                0
            }
            fn node_children(
                &self,
                id: crate::draw::pipeline::NodeId,
            ) -> &[crate::draw::pipeline::NodeId] {
                static ROOT_KIDS: [crate::draw::pipeline::NodeId; 1] = [2];
                static EMPTY: [crate::draw::pipeline::NodeId; 0] = [];
                if id == 1 {
                    &ROOT_KIDS
                } else {
                    &EMPTY
                }
            }
            fn children_clip(&self, _: crate::draw::pipeline::NodeId, _: Rect) -> Option<Rect> {
                None
            }
            fn dirty_rect(&self, _: crate::draw::pipeline::NodeId, frame: Rect) -> Rect {
                frame
            }
            fn scroll_offset(&self, _: crate::draw::pipeline::NodeId) -> Option<(f32, f32)> {
                None
            }
            fn focused_node(&self) -> Option<crate::draw::pipeline::NodeId> {
                None
            }
            fn node_focusable(&self, _: crate::draw::pipeline::NodeId) -> bool {
                false
            }
            fn hit_test(&self, _: Point) -> Option<crate::draw::pipeline::NodeId> {
                None
            }
            fn parent(
                &self,
                id: crate::draw::pipeline::NodeId,
            ) -> Option<crate::draw::pipeline::NodeId> {
                if id == 2 {
                    Some(1)
                } else {
                    None
                }
            }
            fn paint(&self, _: crate::draw::pipeline::NodeId, _: Rect, _: &mut PaintContext<'_>) {}
        }

        let mut renderer = FrameRenderer::new();
        let mut engine = NullEngine::new();
        let _ = engine.initialize(256, 256);
        let tokens = MockTokens;
        let theme = ThemeSnapshot::new(&tokens);
        let fs = FontService::new();
        let img = ImageService::new();
        let scene = PartialDirtyScene::new();
        let partial = scene.dirty_region();
        assert!(!partial.full_frame);
        // 子节点在 scene 的 partial dirty 之外
        assert!(!partial.intersects(Rect::new(200.0, 200.0, 30.0, 30.0)));

        let out = renderer.render_frame(
            &mut engine,
            &scene,
            FrameRenderInput {
                rendered_first: false,
                dirty_region: &partial,
                tree_version: 1,
                scroll_move: None,
                theme,
                font: FontHandle::default(),
                font_service: &fs,
                image_service: &img,
                debug_mode: false,
                hover_pos: None,
                metrics: None,
            },
        );
        assert_eq!(out.outcome, RenderOutcome::Present(DamageRegion::full()));
    }

    #[test]
    fn mock_scene_render_frame_does_not_panic() {
        let mut renderer = FrameRenderer::new();
        let mut engine = NullEngine::new();
        let _ = engine.initialize(64, 64);
        let tokens = MockTokens;
        let theme = ThemeSnapshot::new(&tokens);
        let fs = FontService::new();
        let img = ImageService::new();
        let region = DirtyRegion::full();
        let out = renderer.render_frame(
            &mut engine,
            &EmptyScene::new(),
            FrameRenderInput {
                rendered_first: false,
                dirty_region: &region,
                tree_version: 0,
                scroll_move: None,
                theme,
                font: FontHandle::default(),
                font_service: &fs,
                image_service: &img,
                debug_mode: false,
                hover_pos: None,
                metrics: None,
            },
        );
        assert_eq!(out.outcome, RenderOutcome::Present(DamageRegion::full()));
    }
}

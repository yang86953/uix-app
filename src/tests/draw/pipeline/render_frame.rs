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
    fn node_children(&self, _: crate::draw::pipeline::NodeId) -> &[crate::draw::pipeline::NodeId] {
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
    fn parent(&self, _: crate::draw::pipeline::NodeId) -> Option<crate::draw::pipeline::NodeId> {
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

#[test]
fn rendered_frame_with_empty_dirty_region_is_idle() {
    let mut renderer = FrameRenderer::new();
    let mut engine = NullEngine::new();
    let _ = engine.initialize(64, 64);
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    let img = ImageService::new();
    let region = DirtyRegion::empty();
    let out = renderer.render_frame(
        &mut engine,
        &EmptyScene::new(),
        FrameRenderInput {
            rendered_first: true,
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

    assert_eq!(out.outcome, RenderOutcome::Idle);
    assert_eq!(out.inv_source, InvalidationSource::None);
}

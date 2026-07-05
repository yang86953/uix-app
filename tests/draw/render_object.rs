//! draw 域 — RenderObject 集成测试。

use std::cell::RefCell;
use std::collections::HashMap;

use uix::draw::compositor::{RenderObjectTree, ScenePaint};
use uix::draw::font_service::FontService;
use uix::draw::painting::{
    IBoxShadowTokens, IColorTokens, ISpacingTokens, ITypographyTokens, PaintContext, ShadowToken,
    ThemeSnapshot, ThemeTokens,
};
use uix::draw::image::ImageService;
use uix::draw::pipeline::{FrameRenderInput, FrameRenderer, NodeId};
use uix::draw::traits::GraphicsEngine;
use uix::draw::types::DirtyRegion;
use uix::draw::{Color, FontHandle, SoftwareEngine};
use uix::native::{Point, Rect};

const ROOT: NodeId = 1;
const LEAF: NodeId = 2;

struct LeafScene {
    tree_version: u64,
    dirty_region: DirtyRegion,
    nodes_dirty: HashMap<NodeId, bool>,
    paint_counts: RefCell<HashMap<NodeId, u32>>,
}

impl LeafScene {
    fn first_frame() -> Self {
        let mut nodes_dirty = HashMap::new();
        nodes_dirty.insert(ROOT, true);
        nodes_dirty.insert(LEAF, true);
        Self {
            tree_version: 1,
            dirty_region: DirtyRegion::full(),
            nodes_dirty,
            paint_counts: RefCell::new(HashMap::new()),
        }
    }

    fn mark_all_clean(&mut self) {
        for dirty in self.nodes_dirty.values_mut() {
            *dirty = false;
        }
    }

    fn leaf_paint_count(&self) -> u32 {
        self.paint_counts.borrow().get(&LEAF).copied().unwrap_or(0)
    }
}

impl ScenePaint for LeafScene {
    fn root_id(&self) -> Option<NodeId> {
        Some(ROOT)
    }

    fn tree_version(&self) -> u64 {
        self.tree_version
    }

    fn dirty_region(&self) -> DirtyRegion {
        self.dirty_region.clone()
    }

    fn node_visible(&self, id: NodeId) -> bool {
        id == ROOT || id == LEAF
    }

    fn node_frame(&self, id: NodeId) -> Rect {
        match id {
            ROOT => Rect::new(0.0, 0.0, 100.0, 100.0),
            LEAF => Rect::new(10.0, 10.0, 40.0, 40.0),
            _ => Rect::zero(),
        }
    }

    fn node_dirty(&self, id: NodeId) -> bool {
        self.nodes_dirty.get(&id).copied().unwrap_or(false)
    }

    fn node_z_index(&self, id: NodeId) -> i32 {
        id as i32
    }

    fn node_children(&self, id: NodeId) -> &[NodeId] {
        if id == ROOT {
            &[LEAF][..]
        } else {
            &[]
        }
    }

    fn is_repaint_boundary(&self, _: NodeId) -> bool {
        false
    }

    fn children_clip(&self, _: NodeId, _: Rect) -> Option<Rect> {
        None
    }

    fn dirty_rect(&self, _: NodeId, frame: Rect) -> Rect {
        frame
    }

    fn scroll_offset(&self, _: NodeId) -> Option<(f32, f32)> {
        None
    }

    fn focused_node(&self) -> Option<NodeId> {
        None
    }

    fn node_focusable(&self, _: NodeId) -> bool {
        false
    }

    fn hit_test(&self, _: Point) -> Option<NodeId> {
        None
    }

    fn parent(&self, id: NodeId) -> Option<NodeId> {
        if id == LEAF {
            Some(ROOT)
        } else {
            None
        }
    }

    fn paint(&self, id: NodeId, frame: Rect, ctx: &mut PaintContext<'_>) {
        *self.paint_counts.borrow_mut().entry(id).or_insert(0) += 1;
        if id == LEAF {
            ctx.fill_rect(frame, Color::from_rgba(255, 0, 0, 255), None);
        }
    }
}

struct MockTokens;

impl IColorTokens for MockTokens {
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

impl ITypographyTokens for MockTokens {
    fn font_family(&self) -> &str {
        "sans"
    }
}

impl IBoxShadowTokens for MockTokens {
    fn box_shadow(&self) -> ShadowToken {
        ShadowToken::none()
    }
    fn box_shadow_secondary(&self) -> ShadowToken {
        ShadowToken::none()
    }
}

impl ISpacingTokens for MockTokens {}

impl ThemeTokens for MockTokens {}

#[test]
fn render_object_tree_sync_indexes_visible_nodes() {
    let scene = LeafScene::first_frame();
    let mut tree = RenderObjectTree::new();
    tree.sync(&scene);
    assert_eq!(tree.len(), 2);
    assert!(tree.get(LEAF).is_some());
}

#[test]
fn frame_renderer_replays_clean_leaf_via_display_list() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(100, 100).unwrap();

    let mut scene = LeafScene::first_frame();
    let mut renderer = FrameRenderer::new();
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
        let fs = FontService::new();
        let img = ImageService::new();
    let region = DirtyRegion::full();

    renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &region,
            tree_version: scene.tree_version(),
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font: FontHandle::default(),
            font_service: &fs,
            image_service: &img,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    assert_eq!(scene.leaf_paint_count(), 1);

    scene.mark_all_clean();
    renderer.render_frame(
        &mut engine,
        &scene,
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &region,
            tree_version: scene.tree_version(),
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
    assert_eq!(
        scene.leaf_paint_count(),
        1,
        "干净 leaf 在整帧 dirty 下应重放 DisplayList 而非再次 paint"
    );
    assert!(
        renderer
            .render_object_tree()
            .get(LEAF)
            .and_then(|e| e.display_list.as_ref())
            .is_some()
    );
}

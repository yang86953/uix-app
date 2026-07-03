//! Phase 5 — ScrollView viewport 坐标与 strip 脏剪枝集成测试。

use std::cell::RefCell;
use std::collections::HashMap;

use uix_graphics::compositor::{LayerTree, ScenePaint};
use uix_graphics::font_service::FontService;
use uix_graphics::painting::{
    PaintContext, ShadowToken, ThemeSnapshot, ThemeTokens, IBoxShadowTokens, IColorTokens,
    ISpacingTokens, ITypographyTokens,
};
use uix_graphics::pipeline::NodeId;
use uix_graphics::traits::{GraphicsEngine, UpdateStrategy};
use uix_graphics::types::DirtyRegion;
use uix_graphics::{Color, FontHandle, SoftwareEngine};
use uix_platform::Rect;

const VIEWPORT: NodeId = 1;
const VISIBLE_CHILD: NodeId = 2;
const OFFSCREEN_CHILD: NodeId = 3;

struct ScrollStripScene {
    dirty_region: DirtyRegion,
    scroll_y: f32,
    paint_counts: RefCell<HashMap<NodeId, u32>>,
}

impl ScrollStripScene {
    fn strip_at_bottom() -> Self {
        Self {
            dirty_region: DirtyRegion::area(Rect::new(0.0, 90.0, 100.0, 10.0)),
            scroll_y: 50.0,
            paint_counts: RefCell::new(HashMap::new()),
        }
    }

    fn count(&self, id: NodeId) -> u32 {
        self.paint_counts.borrow().get(&id).copied().unwrap_or(0)
    }
}

impl ScenePaint for ScrollStripScene {
    fn root_id(&self) -> Option<NodeId> {
        Some(VIEWPORT)
    }
    fn tree_version(&self) -> u64 {
        1
    }
    fn dirty_region(&self) -> DirtyRegion {
        self.dirty_region.clone()
    }
    fn node_visible(&self, _: NodeId) -> bool {
        true
    }
    fn node_frame(&self, id: NodeId) -> Rect {
        match id {
            VIEWPORT => Rect::new(0.0, 0.0, 100.0, 100.0),
            VISIBLE_CHILD => Rect::new(0.0, 140.0, 100.0, 20.0),
            OFFSCREEN_CHILD => Rect::new(0.0, 10.0, 100.0, 20.0),
            _ => Rect::zero(),
        }
    }
    fn node_dirty(&self, _: NodeId) -> bool {
        false
    }
    fn node_z_index(&self, id: NodeId) -> i32 {
        id as i32
    }
    fn node_children(&self, id: NodeId) -> &[NodeId] {
        match id {
            VIEWPORT => &[VISIBLE_CHILD, OFFSCREEN_CHILD][..],
            _ => &[],
        }
    }
    fn is_repaint_boundary(&self, _: NodeId) -> bool {
        false
    }
    fn children_clip(&self, id: NodeId, _: Rect) -> Option<Rect> {
        if id == VIEWPORT {
            Some(Rect::new(0.0, 0.0, 100.0, 100.0))
        } else {
            None
        }
    }
    fn dirty_rect(&self, _: NodeId, frame: Rect) -> Rect {
        frame
    }
    fn scroll_offset(&self, id: NodeId) -> Option<(f32, f32)> {
        if id == VIEWPORT {
            Some((0.0, self.scroll_y))
        } else {
            None
        }
    }
    fn focused_node(&self) -> Option<NodeId> {
        None
    }
    fn node_focusable(&self, _: NodeId) -> bool {
        false
    }
    fn hit_test(&self, _: uix_platform::Point) -> Option<NodeId> {
        None
    }
    fn parent(&self, id: NodeId) -> Option<NodeId> {
        if id == VISIBLE_CHILD || id == OFFSCREEN_CHILD {
            Some(VIEWPORT)
        } else {
            None
        }
    }
    fn paint(&self, id: NodeId, frame: Rect, ctx: &mut PaintContext<'_>) {
        *self.paint_counts.borrow_mut().entry(id).or_insert(0) += 1;
        ctx.fill_rect(frame, Color::from_rgba(80, 80, 80, 255), None);
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
        Color::from_rgba(255, 255, 0, 255)
    }
    fn color_warning_bg(&self) -> Color {
        Color::from_rgba(255, 255, 0, 255)
    }
    fn color_warning_border(&self) -> Color {
        Color::from_rgba(255, 255, 0, 255)
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

fn render_scene(engine: &mut SoftwareEngine, tree: &mut LayerTree, scene: &ScrollStripScene) {
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    engine.begin_frame(UpdateStrategy::DirtyRects(scene.dirty_region().rects().to_vec()));
    tree.render(
        engine,
        scene,
        &theme,
        FontHandle::default(),
        &fs,
        false,
        None,
        None,
    );
    engine.end_frame();
}

#[test]
fn scroll_strip_only_repaints_visible_child_in_viewport() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(100, 100).unwrap();

    let scene = ScrollStripScene::strip_at_bottom();
    let mut tree = LayerTree::new();
    tree.build(&scene);

    render_scene(&mut engine, &mut tree, &scene);

    assert_eq!(
        scene.count(VISIBLE_CHILD),
        1,
        "strip 与 viewport 投影相交的子节点应绘制"
    );
    assert_eq!(
        scene.count(OFFSCREEN_CHILD),
        0,
        "viewport 外 content 子节点不应绘制"
    );
}

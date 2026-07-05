//! draw 域 — 重绘边界集成测试。

use std::cell::RefCell;
use std::collections::HashMap;

use uix::draw::compositor::{LayerTree, ScenePaint};
use uix::draw::font_service::FontService;
use uix::draw::painting::{
    PaintContext, ShadowToken, ThemeSnapshot, ThemeTokens, IBoxShadowTokens, IColorTokens,
    ISpacingTokens, ITypographyTokens,
};
use uix::draw::pipeline::NodeId;
use uix::draw::traits::{GraphicsEngine, UpdateStrategy};
use uix::draw::types::DirtyRegion;
use uix::draw::{Color, FontHandle, SoftwareEngine};
use uix::native::Rect;

const ROOT: NodeId = 1;
const BOUNDARY: NodeId = 2;
const INNER: NodeId = 3;
const SIBLING: NodeId = 4;

struct NodeSpec {
    frame: Rect,
    repaint_boundary: bool,
    dirty: bool,
    color: Color,
}

struct BoundaryScene {
    nodes: HashMap<NodeId, NodeSpec>,
    dirty_region: DirtyRegion,
    tree_version: u64,
    paint_counts: RefCell<HashMap<NodeId, u32>>,
}

impl BoundaryScene {
    fn full_frame_dirty() -> Self {
        let mut nodes = HashMap::new();
        nodes.insert(
            ROOT,
            NodeSpec {
                frame: Rect::new(0.0, 0.0, 200.0, 200.0),
                repaint_boundary: false,
                dirty: true,
                color: Color::transparent(),
            },
        );
        nodes.insert(
            BOUNDARY,
            NodeSpec {
                frame: Rect::new(10.0, 10.0, 80.0, 80.0),
                repaint_boundary: true,
                dirty: true,
                color: Color::transparent(),
            },
        );
        nodes.insert(
            INNER,
            NodeSpec {
                frame: Rect::new(20.0, 20.0, 40.0, 40.0),
                repaint_boundary: false,
                dirty: true,
                color: Color::from_rgba(255, 0, 0, 255),
            },
        );
        nodes.insert(
            SIBLING,
            NodeSpec {
                frame: Rect::new(100.0, 10.0, 50.0, 50.0),
                repaint_boundary: false,
                dirty: true,
                color: Color::from_rgba(0, 0, 255, 255),
            },
        );
        Self {
            nodes,
            dirty_region: DirtyRegion::full(),
            tree_version: 1,
            paint_counts: RefCell::new(HashMap::new()),
        }
    }

    fn mark_all_clean(&mut self) {
        for node in self.nodes.values_mut() {
            node.dirty = false;
        }
    }

    fn dirty_sibling_only(&mut self) {
        if let Some(s) = self.nodes.get_mut(&SIBLING) {
            s.dirty = true;
        }
        self.dirty_region = DirtyRegion::area(Rect::new(100.0, 10.0, 50.0, 50.0));
    }

    fn inner_paint_count(&self) -> u32 {
        self.paint_counts.borrow().get(&INNER).copied().unwrap_or(0)
    }
}

impl ScenePaint for BoundaryScene {
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
        self.nodes.contains_key(&id)
    }

    fn node_frame(&self, id: NodeId) -> Rect {
        self.nodes.get(&id).map(|n| n.frame).unwrap_or_default()
    }

    fn node_dirty(&self, id: NodeId) -> bool {
        self.nodes.get(&id).is_some_and(|n| n.dirty)
    }

    fn node_z_index(&self, id: NodeId) -> i32 {
        id as i32
    }

    fn node_children(&self, id: NodeId) -> &[NodeId] {
        match id {
            ROOT => &[BOUNDARY, SIBLING][..],
            BOUNDARY => &[INNER][..],
            _ => &[],
        }
    }

    fn is_repaint_boundary(&self, id: NodeId) -> bool {
        self.nodes.get(&id).is_some_and(|n| n.repaint_boundary)
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

    fn hit_test(&self, _: uix::native::Point) -> Option<NodeId> {
        None
    }

    fn parent(&self, id: NodeId) -> Option<NodeId> {
        match id {
            INNER => Some(BOUNDARY),
            BOUNDARY | SIBLING => Some(ROOT),
            _ => None,
        }
    }

    fn paint(&self, id: NodeId, frame: Rect, ctx: &mut PaintContext<'_>) {
        *self.paint_counts.borrow_mut().entry(id).or_insert(0) += 1;
        if let Some(node) = self.nodes.get(&id) {
            if node.color.a > 0 {
                ctx.fill_rect(frame, node.color, None);
            }
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

fn render_scene(engine: &mut SoftwareEngine, tree: &mut LayerTree, scene: &BoundaryScene) {
    let tokens = MockTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fs = FontService::new();
    engine.begin_frame(UpdateStrategy::FullRedraw);
    tree.render(
        engine,
        scene,
        &DirtyRegion::full(),
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
fn repaint_boundary_outer_change_skips_inner_rasterize() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(200, 200).unwrap();

    let mut scene = BoundaryScene::full_frame_dirty();
    let mut tree = LayerTree::new();
    tree.build(&scene);

    render_scene(&mut engine, &mut tree, &scene);
    assert_eq!(scene.inner_paint_count(), 1, "首帧应栅格化 boundary 内部");

    scene.mark_all_clean();
    scene.dirty_sibling_only();
    tree.update_dirty(&scene);

    render_scene(&mut engine, &mut tree, &scene);
    assert_eq!(
        scene.inner_paint_count(),
        1,
        "boundary 外 sibling 变更不应触发内部 re-render"
    );
}

#[test]
fn repaint_boundary_clean_path_blits_from_offscreen() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(200, 200).unwrap();

    let mut scene = BoundaryScene::full_frame_dirty();
    let mut tree = LayerTree::new();
    tree.build(&scene);
    render_scene(&mut engine, &mut tree, &scene);

    scene.mark_all_clean();
    // 仅标记 boundary 区域需要重绘（模拟父级间隙修正），内部仍 clean
    scene.dirty_region = DirtyRegion::area(Rect::new(10.0, 10.0, 80.0, 80.0));
    tree.update_dirty(&scene);

    let before = scene.inner_paint_count();
    render_scene(&mut engine, &mut tree, &scene);
    assert_eq!(
        scene.inner_paint_count(),
        before,
        "clean Picture 应 blit 缓存而非重绘内部"
    );
}

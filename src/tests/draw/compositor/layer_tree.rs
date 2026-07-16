use crate::draw::compositor::layer_tree::*;
use crate::draw::compositor::ScenePaint;
use crate::draw::traits::GraphicsEngine;
use crate::draw::Transform;
use crate::tests::common::*;

#[derive(Clone)]
struct TestNode {
    id: NodeId,
    frame: Rect,
    children: Vec<NodeId>,
    policy: PicturePolicy,
    has_handler: bool,
    dynamic: bool,
    interactive: bool,
    continuous_pointer: bool,
    overlay: bool,
    focusable: bool,
    clip: bool,
    scroll: bool,
    transform: Transform,
    opacity: f32,
}

impl TestNode {
    fn eligible(id: NodeId, children: Vec<NodeId>) -> Self {
        Self {
            id,
            frame: Rect::new(0.0, 0.0, 300.0, 300.0),
            children,
            policy: PicturePolicy::Eligible,
            has_handler: false,
            dynamic: false,
            interactive: false,
            continuous_pointer: false,
            overlay: false,
            focusable: false,
            clip: false,
            scroll: false,
            transform: Transform::identity(),
            opacity: 1.0,
        }
    }
}

struct TestScene {
    nodes: Vec<TestNode>,
    dirty_ids: HashSet<NodeId>,
    paint: Option<for<'a> fn(NodeId, &mut PaintContext<'a>)>,
}

impl TestScene {
    fn static_tree(node_count: usize) -> Self {
        let children = (2..=node_count).map(NodeId::new).collect();
        let mut nodes = vec![TestNode::eligible(NodeId::new(1), children)];
        for id in 2..=node_count {
            nodes.push(TestNode::eligible(NodeId::new(id), Vec::new()));
        }
        Self {
            nodes,
            dirty_ids: HashSet::new(),
            paint: None,
        }
    }

    fn node_mut(&mut self, id: NodeId) -> &mut TestNode {
        self.nodes.iter_mut().find(|node| node.id == id).unwrap()
    }

    fn node(&self, id: NodeId) -> &TestNode {
        self.nodes.iter().find(|node| node.id == id).unwrap()
    }

    fn build_layer_tree(&self) -> LayerTree {
        let mut tree = LayerTree::new();
        tree.build(self, true);
        tree
    }
}

impl ScenePaint for TestScene {
    fn root_id(&self) -> Option<NodeId> {
        Some(NodeId::new(1))
    }

    fn tree_version(&self) -> u64 {
        1
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::empty()
    }

    fn node_visible(&self, id: NodeId) -> bool {
        self.nodes.iter().any(|node| node.id == id)
    }

    fn node_frame(&self, id: NodeId) -> Rect {
        self.node(id).frame
    }

    fn node_dirty(&self, id: NodeId) -> bool {
        self.dirty_ids.contains(&id)
    }

    fn node_z_index(&self, _id: NodeId) -> i32 {
        0
    }

    fn node_transform(&self, id: NodeId) -> Transform {
        self.node(id).transform
    }

    fn node_opacity(&self, id: NodeId) -> f32 {
        self.node(id).opacity
    }

    fn node_children(&self, id: NodeId) -> &[NodeId] {
        &self.node(id).children
    }

    fn node_picture_policy(&self, id: NodeId) -> PicturePolicy {
        self.node(id).policy
    }

    fn node_has_semantic_handlers(&self, id: NodeId) -> bool {
        self.node(id).has_handler
    }

    fn node_has_dynamic_content(&self, id: NodeId) -> bool {
        self.node(id).dynamic
    }

    fn node_has_interactive_state(&self, id: NodeId) -> bool {
        self.node(id).interactive
    }

    fn node_wants_continuous_pointer_move(&self, id: NodeId) -> bool {
        self.node(id).continuous_pointer
    }

    fn node_is_overlay(&self, id: NodeId) -> bool {
        self.node(id).overlay
    }

    fn children_clip(&self, id: NodeId, frame: Rect) -> Option<Rect> {
        self.node(id).clip.then_some(frame)
    }

    fn dirty_rect(&self, _id: NodeId, frame: Rect) -> Rect {
        frame
    }

    fn scroll_offset(&self, id: NodeId) -> Option<(f32, f32)> {
        self.node(id).scroll.then_some((1.0, 0.0))
    }

    fn focused_node(&self) -> Option<NodeId> {
        None
    }

    fn node_focusable(&self, id: NodeId) -> bool {
        self.node(id).focusable
    }

    fn hit_test(&self, _pos: Point) -> Option<NodeId> {
        None
    }

    fn parent(&self, _id: NodeId) -> Option<NodeId> {
        None
    }

    fn paint(&self, id: NodeId, _frame: Rect, ctx: &mut PaintContext<'_>) {
        if let Some(paint) = self.paint {
            paint(id, ctx);
        }
    }
}

struct TestTokens;

macro_rules! test_color_tokens {
    ($($name:ident),+ $(,)?) => {
        $(
            fn $name(&self) -> crate::draw::Color {
                crate::draw::Color::black()
            }
        )+
    };
}

impl crate::draw::painting::IColorTokens for TestTokens {
    test_color_tokens!(
        color_primary,
        color_primary_hover,
        color_primary_active,
        color_primary_bg,
        color_primary_border,
        color_bg_container,
        color_bg_elevated,
        color_bg_raised,
        color_bg_overlay,
        color_bg_layout,
        color_bg_spotlight,
        color_bg_mask,
        color_border,
        color_border_secondary,
        color_fill,
        color_fill_secondary,
        color_fill_tertiary,
        color_fill_quaternary,
        color_text,
        color_text_secondary,
        color_text_tertiary,
        color_text_quaternary,
        color_white,
        color_black,
        color_shadow,
        color_shadow_secondary,
        color_success,
        color_success_bg,
        color_success_border,
        color_warning,
        color_warning_bg,
        color_warning_border,
        color_error,
        color_error_bg,
        color_error_border,
        color_info,
        color_info_bg,
        color_info_border,
        color_link,
        color_link_hover,
        color_link_active,
    );
}

impl crate::draw::painting::ITypographyTokens for TestTokens {
    fn font_family(&self) -> &str {
        "sans"
    }
}

impl crate::draw::painting::IBoxShadowTokens for TestTokens {
    fn box_shadow(&self) -> crate::draw::painting::ShadowToken {
        crate::draw::painting::ShadowToken::none()
    }

    fn box_shadow_secondary(&self) -> crate::draw::painting::ShadowToken {
        crate::draw::painting::ShadowToken::none()
    }
}

impl crate::draw::painting::ISpacingTokens for TestTokens {}
impl crate::draw::painting::ThemeTokens for TestTokens {}

#[test]
fn eligible_subtree_stays_direct_without_offscreen_support() {
    let scene = TestScene::static_tree(8);
    let mut tree = LayerTree::new();
    tree.build(&scene, false);

    assert!(matches!(
        tree.root_node(),
        Some(LayerNode::Direct { node_id, .. }) if *node_id == NodeId::new(1)
    ));
}

#[test]
fn eligible_large_static_subtree_builds_picture_layer() {
    let scene = TestScene::static_tree(8);
    let tree = scene.build_layer_tree();

    assert!(matches!(
        tree.root_node(),
        Some(LayerNode::Picture { node_id, .. }) if *node_id == NodeId::new(1)
    ));
}

#[test]
fn picture_policy_requires_node_count_and_pixel_thresholds() {
    let small_count = TestScene::static_tree(7).build_layer_tree();
    assert!(matches!(
        small_count.root_node(),
        Some(LayerNode::Direct { node_id, .. }) if *node_id == NodeId::new(1)
    ));

    let mut small_pixels = TestScene::static_tree(8);
    for node in &mut small_pixels.nodes {
        node.frame = Rect::new(0.0, 0.0, 10.0, 10.0);
    }
    let small_pixels = small_pixels.build_layer_tree();
    assert!(matches!(
        small_pixels.root_node(),
        Some(LayerNode::Direct { node_id, .. }) if *node_id == NodeId::new(1)
    ));
}

#[test]
fn runtime_signals_force_picture_policy_never_for_subtree() {
    let runtime_signals: [fn(&mut TestNode); 9] = [
        |node: &mut TestNode| node.has_handler = true,
        |node: &mut TestNode| node.dynamic = true,
        |node: &mut TestNode| node.interactive = true,
        |node: &mut TestNode| node.continuous_pointer = true,
        |node: &mut TestNode| node.overlay = true,
        |node: &mut TestNode| node.focusable = true,
        |node: &mut TestNode| node.scroll = true,
        |node: &mut TestNode| node.transform = Transform::translate(1.0, 0.0),
        |node: &mut TestNode| node.opacity = 0.5,
    ];

    for mark_runtime_signal in runtime_signals {
        let mut scene = TestScene::static_tree(8);
        mark_runtime_signal(scene.node_mut(NodeId::new(2)));
        let tree = scene.build_layer_tree();

        assert!(matches!(
            tree.root_node(),
            Some(LayerNode::Direct { node_id, .. }) if *node_id == NodeId::new(1)
        ));
    }
}

#[test]
fn transformed_ancestor_disables_descendant_picture_cache() {
    let mut scene = TestScene::static_tree(9);
    scene.node_mut(NodeId::new(1)).children = vec![NodeId::new(2)];
    scene.node_mut(NodeId::new(2)).children = (3..=9).map(NodeId::new).collect();
    scene.node_mut(NodeId::new(1)).transform = Transform::scale(1.1, 1.1);

    let tree = scene.build_layer_tree();
    let Some(LayerNode::Direct { children, .. }) = tree.root_node() else {
        panic!("transformed root must remain direct");
    };
    assert!(matches!(
        children.as_slice(),
        [LayerNode::Direct { node_id, .. }] if *node_id == NodeId::new(2)
    ));
}

#[test]
fn clip_nodes_remain_clip_layers_instead_of_picture_layers() {
    let mut scene = TestScene::static_tree(8);
    scene.node_mut(NodeId::new(1)).clip = true;
    let tree = scene.build_layer_tree();

    assert!(matches!(
        tree.root_node(),
        Some(LayerNode::ClipRect { node_id, .. }) if *node_id == NodeId::new(1)
    ));
}

#[test]
fn overlay_children_are_detached_from_clipped_ancestor_layers() {
    let mut scene = TestScene::static_tree(2);
    scene.node_mut(NodeId::new(1)).clip = true;
    scene.node_mut(NodeId::new(2)).overlay = true;
    let tree = scene.build_layer_tree();

    let Some(LayerNode::ClipRect { children, .. }) = tree.root_node() else {
        panic!("root should remain a clip layer");
    };
    assert!(
        children
            .iter()
            .all(|child| child.node_id() != NodeId::new(2)),
        "active overlays must not inherit an ancestor clip layer"
    );
    assert!(matches!(
        tree.overlay_nodes(),
        [LayerNode::Direct { node_id, .. }] if *node_id == NodeId::new(2)
    ));
}

/// Simulates a native canvas whose viewport `pop_clip` leaves its scissor in
/// effect. LayerTree must restore the root canvas state before painting a
/// detached overlay, otherwise a full-window overlay is cut to the normal
/// tree's ScrollView viewport.
struct LeakyClipCanvas {
    pixels: Vec<u32>,
    clip: Rect,
    transform: crate::draw::Transform,
    clip_stack: Vec<Rect>,
    state_stack: Vec<(Rect, crate::draw::Transform)>,
}

impl LeakyClipCanvas {
    fn new(width: i32, height: i32) -> Self {
        Self {
            pixels: vec![0; (width * height) as usize],
            clip: Rect::new(0.0, 0.0, width as f32, height as f32),
            transform: crate::draw::Transform::identity(),
            clip_stack: Vec::new(),
            state_stack: Vec::new(),
        }
    }
}

impl crate::draw::traits::Canvas2D for LeakyClipCanvas {
    fn save(&mut self) {
        self.state_stack.push((self.clip, self.transform));
    }

    fn restore(&mut self) {
        let (clip, transform) = self
            .state_stack
            .pop()
            .expect("restore must follow a matching save");
        self.clip = clip;
        self.transform = transform;
    }

    fn push_clip(&mut self, rect: Rect) {
        self.clip_stack.push(self.clip);
        self.clip = self.clip.intersect(&rect).unwrap_or_else(Rect::zero);
    }

    fn pop_clip(&mut self) {
        // Deliberately reproduce a backend clip-state leak.
    }

    fn set_opacity(&mut self, _: f32) {}

    fn opacity(&self) -> f32 {
        1.0
    }

    fn current_transform(&self) -> crate::draw::Transform {
        self.transform
    }

    fn set_transform(&mut self, transform: crate::draw::Transform) {
        self.transform = transform;
    }

    fn set_blend_mode(&mut self, _: crate::draw::BlendMode) {}

    fn push_clip_path(&mut self, _: &crate::draw::Path) {}

    fn pixels_mut(&mut self) -> &mut [u32] {
        &mut self.pixels
    }

    fn surface_size(&self) -> crate::core::Size {
        crate::core::Size::new(256.0, 256.0)
    }

    fn current_clip(&self) -> Rect {
        self.clip
    }

    fn scroll_region(&mut self, _: Rect, _: f32, _: f32) {
        // Test double: this regression isolates clip-stack restoration.
    }
}

struct LeakyClipEngine {
    canvas: LeakyClipCanvas,
    destroy_error: Option<crate::core::Error>,
    destroy_attempts: usize,
}

impl LeakyClipEngine {
    fn new() -> Self {
        Self {
            canvas: LeakyClipCanvas::new(256, 256),
            destroy_error: None,
            destroy_attempts: 0,
        }
    }
}

impl GraphicsEngine for LeakyClipEngine {
    fn initialize(&mut self, _: i32, _: i32) -> Result<(), crate::core::Error> {
        Ok(())
    }

    fn try_shutdown(&mut self) -> Result<(), crate::core::Error> {
        Ok(())
    }

    fn resize(&mut self, _: i32, _: i32) -> Result<(), crate::core::Error> {
        Ok(())
    }

    fn begin_frame(
        &mut self,
        _: crate::draw::traits::UpdateStrategy,
    ) -> crate::draw::engine::RenderOutcome {
        crate::draw::engine::RenderOutcome::FrameReady(crate::draw::backend::DamageRegion::full())
    }

    fn end_frame(
        &mut self,
        damage: &crate::draw::backend::DamageRegion,
    ) -> crate::draw::engine::RenderOutcome {
        crate::draw::engine::RenderOutcome::Present(damage.clone())
    }

    fn canvas_2d(&mut self) -> &mut dyn crate::draw::traits::Canvas2D {
        &mut self.canvas
    }

    fn try_destroy_offscreen(&mut self, _handle: ImageHandle) -> Result<(), crate::core::Error> {
        self.destroy_attempts += 1;
        match &self.destroy_error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

#[test]
fn orphaned_offscreen_sweep_preserves_unreleased_handles_after_checked_failure() {
    let mut tree = LayerTree::new();
    tree.orphaned_handles_mut()
        .extend([ImageHandle(7), ImageHandle(11)]);
    let mut engine = LeakyClipEngine::new();
    engine.destroy_error = Some(crate::core::Error::new(
        crate::core::Errc::GraphicsDeviceLost,
        "injected orphaned Picture destroy failure",
    ));

    let error = tree
        .sweep_orphaned_offscreens(&mut engine)
        .expect_err("orphan sweep must propagate its first checked destroy failure");
    assert_eq!(error.code(), crate::core::Errc::GraphicsDeviceLost);
    assert_eq!(tree.orphaned_handles(), &[ImageHandle(7), ImageHandle(11)]);
    assert_eq!(engine.destroy_attempts, 1);

    engine.destroy_error = None;
    tree.sweep_orphaned_offscreens(&mut engine)
        .expect("retained orphan handles must be retryable");
    assert!(tree.orphaned_handles().is_empty());
    assert_eq!(engine.destroy_attempts, 3);
}

fn paint_overlay_outside_viewport(id: NodeId, ctx: &mut PaintContext<'_>) {
    if id == NodeId::new(2) {
        ctx.fill_rect(
            Rect::new(180.0, 24.0, 32.0, 32.0),
            crate::draw::Color::red(),
            None,
        );
    }
}

#[test]
fn detached_overlay_restores_canvas_state_after_a_clipped_root() {
    use crate::draw::painting::ThemeSnapshot;

    let mut scene = TestScene::static_tree(2);
    scene.node_mut(NodeId::new(1)).frame = Rect::new(0.0, 0.0, 100.0, 100.0);
    scene.node_mut(NodeId::new(1)).clip = true;
    scene.node_mut(NodeId::new(2)).frame = Rect::zero();
    scene.node_mut(NodeId::new(2)).overlay = true;
    scene.dirty_ids.insert(NodeId::new(2));
    scene.paint = Some(paint_overlay_outside_viewport);

    let mut tree = scene.build_layer_tree();
    let mut engine = LeakyClipEngine::new();
    let tokens = TestTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fonts = FontService::new();
    let images = ImageService::new();

    tree.render(
        &mut engine,
        &scene,
        &DirtyRegion::full(),
        &theme,
        FontHandle::default(),
        &fonts,
        &images,
        false,
        None,
        None,
    )
    .expect("test tree rendering");

    assert_ne!(
        engine.canvas.pixels[24 * 256 + 180],
        0,
        "detached overlay must paint outside the normal tree's viewport clip"
    );
}

fn paint_affine_target(id: NodeId, ctx: &mut PaintContext<'_>) {
    if id == NodeId::new(1) {
        ctx.fill_rect(
            Rect::new(1.0, 1.0, 2.0, 2.0),
            crate::draw::Color::red(),
            None,
        );
    }
}

#[test]
fn direct_layer_applies_scene_transform_to_widget_paint() {
    use crate::draw::painting::ThemeSnapshot;

    let mut scene = TestScene::static_tree(1);
    scene.node_mut(NodeId::new(1)).transform =
        Transform::translate(4.0, 3.0).concat(Transform::scale(2.0, 2.0));
    scene.dirty_ids.insert(NodeId::new(1));
    scene.paint = Some(paint_affine_target);

    let mut tree = LayerTree::new();
    tree.build(&scene, false);
    let mut engine = SoftwareEngine::new();
    engine.initialize(20, 16).expect("software engine init");
    let tokens = TestTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fonts = FontService::new();
    let images = ImageService::new();

    tree.render(
        &mut engine,
        &scene,
        &DirtyRegion::full(),
        &theme,
        FontHandle::default(),
        &fonts,
        &images,
        false,
        None,
        None,
    )
    .expect("transformed layer rendering");

    let pixels = engine.canvas_2d().pixels_mut();
    let at = |x: usize, y: usize| pixels[y * 20 + x];
    assert_eq!(at(1, 1), 0, "untransformed location must remain empty");
    assert_ne!(at(6, 5), 0, "mapped top-left pixel must be painted");
    assert_ne!(at(9, 8), 0, "mapped lower-right pixel must be painted");
    assert_eq!(at(10, 8), 0, "mapped width must remain bounded");
}

fn paint_opacity_target(id: NodeId, ctx: &mut PaintContext<'_>) {
    if id == NodeId::new(2) {
        ctx.fill_rect(
            Rect::new(1.0, 1.0, 2.0, 2.0),
            crate::draw::Color::red(),
            None,
        );
    }
}

#[test]
fn direct_layers_multiply_parent_and_child_opacity() {
    use crate::draw::painting::ThemeSnapshot;

    let mut scene = TestScene::static_tree(2);
    scene.node_mut(NodeId::new(1)).opacity = 0.5;
    scene.node_mut(NodeId::new(2)).opacity = 0.5;
    scene.dirty_ids.extend([NodeId::new(1), NodeId::new(2)]);
    scene.paint = Some(paint_opacity_target);

    let mut tree = LayerTree::new();
    tree.build(&scene, false);
    let mut engine = SoftwareEngine::new();
    engine.initialize(8, 8).expect("software engine init");
    let tokens = TestTokens;
    let theme = ThemeSnapshot::new(&tokens);
    let fonts = FontService::new();
    let images = ImageService::new();

    tree.render(
        &mut engine,
        &scene,
        &DirtyRegion::full(),
        &theme,
        FontHandle::default(),
        &fonts,
        &images,
        false,
        None,
        None,
    )
    .expect("opacity layer rendering");

    let pixel = engine.canvas_2d().pixels_mut()[1 * 8 + 1];
    let alpha = (pixel >> 24) & 0xff;
    assert!(
        (63..=64).contains(&alpha),
        "nested 0.5 opacity should yield quarter alpha, got {alpha}"
    );
}

#[test]
fn picture_partial_dirty_rerasterize_repaints_all_direct_children() {
    use crate::draw::painting::ThemeSnapshot;
    use std::collections::HashSet;

    struct PaintCountScene {
        base: TestScene,
        painted: RefCell<HashSet<NodeId>>,
        dirty_ids: HashSet<NodeId>,
        region: DirtyRegion,
    }

    impl ScenePaint for PaintCountScene {
        fn root_id(&self) -> Option<NodeId> {
            self.base.root_id()
        }
        fn tree_version(&self) -> u64 {
            1
        }
        fn dirty_region(&self) -> DirtyRegion {
            self.region.clone()
        }
        fn node_visible(&self, id: NodeId) -> bool {
            self.base.node_visible(id)
        }
        fn node_frame(&self, id: NodeId) -> Rect {
            // 子节点纵向错开，便于构造「仅一项与 dirty 相交」
            match id {
                id if id == NodeId::new(1) => Rect::new(0.0, 0.0, 220.0, 400.0),
                id if id.slot() >= 2 => {
                    let i = (id.slot() - 2) as f32;
                    Rect::new(8.0, 40.0 + i * 40.0, 200.0, 36.0)
                }
                _ => Rect::zero(),
            }
        }
        fn node_dirty(&self, id: NodeId) -> bool {
            self.dirty_ids.contains(&id)
        }
        fn node_z_index(&self, id: NodeId) -> i32 {
            self.base.node_z_index(id)
        }
        fn node_children(&self, id: NodeId) -> &[NodeId] {
            self.base.node_children(id)
        }
        fn node_picture_policy(&self, id: NodeId) -> PicturePolicy {
            self.base.node_picture_policy(id)
        }
        fn children_clip(&self, id: NodeId, frame: Rect) -> Option<Rect> {
            self.base.children_clip(id, frame)
        }
        fn dirty_rect(&self, _id: NodeId, frame: Rect) -> Rect {
            frame
        }
        fn scroll_offset(&self, id: NodeId) -> Option<(f32, f32)> {
            self.base.scroll_offset(id)
        }
        fn focused_node(&self) -> Option<NodeId> {
            None
        }
        fn node_focusable(&self, id: NodeId) -> bool {
            self.base.node_focusable(id)
        }
        fn hit_test(&self, _: Point) -> Option<NodeId> {
            None
        }
        fn parent(&self, id: NodeId) -> Option<NodeId> {
            if id.slot() >= 2 {
                Some(NodeId::new(1))
            } else {
                None
            }
        }
        fn paint(&self, id: NodeId, _: Rect, _: &mut PaintContext<'_>) {
            self.painted.borrow_mut().insert(id);
        }
    }

    // 8 节点：根成 Picture，子项各自未达阈值 → Direct（与侧栏 Column+Label 同构）
    let base = TestScene::static_tree(8);
    let mut tree = base.build_layer_tree();
    assert!(matches!(tree.root_node(), Some(LayerNode::Picture { .. })));

    // 仅标脏第 2 项（y≈80），与第 7 项（y≈280）不相交
    let hover_child = NodeId::new(3);
    let far_child = NodeId::new(8);
    let mut region = DirtyRegion::empty();
    region.add_rect(Rect::new(8.0, 80.0, 200.0, 36.0));
    let scene = PaintCountScene {
        base,
        painted: RefCell::new(HashSet::new()),
        dirty_ids: HashSet::from([hover_child]),
        region: region.clone(),
    };

    let mut engine = SoftwareEngine::new();
    engine.initialize(256, 512).expect("init");
    tree.update_dirty(&scene);
    // 最小 ThemeTokens，供 LayerTree.render 使用
    struct Tok;
    impl crate::draw::painting::IColorTokens for Tok {
        fn color_primary(&self) -> crate::draw::Color {
            crate::draw::Color::blue()
        }
        fn color_primary_hover(&self) -> crate::draw::Color {
            crate::draw::Color::blue()
        }
        fn color_primary_active(&self) -> crate::draw::Color {
            crate::draw::Color::blue()
        }
        fn color_primary_bg(&self) -> crate::draw::Color {
            crate::draw::Color::blue()
        }
        fn color_primary_border(&self) -> crate::draw::Color {
            crate::draw::Color::blue()
        }
        fn color_bg_container(&self) -> crate::draw::Color {
            crate::draw::Color::white()
        }
        fn color_bg_elevated(&self) -> crate::draw::Color {
            crate::draw::Color::white()
        }
        fn color_bg_raised(&self) -> crate::draw::Color {
            crate::draw::Color::white()
        }
        fn color_bg_overlay(&self) -> crate::draw::Color {
            crate::draw::Color::white()
        }
        fn color_bg_layout(&self) -> crate::draw::Color {
            crate::draw::Color::white()
        }
        fn color_bg_spotlight(&self) -> crate::draw::Color {
            crate::draw::Color::white()
        }
        fn color_bg_mask(&self) -> crate::draw::Color {
            crate::draw::Color::white()
        }
        fn color_border(&self) -> crate::draw::Color {
            crate::draw::Color::black()
        }
        fn color_border_secondary(&self) -> crate::draw::Color {
            crate::draw::Color::black()
        }
        fn color_fill(&self) -> crate::draw::Color {
            crate::draw::Color::black()
        }
        fn color_fill_secondary(&self) -> crate::draw::Color {
            crate::draw::Color::black()
        }
        fn color_fill_tertiary(&self) -> crate::draw::Color {
            crate::draw::Color::black()
        }
        fn color_fill_quaternary(&self) -> crate::draw::Color {
            crate::draw::Color::black()
        }
        fn color_text(&self) -> crate::draw::Color {
            crate::draw::Color::black()
        }
        fn color_text_secondary(&self) -> crate::draw::Color {
            crate::draw::Color::black()
        }
        fn color_text_tertiary(&self) -> crate::draw::Color {
            crate::draw::Color::black()
        }
        fn color_text_quaternary(&self) -> crate::draw::Color {
            crate::draw::Color::black()
        }
        fn color_white(&self) -> crate::draw::Color {
            crate::draw::Color::white()
        }
        fn color_black(&self) -> crate::draw::Color {
            crate::draw::Color::black()
        }
        fn color_shadow(&self) -> crate::draw::Color {
            crate::draw::Color::black()
        }
        fn color_shadow_secondary(&self) -> crate::draw::Color {
            crate::draw::Color::black()
        }
        fn color_success(&self) -> crate::draw::Color {
            crate::draw::Color::green()
        }
        fn color_success_bg(&self) -> crate::draw::Color {
            crate::draw::Color::green()
        }
        fn color_success_border(&self) -> crate::draw::Color {
            crate::draw::Color::green()
        }
        fn color_warning(&self) -> crate::draw::Color {
            crate::draw::Color::from_rgb(255, 200, 0)
        }
        fn color_warning_bg(&self) -> crate::draw::Color {
            crate::draw::Color::from_rgb(255, 200, 0)
        }
        fn color_warning_border(&self) -> crate::draw::Color {
            crate::draw::Color::from_rgb(255, 200, 0)
        }
        fn color_error(&self) -> crate::draw::Color {
            crate::draw::Color::red()
        }
        fn color_error_bg(&self) -> crate::draw::Color {
            crate::draw::Color::red()
        }
        fn color_error_border(&self) -> crate::draw::Color {
            crate::draw::Color::red()
        }
        fn color_info(&self) -> crate::draw::Color {
            crate::draw::Color::blue()
        }
        fn color_info_bg(&self) -> crate::draw::Color {
            crate::draw::Color::blue()
        }
        fn color_info_border(&self) -> crate::draw::Color {
            crate::draw::Color::blue()
        }
        fn color_link(&self) -> crate::draw::Color {
            crate::draw::Color::blue()
        }
        fn color_link_hover(&self) -> crate::draw::Color {
            crate::draw::Color::blue()
        }
        fn color_link_active(&self) -> crate::draw::Color {
            crate::draw::Color::blue()
        }
    }
    impl crate::draw::painting::ITypographyTokens for Tok {
        fn font_family(&self) -> &str {
            "sans"
        }
    }
    impl crate::draw::painting::IBoxShadowTokens for Tok {
        fn box_shadow(&self) -> crate::draw::painting::ShadowToken {
            crate::draw::painting::ShadowToken::none()
        }
        fn box_shadow_secondary(&self) -> crate::draw::painting::ShadowToken {
            crate::draw::painting::ShadowToken::none()
        }
    }
    impl crate::draw::painting::ISpacingTokens for Tok {}
    impl crate::draw::painting::ThemeTokens for Tok {}

    let tok = Tok;
    let theme = ThemeSnapshot::new(&tok);
    let fs = FontService::new();
    let img = ImageService::new();

    engine.begin_frame(crate::draw::traits::UpdateStrategy::FullRedraw);
    tree.render(
        &mut engine,
        &scene,
        &region,
        &theme,
        FontHandle::default(),
        &fs,
        &img,
        false,
        None,
        None,
    )
    .expect("test tree rendering");
    engine.end_frame(&crate::draw::backend::DamageRegion::full());

    let painted = scene.painted.borrow().clone();
    assert!(painted.contains(&hover_child), "hovered child must repaint");
    assert!(
        painted.contains(&far_child),
        "sibling outside screen dirty must still repaint after Picture clear"
    );
    assert!(
        painted.contains(&NodeId::new(1)),
        "picture root content must repaint"
    );
}

use crate::draw::compositor::ScenePaint;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::render_object::tree::*;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;

struct AlphaScene;

impl ScenePaint for AlphaScene {
    fn root_id(&self) -> Option<NodeId> {
        Some(NodeId::new(1))
    }
    fn tree_version(&self) -> u64 {
        1
    }
    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::full()
    }
    fn node_visible(&self, _: NodeId) -> bool {
        true
    }
    fn node_frame(&self, _: NodeId) -> Rect {
        Rect::new(0.0, 0.0, 8.0, 8.0)
    }
    fn node_dirty(&self, _: NodeId) -> bool {
        true
    }
    fn node_z_index(&self, _: NodeId) -> i32 {
        0
    }
    fn node_children(&self, _: NodeId) -> &[NodeId] {
        &[]
    }
    fn node_picture_policy(&self, _: NodeId) -> PicturePolicy {
        PicturePolicy::Never
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
    fn parent(&self, _: NodeId) -> Option<NodeId> {
        None
    }
    fn paint(&self, _: NodeId, frame: Rect, ctx: &mut PaintContext<'_>) {
        ctx.fill_rect(frame, Color::from_rgba(255, 0, 0, 128), None);
    }
}

#[test]
fn recording_does_not_replay_over_the_live_paint_in_the_same_frame() {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(8, 8));
    let fonts = FontService::new();
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new(
        &mut canvas,
        FontHandle::default(),
        &fonts,
        &images,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        8,
        8,
    );
    let mut tree = RenderObjectTree::new();

    tree.paint_content(
        NodeId::new(1),
        Rect::new(0.0, 0.0, 8.0, 8.0),
        &AlphaScene,
        &mut ctx,
    );
    drop(ctx);

    let pixel = canvas.surface().pixels()[0];
    assert_eq!((pixel >> 24) & 0xff, 128, "alpha paint must run once");
    assert!(tree
        .get(NodeId::new(1))
        .and_then(|entry| entry.display_list.as_ref())
        .is_some());
}

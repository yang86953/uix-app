use crate::tests::common::*;
use crate::draw::compositor::ScenePaint;
use crate::draw::compositor::viewport_transform::*;


const ROOT: NodeId = NodeId::new(1);
const CHILD: NodeId = NodeId::new(2);
const CHILDREN: &[NodeId] = &[CHILD];

struct ScrollScene {
    region: DirtyRegion,
    scroll_y: f32,
}

impl ScrollScene {
    fn with_strip_at_viewport_bottom() -> Self {
        // viewport (0,0,100,100)，strip 在底部 10px
        Self {
            region: DirtyRegion::area(Rect::new(0.0, 90.0, 100.0, 10.0)),
            scroll_y: 50.0,
        }
    }
}

impl ScenePaint for ScrollScene {
    fn root_id(&self) -> Option<NodeId> {
        Some(ROOT)
    }
    fn tree_version(&self) -> u64 {
        1
    }
    fn dirty_region(&self) -> DirtyRegion {
        self.region.clone()
    }
    fn node_visible(&self, _: NodeId) -> bool {
        true
    }
    fn node_frame(&self, id: NodeId) -> Rect {
        match id {
            ROOT => Rect::new(0.0, 0.0, 100.0, 100.0),
            // content 坐标：子节点在 scroll 后可见区底部
            CHILD => Rect::new(0.0, 140.0, 100.0, 20.0),
            _ => Rect::zero(),
        }
    }
    fn node_dirty(&self, _: NodeId) -> bool {
        false
    }
    fn node_z_index(&self, _: NodeId) -> i32 {
        0
    }
    fn node_children(&self, id: NodeId) -> &[NodeId] {
        match id {
            ROOT => CHILDREN,
            _ => &[],
        }
    }
    fn children_clip(&self, id: NodeId, _: Rect) -> Option<Rect> {
        if id == ROOT {
            Some(Rect::new(0.0, 0.0, 100.0, 100.0))
        } else {
            None
        }
    }
    fn dirty_rect(&self, _: NodeId, frame: Rect) -> Rect {
        frame
    }
    fn scroll_offset(&self, id: NodeId) -> Option<(f32, f32)> {
        if id == ROOT {
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
    fn hit_test(&self, _: Point) -> Option<NodeId> {
        None
    }
    fn parent(&self, id: NodeId) -> Option<NodeId> {
        if id == CHILD {
            Some(ROOT)
        } else {
            None
        }
    }
    fn paint(&self, _: NodeId, _: Rect, _: &mut PaintContext<'_>) {}
}

#[test]
fn content_to_viewport_maps_scroll_child_into_visible_strip() {
    let scene = ScrollScene::with_strip_at_viewport_bottom();
    // child y=140, scroll_y=50 → viewport y=90，高度 20 → 与 strip [90,100] 相交
    assert!(needs_paint(&scene, CHILD, &scene.dirty_region()));
}

#[test]
fn node_viewport_frame_follows_scroll_offset() {
    let scene = ScrollScene::with_strip_at_viewport_bottom();
    // content (0,140) − scroll 50 → viewport (0,90)
    assert_eq!(
        node_viewport_frame(&scene, CHILD),
        Rect::new(0.0, 90.0, 100.0, 20.0)
    );
}

#[test]
fn visible_viewport_rect_clips_to_scroll_viewport() {
    let scene = ScrollScene::with_strip_at_viewport_bottom();
    // child 投影 [90,110)，viewport clip [0,100) → 可见 [90,100)
    assert_eq!(
        visible_viewport_rect(&scene, CHILD),
        Some(Rect::new(0.0, 90.0, 100.0, 10.0))
    );
}

#[test]
fn visible_viewport_rect_none_when_fully_scrolled_out() {
    let scene = ScrollScene {
        region: DirtyRegion::full(),
        scroll_y: 200.0,
    };
    // child y=140 − 200 = -60，与 viewport [0,100) 无交
    assert!(visible_viewport_rect(&scene, CHILD).is_none());
}

#[test]
fn content_frame_without_transform_misses_strip() {
    let scene = ScrollScene::with_strip_at_viewport_bottom();
    let frame = scene.node_frame(CHILD);
    assert!(!scene.dirty_region().intersects(frame));
}

#[test]
fn far_content_child_skipped_when_not_dirty() {
    let mut scene = ScrollScene::with_strip_at_viewport_bottom();
    // 子节点远在 content 顶部，scroll 后不在 strip
    scene.region = DirtyRegion::area(Rect::new(0.0, 90.0, 100.0, 10.0));
    let scene = scene;
    // 伪造 content 顶部子节点 id=3
    assert!(!needs_paint_rect(
        &scene,
        CHILD,
        Rect::new(0.0, 10.0, 100.0, 20.0),
        &scene.dirty_region()
    ));
}

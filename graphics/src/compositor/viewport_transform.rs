//! Viewport 坐标变换 — content 与 viewport 空间统一（Phase 5）。
//!
//! 子节点 frame 处于 content 坐标（ScrollView layout 自然坐标），
//! dirty_region 处于 viewport/screen 坐标；绘制时 canvas translate(-scroll) 对齐。
//! 脏剪枝须先将 content frame 映射到 viewport 再与 dirty_region 求交。

use uix_platform::Rect;

use crate::compositor::ScenePaint;
use crate::pipeline::NodeId;
use crate::types::DirtyRegion;

/// 累计从根到 node 路径上所有 viewport 祖先的 scroll 偏移。
pub fn cumulative_scroll(scene: &impl ScenePaint, node_id: NodeId) -> (f32, f32) {
    let mut sx = 0.0f32;
    let mut sy = 0.0f32;
    let mut current = scene.parent(node_id);
    while let Some(pid) = current {
        if is_viewport(scene, pid) {
            if let Some((ox, oy)) = scene.scroll_offset(pid) {
                sx += ox;
                sy += oy;
            }
        }
        current = scene.parent(pid);
    }
    (sx, sy)
}

/// content 坐标矩形 → viewport 可见坐标（与 dirty_region 同空间）。
pub fn content_to_viewport(content: Rect, scroll_x: f32, scroll_y: f32) -> Rect {
    Rect::new(
        content.x - scroll_x,
        content.y - scroll_y,
        content.w,
        content.h,
    )
}

/// 节点是否需绘制：自身脏，或其 viewport 投影与 dirty_region 相交。
pub fn needs_paint(scene: &impl ScenePaint, node_id: NodeId, dirty_region: &DirtyRegion) -> bool {
    if scene.node_dirty(node_id) {
        return true;
    }
    let frame = scene.node_frame(node_id);
    if frame.w <= 0.0 || frame.h <= 0.0 {
        return false;
    }
    let (sx, sy) = cumulative_scroll(scene, node_id);
    let vp = content_to_viewport(frame, sx, sy);
    dirty_region.intersects(vp)
}

/// overlay / dirty_rect 区域是否需重绘。
pub fn needs_paint_rect(
    scene: &impl ScenePaint,
    node_id: NodeId,
    area: Rect,
    dirty_region: &DirtyRegion,
) -> bool {
    if area.w <= 0.0 || area.h <= 0.0 {
        return false;
    }
    let (sx, sy) = cumulative_scroll(scene, node_id);
    let vp = content_to_viewport(area, sx, sy);
    dirty_region.intersects(vp)
}

fn is_viewport(scene: &impl ScenePaint, id: NodeId) -> bool {
    scene
        .children_clip(id, scene.node_frame(id))
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uix_platform::Point;

    use crate::painting::PaintContext;

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
            Some(1)
        }
        fn tree_version(&self) -> u64 {
            1
        }
        fn dirty_region(&self) -> &DirtyRegion {
            &self.region
        }
        fn node_visible(&self, _: NodeId) -> bool {
            true
        }
        fn node_frame(&self, id: NodeId) -> Rect {
            match id {
                1 => Rect::new(0.0, 0.0, 100.0, 100.0),
                // content 坐标：子节点在 scroll 后可见区底部
                2 => Rect::new(0.0, 140.0, 100.0, 20.0),
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
                1 => &[2][..],
                _ => &[],
            }
        }
        fn is_repaint_boundary(&self, _: NodeId) -> bool {
            false
        }
        fn children_clip(&self, id: NodeId, _: Rect) -> Option<Rect> {
            if id == 1 {
                Some(Rect::new(0.0, 0.0, 100.0, 100.0))
            } else {
                None
            }
        }
        fn dirty_rect(&self, _: NodeId, frame: Rect) -> Rect {
            frame
        }
        fn scroll_offset(&self, id: NodeId) -> Option<(f32, f32)> {
            if id == 1 {
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
            if id == 2 {
                Some(1)
            } else {
                None
            }
        }
        fn paint(&self, _: NodeId, _: Rect, _: &mut PaintContext<'_>) {}
        fn paint_overlay(&self, _: NodeId, _: Rect, _: &mut PaintContext<'_>) {}
    }

    #[test]
    fn content_to_viewport_maps_scroll_child_into_visible_strip() {
        let scene = ScrollScene::with_strip_at_viewport_bottom();
        // child y=140, scroll_y=50 → viewport y=90，高度 20 → 与 strip [90,100] 相交
        assert!(needs_paint(&scene, 2, scene.dirty_region()));
    }

    #[test]
    fn content_frame_without_transform_misses_strip() {
        let scene = ScrollScene::with_strip_at_viewport_bottom();
        let frame = scene.node_frame(2);
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
            2,
            Rect::new(0.0, 10.0, 100.0, 20.0),
            scene.dirty_region()
        ));
    }
}

use super::tree_core::WidgetTree;
use super::*;
use crate::draw::Transform;
use crate::ui::component::view_transform::ViewTransform;

impl WidgetTree {
    fn is_overlay_node(&self, id: WidgetId) -> bool {
        self.get(id)
            .is_some_and(|node| node.overlay_entry(id, node.frame()).is_some())
    }

    fn visual_path(&self, id: WidgetId) -> Vec<WidgetId> {
        let mut path = Vec::new();
        let mut current = Some(id);
        while let Some(current_id) = current {
            path.push(current_id);
            if self.is_overlay_node(current_id) {
                break;
            }
            current = self.get(current_id).and_then(|node| node.parent());
        }
        path.reverse();
        path
    }

    pub(crate) fn node_visual_transform(&self, id: WidgetId) -> Option<Transform> {
        let path = self.visual_path(id);
        if path.is_empty() {
            return None;
        }
        let mut transform = Transform::identity();
        for (index, current_id) in path.iter().copied().enumerate() {
            let node = self.get(current_id)?;
            transform = transform.concat(node.visual_transform_matrix());
            if index + 1 < path.len() {
                if let Some((sx, sy)) = node.viewport_scroll_offset() {
                    transform = transform.concat(Transform::translate(-sx, -sy));
                }
            }
        }
        Some(transform)
    }

    pub(crate) fn node_visual_rect(&self, id: WidgetId, rect: Rect) -> Option<Rect> {
        self.node_visual_transform(id)
            .map(|transform| transform.transform_rect(rect))
    }

    pub(crate) fn point_to_node_layout(&self, id: WidgetId, point: Point) -> Option<Point> {
        Some(
            self.node_visual_transform(id)?
                .inverse()?
                .transform_point(point),
        )
    }

    pub(crate) fn path_has_visual_transform(&self, id: WidgetId) -> bool {
        self.visual_path(id).into_iter().any(|current_id| {
            self.get(current_id)
                .is_some_and(BoxedWidget::has_effective_visual_transform)
        })
    }

    pub(crate) fn visual_subtree_bounds(&self, id: WidgetId) -> Option<Rect> {
        let node = self.get(id)?;
        if !node.visible() {
            return None;
        }
        let frame = node.frame();
        let dirty = node.dirty_rect(frame);
        let geometry = if dirty.w > 0.0 && dirty.h > 0.0 {
            dirty
        } else if frame.w > 0.0 && frame.h > 0.0 {
            frame
        } else {
            node.hit_test_frame(frame)
        };
        let mut bounds = (geometry.w > 0.0 && geometry.h > 0.0)
            .then(|| self.node_visual_rect(id, geometry))
            .flatten();

        for child in node.children().iter().copied() {
            if self.is_overlay_node(child) {
                continue;
            }
            if let Some(child_bounds) = self.visual_subtree_bounds(child) {
                bounds = Some(match bounds {
                    Some(current) => union_rect(current, child_bounds),
                    None => child_bounds,
                });
            }
        }
        bounds
    }

    pub(crate) fn visible_visual_rect_for(&self, id: WidgetId) -> Option<Rect> {
        if !self.is_effectively_visible(id) {
            return None;
        }
        let node = self.get(id)?;
        let frame = node.frame();
        let base = if frame.w > 0.0 && frame.h > 0.0 {
            frame
        } else {
            node.hit_test_frame(frame)
        };
        if base.w <= 0.0 || base.h <= 0.0 {
            return None;
        }
        let mut rect = self.node_visual_rect(id, base)?;
        let path = self.visual_path(id);
        let mut transform = Transform::identity();
        for (index, current_id) in path.iter().copied().enumerate() {
            let current = self.get(current_id)?;
            if !current.visible() {
                return None;
            }
            transform = transform.concat(current.visual_transform_matrix());
            if index + 1 < path.len() {
                if let Some(clip) = current.children_clip(current.frame()) {
                    rect = rect.intersect(&transform.transform_rect(clip))?;
                }
                if let Some((sx, sy)) = current.viewport_scroll_offset() {
                    transform = transform.concat(Transform::translate(-sx, -sy));
                }
            }
        }
        Some(rect)
    }

    pub(crate) fn set_visual_transform(&mut self, id: WidgetId, transform: ViewTransform) -> bool {
        let Some(current) = self.get(id).map(BoxedWidget::visual_transform) else {
            return false;
        };
        if current == transform {
            return false;
        }
        let old_bounds = self.visual_subtree_bounds(id);
        if let Some(node) = self.get_mut(id) {
            node.set_visual_transform(transform);
        }
        self.tree_version = self.tree_version.wrapping_add(1);
        let new_bounds = self.visual_subtree_bounds(id);
        for rect in [old_bounds, new_bounds].into_iter().flatten() {
            if rect.w > 0.0 && rect.h > 0.0 {
                self.push_paint_invalidation(id, Some(rect));
            }
        }
        true
    }
}

fn union_rect(a: Rect, b: Rect) -> Rect {
    let x1 = a.x.min(b.x);
    let y1 = a.y.min(b.y);
    let x2 = (a.x + a.w).max(b.x + b.w);
    let y2 = (a.y + a.h).max(b.y + b.h);
    Rect::new(x1, y1, x2 - x1, y2 - y1)
}

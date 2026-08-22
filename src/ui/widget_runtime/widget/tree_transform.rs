use super::tree_core::WidgetTree;
use super::*;
use crate::draw::Transform;
use crate::ui::widget_runtime::view_transform::ViewTransform;

// 把视觉边界裁剪契约的验收放在独立文件，避免机制实现与测试相互挤占篇幅。
#[cfg(test)]
#[path = "../../../../tests/unit/ui/widget_runtime/widget/tree_transform__tests.rs"]
mod tests;

impl WidgetTree {
    /// 判断节点是否为悬浮层节点（overlay 挂载点）。
    fn is_overlay_node(&self, id: WidgetId) -> bool {
        // fixed 节点与组件浮层都从祖先滚动和裁剪路径中提升。
        self.node_is_fixed(id)
            || self
                // 读取组件自身声明的浮层入口。
                .get(id)
                // 存在浮层条目时截断视觉父链。
                .is_some_and(|node| node.overlay_entry(id, node.frame()).is_some())
    }

    /// 计算从树根到指定节点的视觉路径（遇悬浮层节点截断，根在前）。
    fn visual_path(&self, id: WidgetId) -> Vec<WidgetId> {
        let mut path = Vec::new();
        let mut current = Some(id);
        // 沿父链向上收集，直到树根或悬浮层节点。
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

    /// 计算节点到屏幕的视觉变换：沿视觉路径逐级串联矩阵并叠加滚动偏移。
    pub(crate) fn node_visual_transform(&self, id: WidgetId) -> Option<Transform> {
        let path = self.visual_path(id);
        if path.is_empty() {
            return None;
        }
        let mut transform = Transform::identity();
        for (index, current_id) in path.iter().copied().enumerate() {
            let node = self.get(current_id)?;
            // 合成 relative/sticky 定位偏移与作者视觉变换。
            transform = transform.concat(self.positioned_visual_transform(current_id));
            // 除末尾节点外，还需补偿视口滚动偏移（子节点相对滚动）。
            if index + 1 < path.len() {
                if let Some((sx, sy)) = node.viewport_scroll_offset() {
                    transform = transform.concat(Transform::translate(-sx, -sy));
                }
            }
        }
        Some(transform)
    }

    /// 将节点局部矩形变换为屏幕视觉矩形。
    pub(crate) fn node_visual_rect(&self, id: WidgetId, rect: Rect) -> Option<Rect> {
        self.node_visual_transform(id)
            .map(|transform| transform.transform_rect(rect))
    }

    /// 将屏幕坐标逆变换回节点局部坐标。
    pub(crate) fn point_to_node_layout(&self, id: WidgetId, point: Point) -> Option<Point> {
        Some(
            self.node_visual_transform(id)?
                .inverse()?
                .transform_point(point),
        )
    }

    /// 视觉路径上是否存在任意有效视觉变换的节点。
    pub(crate) fn path_has_visual_transform(&self, id: WidgetId) -> bool {
        self.visual_path(id).into_iter().any(|current_id| {
            self.get(current_id)
                .is_some_and(BoxedWidget::has_effective_visual_transform)
        })
    }

    /// 计算子树（不含子悬浮层）的视觉包围盒，用于变换前后的重绘定位。
    pub(crate) fn visual_subtree_bounds(&self, id: WidgetId) -> Option<Rect> {
        let node = self.get(id)?;
        // 不可见节点无包围盒。
        if !node.visible() {
            return None;
        }
        let frame = node.frame();
        // 优先用脏矩形，其次帧矩形，最后退化为命中测试帧。
        let dirty = node.dirty_rect(frame);
        let geometry = if dirty.w > 0.0 && dirty.h > 0.0 {
            dirty
        } else if frame.w > 0.0 && frame.h > 0.0 {
            frame
        } else {
            node.hit_test_frame(frame)
        };
        let mut bounds = (geometry.w > 0.0 && geometry.h > 0.0)
            .then(|| self.clipped_visual_rect(id, geometry))
            .flatten();

        // 递归合并各子节点的包围盒（跳过悬浮层节点）。
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

    /// 把节点局部矩形映射到屏幕，并按视觉祖先的子裁剪区收窄。
    pub(crate) fn clipped_visual_rect(&self, id: WidgetId, base: Rect) -> Option<Rect> {
        if base.w <= 0.0 || base.h <= 0.0 {
            return None;
        }
        let mut path = Vec::new();
        let mut current = Some(id);
        let mut collect_visual_path = true;
        // 一次父链遍历同时完成有效可见性判断与悬浮层视觉路径截断。
        while let Some(current_id) = current {
            let node = self.get(current_id)?;
            if !node.visible() {
                return None;
            }
            if collect_visual_path {
                path.push(current_id);
                if self.is_overlay_node(current_id) {
                    collect_visual_path = false;
                }
            }
            current = node.parent();
        }
        path.reverse();
        let mut transform = Transform::identity();
        let mut clip_bounds: Option<Rect> = None;
        // 沿视觉路径逐级应用变换，并用各层子裁剪区收窄矩形。
        for (index, current_id) in path.iter().copied().enumerate() {
            let current = self.get(current_id)?;
            // 使用与合成器一致的定位与作者变换组合。
            transform = transform.concat(self.positioned_visual_transform(current_id));
            if index + 1 < path.len() {
                // 子裁剪区位于滚动内容平移之前，与合成器顺序保持一致。
                if let Some(clip) = current.children_clip(current.frame()) {
                    let transformed_clip = transform.transform_rect(clip);
                    clip_bounds = Some(match clip_bounds {
                        Some(bounds) => bounds.intersect(&transformed_clip)?,
                        None => transformed_clip,
                    });
                }
                // 后续后代坐标需要补偿当前视口滚动偏移。
                if let Some((sx, sy)) = current.viewport_scroll_offset() {
                    transform = transform.concat(Transform::translate(-sx, -sy));
                }
            }
        }
        let rect = transform.transform_rect(base);
        match clip_bounds {
            Some(clip) => rect.intersect(&clip),
            None => Some(rect),
        }
    }

    /// 计算节点在屏幕上的可见视觉矩形（逐级裁剪，含滚动补偿）。
    pub(crate) fn visible_visual_rect_for(&self, id: WidgetId) -> Option<Rect> {
        let node = self.get(id)?;
        let frame = node.frame();
        // 无帧时退化为命中测试帧。
        let base = if frame.w > 0.0 && frame.h > 0.0 {
            frame
        } else {
            node.hit_test_frame(frame)
        };
        self.clipped_visual_rect(id, base)
    }

    /// 设置节点视觉变换；变化时自增树版本并推动旧/新包围盒重绘。
    pub(crate) fn set_visual_transform(&mut self, id: WidgetId, transform: ViewTransform) -> bool {
        let Some(current) = self.get(id).map(BoxedWidget::visual_transform) else {
            return false;
        };
        // 变换未变化时无需重绘。
        if current == transform {
            return false;
        }
        let old_bounds = self.visual_subtree_bounds(id);
        if let Some(node) = self.get_mut(id) {
            node.set_visual_transform(transform);
        }
        self.tree_version = self.tree_version.wrapping_add(1);
        let new_bounds = self.visual_subtree_bounds(id);
        // 旧位置与新位置都需要重绘，避免残留。
        for rect in [old_bounds, new_bounds].into_iter().flatten() {
            if rect.w > 0.0 && rect.h > 0.0 {
                self.push_paint_invalidation(id, Some(rect));
            }
        }
        true
    }
}

/// 合并两个矩形为包含两者的最小矩形。
fn union_rect(a: Rect, b: Rect) -> Rect {
    let x1 = a.x.min(b.x);
    let y1 = a.y.min(b.y);
    let x2 = (a.x + a.w).max(b.x + b.w);
    let y2 = (a.y + a.h).max(b.y + b.h);
    Rect::new(x1, y1, x2 - x1, y2 - y1)
}

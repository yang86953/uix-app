use super::tree_core::WidgetTree;
use super::*;
use crate::draw::Transform;
use crate::draw::scene::ScenePaint;
// 直接读取节点定位模式以选择普通子节点坐标快路径。
use crate::ui::PositionMode;
use crate::ui::widget_runtime::view_transform::ViewTransform;

impl WidgetTree {
    /// 判断已借用节点是否会截断普通视觉父链。
    fn node_is_visual_root(&self, id: WidgetId, node: &BoxedWidget) -> bool {
        // fixed 无条件提升；普通组件只在类型可能产出浮层时查询当前登记。
        node.position().mode == PositionMode::Fixed
            || (node.may_produce_overlay() && node.overlay_entry(id, node.frame()).is_some())
    }

    /// 判断节点是否为悬浮层节点（overlay 挂载点）。
    fn is_overlay_node(&self, id: WidgetId) -> bool {
        // 单次节点查询同时读取 fixed 与组件浮层能力。
        self.get(id)
            .is_some_and(|node| self.node_is_visual_root(id, node))
    }

    /// 把从树根到指定节点的视觉路径写入调用方工作区（遇悬浮层节点截断）。
    fn fill_visual_path(&self, id: WidgetId, path: &mut Vec<WidgetId>) {
        path.clear();
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
    }

    /// 在树级视觉路径工作区上执行一次同步查询；重入时回退到局部容器。
    fn with_visual_path<R>(&self, id: WidgetId, consume: impl FnOnce(&[WidgetId]) -> R) -> R {
        // 正常窗口事件与布局循环复用树级容量；重入查询不能触发 RefCell panic。
        let mut local_path = Vec::new();
        let mut borrowed_path = self.visual_path_scratch.try_borrow_mut().ok();
        let path = borrowed_path.as_deref_mut().unwrap_or(&mut local_path);
        self.fill_visual_path(id, path);
        let result = consume(path);
        // 清除逻辑内容但保留树级容量供下一次高频查询复用。
        path.clear();
        result
    }

    /// 一次父链遍历同时验证全部祖先可见性并收集浮层截断后的视觉路径。
    fn fill_visible_visual_path(&self, id: WidgetId, path: &mut Vec<WidgetId>) -> bool {
        path.clear();
        let mut current = Some(id);
        let mut collect_visual_path = true;
        while let Some(current_id) = current {
            let Some(node) = self.get(current_id) else {
                return false;
            };
            if !node.visible() {
                return false;
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
        true
    }

    /// 在复用工作区上执行需要完整祖先可见性门禁的视觉查询。
    fn with_visible_visual_path<R>(
        &self,
        id: WidgetId,
        consume: impl FnOnce(&[WidgetId]) -> Option<R>,
    ) -> Option<R> {
        let mut local_path = Vec::new();
        let mut borrowed_path = self.visual_path_scratch.try_borrow_mut().ok();
        let path = borrowed_path.as_deref_mut().unwrap_or(&mut local_path);
        if !self.fill_visible_visual_path(id, path) {
            path.clear();
            return None;
        }
        let result = consume(path);
        path.clear();
        result
    }

    /// 计算节点到屏幕的视觉变换：沿视觉路径逐级串联矩阵并叠加滚动偏移。
    pub(crate) fn node_visual_transform(&self, id: WidgetId) -> Option<Transform> {
        self.with_visual_path(id, |path| {
            if path.is_empty() {
                return None;
            }
            let mut transform = Transform::identity();
            for (index, current_id) in path.iter().copied().enumerate() {
                let node = self.get(current_id)?;
                let current_transform = if index == 0 && self.is_overlay_node(current_id) {
                    // 浮层路径已截断，首节点复用场景声明的根画布最终变换。
                    self.node_overlay_transform(current_id)
                } else {
                    // 合成 relative/sticky 定位偏移与作者视觉变换。
                    self.positioned_visual_transform(current_id)
                };
                transform = transform.concat(current_transform);
                // 除末尾节点外，还需补偿视口滚动偏移（子节点相对滚动）。
                if index + 1 < path.len()
                    && let Some((sx, sy)) = node.viewport_scroll_offset()
                {
                    transform = transform.concat(Transform::translate(-sx, -sy));
                }
            }
            Some(transform)
        })
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

    /// 从父节点已解析的内容坐标增量求出直接子节点布局坐标。
    pub(super) fn point_to_child_layout(
        &self,
        id: WidgetId,
        node: &BoxedWidget,
        parent_content_point: Point,
        screen_point: Point,
    ) -> Option<Point> {
        // fixed 与组件浮层会截断视觉父链，必须从原始屏幕坐标重新开始。
        let (transform, point) = if self.node_is_visual_root(id, node) {
            (self.node_overlay_transform(id), screen_point)
        } else {
            // 普通后代只追加自身变换；父变换与滚动已经反映在内容坐标中。
            let transform = match node.position().mode {
                // static/absolute 不需要再次查询树级定位偏移。
                PositionMode::Static | PositionMode::Absolute => self
                    .parent_children_transform(id)
                    .concat(node.visual_transform_matrix()),
                // relative/sticky 仍由树级定位算法解析当前动态偏移。
                PositionMode::Relative | PositionMode::Sticky => {
                    self.positioned_visual_transform(id)
                }
                // fixed 已在视觉根分支处理；失配时保守拒绝命中。
                PositionMode::Fixed => return None,
            };
            (transform, parent_content_point)
        };
        // 常见单位变换无需执行通用矩阵求逆和点乘。
        if transform.is_identity() {
            return Some(point);
        }
        Some(transform.inverse()?.transform_point(point))
    }

    /// 视觉路径上是否存在任意有效视觉变换的节点。
    pub(crate) fn path_has_visual_transform(&self, id: WidgetId) -> bool {
        self.with_visual_path(id, |path| {
            path.iter().copied().any(|current_id| {
                self.get(current_id)
                    .is_some_and(BoxedWidget::has_effective_visual_transform)
                    || !self.parent_children_transform(current_id).is_identity()
            })
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
        self.with_visible_visual_path(id, |path| {
            let mut transform = Transform::identity();
            let mut clip_bounds: Option<Rect> = None;
            // 沿视觉路径逐级应用变换，并用各层子裁剪区收窄矩形。
            for (index, current_id) in path.iter().copied().enumerate() {
                let current = self.get(current_id)?;
                let current_transform = if index == 0 && self.is_overlay_node(current_id) {
                    // 浮层不继承祖先裁剪，但仍使用与合成器一致的锚点变换。
                    self.node_overlay_transform(current_id)
                } else {
                    self.positioned_visual_transform(current_id)
                };
                transform = transform.concat(current_transform);
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
        })
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

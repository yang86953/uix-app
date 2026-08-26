use super::super::super::*;
use super::super::WidgetTree;
use crate::core::Rect;

use super::{ShrinkOp, frame_constraints};

impl WidgetTree {
    /// Phase 2 重排：对已扩展子节点，measure 结果不得低于当前 frame。
    pub(crate) fn layout_children_preserving_expansions(
        &self,
        id: WidgetId,
        frame: Rect,
        children: &[WidgetId],
        expanded: &std::collections::HashSet<WidgetId>,
        scratch: &mut super::LayoutArrangeScratch,
    ) {
        let Some(node) = self.get(id) else {
            scratch.positions.clear();
            return;
        };
        let Some(layout) = node.widget().as_layout() else {
            scratch.positions.clear();
            return;
        };
        layout.measure_children_into(frame, children, self, &mut scratch.measured);
        for child in &mut scratch.measured {
            if !expanded.contains(&child.id) {
                continue;
            }
            if let Some(cf) = self.get(child.id).map(|n| n.frame()) {
                child.measured_size.w = child.measured_size.w.max(cf.w);
                child.measured_size.h = child.measured_size.h.max(cf.h);
            }
        }
        self.arrange_positioned_children_into(
            layout,
            frame,
            &mut scratch.measured,
            &mut scratch.in_flow,
            &mut scratch.out_of_flow,
            &mut scratch.engine,
            &mut scratch.positions,
        );
    }

    /// 更新所有 viewport 容器的 content_bounds。
    /// 只触发 content_bounds 副作用，不移动子节点位置。
    pub(crate) fn layout_viewports(
        &mut self,
        order: &[WidgetId],
        scratch: &mut super::LayoutArrangeScratch,
    ) {
        for &id in order {
            if !self.is_effectively_visible(id) {
                continue;
            }
            if let Some(node) = self.get(id) {
                if node.children_clip(node.frame()).is_none() {
                    continue;
                }
                let frame = node.frame();
                let children = node.children();
                if children.is_empty() {
                    continue;
                }
                tracing::debug!(
                    "[Layout] Phase 3: viewport id={} frame=({:.0},{:.0},{:.0},{:.0}) {} children",
                    id,
                    frame.x,
                    frame.y,
                    frame.w,
                    frame.h,
                    children.len(),
                );
                // 仅触发 content_bounds 副作用，丢弃返回的 child rects
                node.layout_children_into(frame, children, self, scratch);
            }
        }
    }

    /// Rebuild VirtualScroll child windows when layout frame or scroll offset changes.
    pub(crate) fn refresh_virtual_scroll_children(&mut self, ids: &mut Vec<WidgetId>) {
        ids.clear();
        ids.extend(self.traverse().iter().copied());
        for id in ids.iter().copied() {
            let viewport_h = self.get(id).map(|node| node.frame().h).unwrap_or(0.0);
            if viewport_h <= 0.0 {
                continue;
            }
            self.refresh_virtual_scroll_widget(id, Some(viewport_h));
        }
    }

    // 表格 capability 启用时才扫描并刷新泛型单元格子树。
    #[cfg(feature = "table")]
    pub(crate) fn refresh_table_cell_children(&mut self, ids: &mut Vec<WidgetId>) {
        ids.clear();
        ids.extend(self.traverse().iter().copied());
        for id in ids.iter().copied() {
            if self.refresh_table_cell_widget(id) {
                self.push_layout_invalidation(id);
                self.propagate_layout_invalidation(id);
            }
        }
    }

    pub(crate) fn refresh_image_error_children(&mut self, ids: &mut Vec<WidgetId>) {
        ids.clear();
        ids.extend(self.traverse().iter().copied());
        for id in ids.iter().copied() {
            if self.refresh_image_error_widget(id) {
                self.push_layout_invalidation(id);
                self.propagate_layout_invalidation(id);
            }
        }
    }

    pub(crate) fn refresh_collapse_content_children(&mut self, ids: &mut Vec<WidgetId>) {
        ids.clear();
        ids.extend(self.traverse().iter().copied());
        for id in ids.iter().copied() {
            if self.refresh_collapse_content_widget(id) {
                self.push_layout_invalidation(id);
                self.propagate_layout_invalidation(id);
            }
        }
    }

    /// 父级当前会分配给 `id` 的 frame（Phase 1 槽位）。
    /// Phase 4 不得收缩到该高度以下，否则 Stretch/flex 分配会被下一轮 Phase 1 拉回，形成 thrashing。
    pub(crate) fn parent_allocated_frame(
        &self,
        id: WidgetId,
        scratch: &mut super::LayoutArrangeScratch,
    ) -> Option<Rect> {
        let parent_id = self.get(id).and_then(|n| n.parent())?;
        let parent_frame = self.get(parent_id)?.frame();
        let children = self.get(parent_id)?.children();
        if children.is_empty() {
            return None;
        }
        self.get(parent_id)?
            .layout_children_into(parent_frame, children, self, scratch);
        scratch
            .positions
            .iter()
            .copied()
            .find(|(cid, _)| *cid == id)
            .map(|(_, rect)| rect)
    }

    /// 记录当前节点正常流子树的最下边界，并补偿父级重排产生的整体位移。
    fn record_normal_flow_subtree_bottom(
        &self,
        id: WidgetId,
        effective_visible: &std::collections::HashSet<WidgetId>,
        bottoms: &mut std::collections::HashMap<WidgetId, (f32, f32)>,
    ) -> Option<f32> {
        let node = self.get(id)?;
        let frame = node.frame();
        if node.children_clip(frame).is_some() {
            bottoms.remove(&id);
            return None;
        }
        let mut bottom: Option<f32> = None;
        for &child_id in node.children() {
            let Some(child) = self.get(child_id) else {
                continue;
            };
            // 绝对定位与浮层不参与祖先的滚动内容自然高度。
            if !effective_visible.contains(&child_id) || child.position().mode.is_out_of_flow() {
                continue;
            }
            let child_frame = child.frame();
            let mut child_bottom = child_frame.y + child_frame.h;
            if let Some((recorded_y, descendant_bottom)) = bottoms.get(&child_id).copied() {
                // 父级本轮可能整体移动了子节点；后代范围随相同位移平移。
                child_bottom = child_bottom.max(descendant_bottom + child_frame.y - recorded_y);
            }
            bottom = Some(bottom.map_or(child_bottom, |value| value.max(child_bottom)));
        }
        if let Some(bottom) = bottom {
            bottoms.insert(id, (frame.y, bottom));
        } else {
            bottoms.remove(&id);
        }
        bottom
    }

    /// 收缩过大的容器。与 layout_expand 相反——当子节点高度
    /// 显著小于容器当前高度，且子节点延伸到可见区域时，收缩容器。
    /// 每轮先重新布局子节点（确保兄弟组件靠拢），再检查是否需要收缩。
    /// 返回是否有任何容器被收缩。
    pub(crate) fn layout_shrink(
        &mut self,
        order: &[WidgetId],
        ops: &mut Vec<ShrinkOp>,
        children: &mut Vec<WidgetId>,
        parent_children: &mut Vec<WidgetId>,
        arrange: &mut super::LayoutArrangeScratch,
        subtree_bottoms: &mut std::collections::HashMap<WidgetId, (f32, f32)>,
        layout_damage: &mut super::LayoutFrameDamage,
        effective_visible: &std::collections::HashSet<WidgetId>,
    ) -> bool {
        let mut any_changed = false;
        for _pass in 0..3 {
            let mut pass_changed = false;
            // 反向遍历中每条父子边只汇总一次，避免每个祖先重复扫描全部后代。
            subtree_bottoms.clear();
            // Phase A: 收集需要收缩的容器
            ops.clear();

            for &id in order.iter().rev() {
                if !effective_visible.contains(&id) {
                    continue;
                }
                // 根 frame 由窗口客户区锁定，shrink 同样不得改写。
                if self.root_id == Some(id) {
                    continue;
                }
                let is_viewport = self
                    .get(id)
                    .map(|n| n.children_clip(n.frame()).is_some())
                    .unwrap_or(false);
                if is_viewport {
                    continue;
                }
                // viewport 内的自然内容同样必须允许回缩；条件成员变化已清除旧缓存，
                // 下方再以当前可见子树的真实末端约束收缩结果。

                children.clear();
                match self.get(id) {
                    Some(n) if !n.children().is_empty() => {
                        children.extend_from_slice(n.children());
                    }
                    _ => continue,
                }

                // 先按当前 frame 重新布局子节点（兄弟组件靠拢/张开）
                let Some(frame) = self.get(id).map(|n| n.frame()) else {
                    continue;
                };
                {
                    let Some(node) = self.get(id) else {
                        continue;
                    };
                    node.layout_children_into(frame, children, self, arrange);
                }
                for position_index in 0..arrange.positions.len() {
                    let (child_id, rect) = arrange.positions[position_index];
                    if self.set_layout_frame(child_id, rect, layout_damage) {
                        pass_changed = true;
                    }
                }

                // 检查容器是否需要收缩
                let Some(node_frame) = self.get(id).map(|n| n.frame()) else {
                    continue;
                };
                let mut max_child_bottom = f32::MIN;
                let mut has_visible = false;
                for &cid in children.iter() {
                    if let Some(child) = self.get(cid) {
                        if child.visible() && !child.position().mode.is_out_of_flow() {
                            let cf = child.frame();
                            let child_bottom = cf.y + cf.h;
                            if child_bottom > 0.0 {
                                max_child_bottom = max_child_bottom.max(child_bottom);
                                has_visible = true;
                            }
                        }
                    }
                }
                if !has_visible {
                    continue;
                }

                let scrolls_vertically = self
                    .nearest_viewport_overflow_axes(id)
                    .is_some_and(|(_, vertical)| vertical);
                let subtree_bottom =
                    self.record_normal_flow_subtree_bottom(id, effective_visible, subtree_bottoms);
                let (_, explicit_height) = self.phase2_explicit_size_locks(id);
                if scrolls_vertically && explicit_height {
                    continue;
                }
                let content_bottom = if scrolls_vertically {
                    subtree_bottom.unwrap_or(max_child_bottom)
                } else {
                    max_child_bottom
                };
                let needed_h = content_bottom - node_frame.y;
                // ⭐ 最小高度取子节点实际内容和 measure 的较大值。
                // 设此下限可防止收缩到子节点内容以下，从而避免与
                // layout_expand（Phase 2）形成振荡循环。
                // 使用 1.0 像素绝对最小值而非比例值（如 0.01 * h），
                // 后者在高 DPI 场景下可能过大（2000px * 0.01 = 20px 虚高）。
                let pref_h = if scrolls_vertically {
                    // 滚动子树的缓存刚失效时仍可能在收敛中携带旧槽位；可见子树
                    // 的完整末端才是当前页面的尺寸事实。显式高度已在上方保留。
                    0.0
                } else {
                    let measure_constraints = frame_constraints(node_frame);
                    self.get(id)
                        .map(|n| n.measure(measure_constraints).h)
                        .unwrap_or(0.0)
                };
                let min_h = needed_h.max(pref_h).max(1.0);
                // 不得低于父级 Phase 1 分配高度（Stretch / flex-grow 槽位）。
                // demo 侧栏 column_fit 被 row Stretch 拉到客户区高后，若按内容缩回，
                // 下一轮 Phase 1 会再次拉满 → 同结果 Phase 4 空转 thrashing。
                let parent_floor_h = if scrolls_vertically {
                    0.0
                } else {
                    self.parent_allocated_frame(id, arrange)
                        .map(|r| r.h)
                        .unwrap_or(0.0)
                };
                let effective_needed = min_h.max(parent_floor_h);
                if node_frame.h - effective_needed > 0.5 {
                    tracing::debug!(
                        "[Layout] Phase 4: id={} shrink {:.0}px {:.0}→{:.0} (needed={:.0} pref={:.0} floor={:.0})",
                        id,
                        node_frame.h - effective_needed,
                        node_frame.h,
                        effective_needed,
                        needed_h,
                        pref_h,
                        parent_floor_h,
                    );
                    ops.push(ShrinkOp {
                        id,
                        needed_h: effective_needed,
                    });
                }
            }
            // Phase B: 执行收缩
            for op in ops.iter() {
                if let Some(old_frame) = self.get(op.id).map(|n| n.frame()) {
                    if !self.set_layout_frame(
                        op.id,
                        Rect::new(old_frame.x, old_frame.y, old_frame.w, op.needed_h),
                        layout_damage,
                    ) {
                        continue;
                    }
                    #[cfg(test)]
                    {
                        self.layout_shrink_ops
                            .set(self.layout_shrink_ops.get().wrapping_add(1));
                    }
                    children.clear();
                    if let Some(node) = self.get(op.id) {
                        children.extend_from_slice(node.children());
                    }
                    let new_frame = Rect::new(old_frame.x, old_frame.y, old_frame.w, op.needed_h);
                    // 收缩后重新布局子节点
                    if let Some(node) = self.get(op.id) {
                        node.layout_children_into(new_frame, children, self, arrange);
                    } else {
                        arrange.positions.clear();
                    }
                    for position_index in 0..arrange.positions.len() {
                        let (child_id, rect) = arrange.positions[position_index];
                        let _ = self.set_layout_frame(child_id, rect, layout_damage);
                    }
                    // 重新布局父容器，让兄弟组件靠拢
                    if let Some(pid) = self.get(op.id).and_then(|n| n.parent()) {
                        let parent_frame = self.get(pid).map(|n| n.frame()).unwrap_or_default();
                        parent_children.clear();
                        if let Some(parent) = self.get(pid) {
                            parent_children.extend_from_slice(parent.children());
                        }
                        if !parent_children.is_empty() {
                            if let Some(parent) = self.get(pid) {
                                parent.layout_children_into(
                                    parent_frame,
                                    parent_children,
                                    self,
                                    arrange,
                                );
                            } else {
                                arrange.positions.clear();
                            }
                            for position_index in 0..arrange.positions.len() {
                                let (child_id, rect) = arrange.positions[position_index];
                                let _ = self.set_layout_frame(child_id, rect, layout_damage);
                            }
                        }
                    }
                    pass_changed = true;
                }
            }
            if !pass_changed {
                break;
            }
            any_changed = true;
        }
        any_changed
    }
}

use super::*;

impl WidgetTree {
    /// 2D 命中测试：根据屏幕坐标找到最深的 widget。
    pub fn hit_test(&self, pos: Point) -> Option<ComponentId> {
        // 已停止的树不得用半提交的节点关系计算命中目标。
        if !self.accepts_external_work() {
            // 对事件入口报告没有可交互目标。
            return None;
        }
        self.root_id
            .and_then(|root| self.hit_test_internal(root, pos))
    }

    /// 3D 命中测试：根据 3D 射线找到最深的 widget。
    ///
    /// `spatial` 为当前空间上下文（用于逆变换）。
    /// 使用 Widget::hit_test_3d 方法，支持 3D 变换后的 widget。
    pub fn hit_test_3d(
        &self,
        ray: &crate::draw::geometry::spatial::Ray3D,
        spatial: &crate::draw::geometry::spatial::SpatialContext,
    ) -> Option<ComponentId> {
        // 已停止的树不得用半提交的节点关系计算三维命中目标。
        if !self.accepts_external_work() {
            // 对事件入口报告没有可交互目标。
            return None;
        }
        self.root_id
            .and_then(|root| self.hit_test_3d_internal(root, ray, spatial))
    }

    /// 3D 命中测试内部递归。
    fn hit_test_3d_internal(
        &self,
        id: WidgetId,
        ray: &crate::draw::geometry::spatial::Ray3D,
        spatial: &crate::draw::geometry::spatial::SpatialContext,
    ) -> Option<WidgetId> {
        let node = self.get(id)?;
        if !node.visible() || self.is_pending_removal_subtree(id) {
            return None;
        }
        let mut sorted: Vec<WidgetId> = node.children().to_vec();
        sorted.sort_by(|&a, &b| {
            let za = self.get(a).map_or(0, |c| c.z_index());
            let zb = self.get(b).map_or(0, |c| c.z_index());
            zb.cmp(&za)
        });
        for &child_id in &sorted {
            if let Some(hit) = self.hit_test_3d_internal(child_id, ray, spatial) {
                return Some(hit);
            }
        }
        let frame = node.frame();
        if node.hit_test_3d(ray, spatial, frame) {
            Some(id)
        } else {
            None
        }
    }

    pub(super) fn hit_test_internal(&self, id: WidgetId, pos: Point) -> Option<WidgetId> {
        let node = self.get(id)?;
        if !node.visible() || self.is_pending_removal_subtree(id) {
            return None;
        }

        // Undo the same transform/scroll chain used by compositor painting.
        let layout_pos = self.point_to_node_layout(id, pos)?;

        let can_hit_children = node.hit_test_children()
            && node
                .children_clip(node.frame())
                .is_none_or(|clip| clip.contains(layout_pos));
        if can_hit_children {
            // 把父节点视口坐标转换为子内容坐标，供片段裁剪命中复用。
            let child_clip_pos = node
                // 读取父节点施加在全部子项上的滚动偏移。
                .viewport_scroll_offset()
                // 滚动内容坐标等于视口坐标加当前偏移。
                .map(|(scroll_x, scroll_y)| {
                    // 构造与子节点布局 frame 相同坐标系中的命中点。
                    Point::new(layout_pos.x + scroll_x, layout_pos.y + scroll_y)
                })
                // 非滚动父节点直接沿用当前布局坐标。
                .unwrap_or(layout_pos);
            let mut sorted: Vec<WidgetId> = node.children().to_vec();
            sorted.sort_by(|&a, &b| {
                let za = self.get(a).map_or(0, |c| c.z_index());
                let zb = self.get(b).map_or(0, |c| c.z_index());
                zb.cmp(&za)
            });
            // 同 z-index 时，后出现的兄弟绘制在上层，hit-test 应优先命中
            let mut start = 0;
            while start < sorted.len() {
                let z = self.get(sorted[start]).map_or(0, |c| c.z_index());
                let mut end = start + 1;
                while end < sorted.len() && self.get(sorted[end]).map_or(0, |c| c.z_index()) == z {
                    end += 1;
                }
                sorted[start..end].reverse();
                start = end;
            }
            for &child_id in &sorted {
                // 先按父布局为该子树声明的片段集合过滤命中。
                let inside_parent_regions = self
                    // 读取当前子节点保存的父级片段元数据。
                    .get(child_id)
                    // 缺少片段表示普通未裁剪子树，存在片段则要求命中任一矩形。
                    .and_then(|child| child.parent_clip_regions())
                    // 空片段集合自然拒绝全部命中。
                    .is_none_or(|regions| {
                        // 不连续片段使用集合命中，不能退化为联合包围盒。
                        regions.iter().any(|region| region.contains(child_clip_pos))
                    });
                // 指针落在片段间隙时跳过整个子树。
                if !inside_parent_regions {
                    // 继续检查下一层视觉兄弟节点。
                    continue;
                }
                if let Some(hit) = self.hit_test_internal(child_id, pos) {
                    return Some(hit);
                }
            }
        }
        // 使用 widget 的 hit_test_frame 代替原始 frame，支持 overlay 模式
        let actual_frame = node.frame();
        let hit_frame = node.hit_test_frame(actual_frame);
        if hit_frame.contains(layout_pos) {
            Some(id)
        } else {
            None
        }
    }

    // 验证屏幕点是否位于目标节点及其祖先链的父级片段裁剪中。
    pub(super) fn point_inside_parent_clip_regions(
        // 接收需要验证的目标节点。
        &self,
        // 接收目标节点标识。
        target: WidgetId,
        // 接收屏幕坐标中的指针位置。
        pos: Point,
    ) -> bool {
        // 从目标开始逐级检查每条父子边上的可选片段集合。
        let mut current = Some(target);
        // 祖先链有限，直到根节点结束。
        while let Some(id) = current {
            // 节点已移除时不能继续视为命中。
            let Some(node) = self.get(id) else {
                // 返回失败避免事件落到失效节点。
                return false;
            };
            // 保存父节点标识供本轮片段换算与下一轮遍历。
            let parent = node.parent();
            // 只在父布局声明片段时执行集合命中。
            if let Some(regions) = node.parent_clip_regions() {
                // 根节点没有父坐标系，不能合法携带父级片段。
                let Some(parent_id) = parent else {
                    // 拒绝结构不完整的片段元数据。
                    return false;
                };
                // 读取父节点以换算其滚动内容坐标。
                let Some(parent_node) = self.get(parent_id) else {
                    // 父节点丢失时拒绝命中。
                    return false;
                };
                // 将屏幕点转换到父节点布局坐标。
                let Some(mut parent_pos) = self.point_to_node_layout(parent_id, pos) else {
                    // 不可逆变换下不能安全命中片段。
                    return false;
                };
                // 父节点滚动时，片段与子 frame 位于内容坐标。
                if let Some((scroll_x, scroll_y)) = parent_node.viewport_scroll_offset() {
                    // 加回水平滚动偏移。
                    parent_pos.x += scroll_x;
                    // 加回垂直滚动偏移。
                    parent_pos.y += scroll_y;
                }
                // 任一祖先片段集合未包含指针时立即拒绝。
                if !regions.iter().any(|region| region.contains(parent_pos)) {
                    // 不允许完整 frame 包围盒绕过不连续片段。
                    return false;
                }
            }
            // 上移到父节点继续检查更外层片段。
            current = parent;
        }
        // 全部父级片段约束都通过后接受命中。
        true
    }

    pub fn dispatch_event(&mut self, event: &SystemEvent) -> EventResult {
        // 已停止的树不得继续分发可能触发组件回调的系统事件。
        if !self.accepts_external_work() {
            // 明确拒绝事件，避免外层循环把半树视为可交互。
            return EventResult::NotHandled;
        }
        self.begin_invalidation_batch();
        let result = self.dispatch_event_inner(event);
        self.cancel_hidden_interaction();
        self.finish_invalidation_batch();
        result
    }

    pub(super) fn dispatch_event_inner(&mut self, event: &SystemEvent) -> EventResult {
        if self
            .managers()
            .focus
            .focused_component()
            .is_some_and(|focused| !self.focus_target_available(focused))
        {
            self.set_focus(None);
        }
        if self.ignores_input_while_window_unfocused(event) {
            return EventResult::NotHandled;
        }

        match event {
            SystemEvent::PointerDown { pos, button, mods }
            | SystemEvent::PointerDoubleClick { pos, button, mods } => {
                self.dispatch_pointer_press(event, *pos, *button, *mods)
            }
            SystemEvent::PointerUp { pos, button, mods } => {
                self.dispatch_pointer_release(event, *pos, *button, *mods)
            }
            SystemEvent::PointerMove { pos, mods } => {
                // 拖拽手势检测：potential → active 转换
                if self.managers().drag.is_potential() && !self.managers().drag.is_dragging() {
                    let start_pos = self.managers().drag.start_pos();
                    let dx = pos.x - start_pos.x;
                    let dy = pos.y - start_pos.y;
                    // 5px 阈值：超出才视为拖拽开始
                    if dx.abs() > 5.0 || dy.abs() > 5.0 {
                        self.managers_mut().drag.activate_gesture();
                        // 发射 DragStart 到拖拽目标
                        if let Some(target) = self.managers().drag.target() {
                            let drag_start = SystemEvent::DragStart {
                                pos: *pos,
                                button: self.managers().drag.button(),
                                mods: *mods,
                            };
                            let _ = self.dispatch_to(target, &drag_start);
                        }
                    }
                }
                // 拖拽进行中：发射 DragMove
                if self.managers().drag.is_dragging() {
                    if let Some(target) = self.managers().drag.target() {
                        let last_pos = self.managers().drag.last_pos();
                        let delta = Point::new(pos.x - last_pos.x, pos.y - last_pos.y);
                        let drag_move = SystemEvent::DragMove {
                            pos: *pos,
                            delta,
                            mods: *mods,
                        };
                        let _ = self.dispatch_to(target, &drag_move);
                    }
                }
                if self.managers().drag.is_dragging() || self.managers().drag.is_potential() {
                    self.managers_mut().drag.update_drag(*pos);
                }

                if let Some(drag_target) = self.managers().interaction.pressed_component() {
                    // 文字拖选：pressed 捕获会把 Move 锁在起点节点，须在树层
                    // 协调同父级兄弟行，才能向上/向下扩展选区。
                    if self
                        .get(drag_target)
                        .is_some_and(crate::ui::text_selection::is_dragging)
                        && self.apply_cross_text_selection_drag(drag_target, *pos)
                    {
                        self.rebuild_widget_overlays();
                        return EventResult::Handled;
                    }
                    let result = self.dispatch_to(drag_target, event);
                    self.rebuild_widget_overlays();
                    return result;
                }

                let current_hover = self.managers().interaction.hovered_component();

                if let Some(hovered) = current_hover.filter(|hovered| {
                    self.get(*hovered)
                        .is_some_and(|node| node.is_interaction_enabled())
                }) {
                    if self.pointer_inside_widget_hit_frame(hovered, *pos) {
                        let result = if self
                            .get(hovered)
                            .is_some_and(|node| node.wants_continuous_pointer_move())
                        {
                            self.dispatch_to(hovered, event)
                        } else {
                            EventResult::NotHandled
                        };
                        self.rebuild_widget_overlays();
                        return result;
                    }
                }

                let old_hover = current_hover;
                let new_hover = self
                    .overlay_target_at(*pos)
                    .or_else(|| self.hit_test(*pos))
                    .filter(|target| {
                        self.get(*target)
                            .is_some_and(|node| node.is_interaction_enabled())
                    });
                if new_hover != current_hover {
                    // 仅当组件实际处理了 enter/leave（有 hover 视觉态）才窄标脏。
                    // Label/Icon 等叶子 NotHandled 时若仍 invalidate，局部清屏会挖掉父背景。
                    if let Some(old) = current_hover {
                        if self.dispatch_to(old, &SystemEvent::PointerLeave) == EventResult::Handled
                        {
                            self.invalidate_paint(old);
                        }
                    }
                    if let Some(new) = new_hover {
                        if self.dispatch_to(new, &SystemEvent::PointerEnter) == EventResult::Handled
                        {
                            self.invalidate_paint(new);
                        }
                    }
                    self.managers_mut()
                        .interaction
                        .set_hovered_component(new_hover);
                }
                let result = if let Some(t) = new_hover {
                    if new_hover != old_hover
                        || self
                            .get(t)
                            .is_some_and(|node| node.wants_continuous_pointer_move())
                    {
                        self.dispatch_to(t, event)
                    } else {
                        EventResult::NotHandled
                    }
                } else {
                    EventResult::NotHandled
                };
                self.rebuild_widget_overlays();
                result
            }
            SystemEvent::Wheel { pos, .. } => {
                let target = self
                    .overlay_target_at(*pos)
                    .or_else(|| self.hit_test(*pos))
                    .or(self.managers().interaction.hovered_component())
                    .or(self.root_id);
                if let Some(t) = target {
                    // 捕获阶段：ScrollView 等祖先先处理；Handled 时由 capture 侧登记动画与视口重绘，
                    // 避免仅 mark_dirty 子节点导致 strip 局部清除后内容消失。
                    if self.capture_wheel_to(t, event) == EventResult::Handled {
                        return EventResult::Handled;
                    }
                    self.dispatch_wheel_to(t, event)
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::KeyDown { key, mods } => self.dispatch_key_down(event, *key, *mods),
            SystemEvent::KeyUp { key, .. } => self.dispatch_key_up(event, *key),
            SystemEvent::TextInput { text } => {
                if let Some(t) = self.managers().focus.focused_component() {
                    self.invalidate_paint(t);
                    let result = self.dispatch_to(t, event);
                    if result == EventResult::Handled {
                        let _ = self.dispatch_semantic(SemanticEvent::text_input(t, text.clone()));
                    }
                    result
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::ImeCompositionStart
            | SystemEvent::ImeCompositionUpdate { .. }
            | SystemEvent::ImeCompositionEnd { .. } => {
                if let Some(t) = self.managers().focus.focused_component() {
                    self.invalidate_paint(t);
                    let result = self.dispatch_to(t, event);
                    if result == EventResult::Handled {
                        let semantic = match event {
                            SystemEvent::ImeCompositionStart => {
                                SemanticEvent::ime_composition_start(t)
                            }
                            SystemEvent::ImeCompositionUpdate { text } => {
                                SemanticEvent::ime_composition_update(t, text.clone())
                            }
                            SystemEvent::ImeCompositionEnd { text } => {
                                SemanticEvent::ime_composition_end(t, text.clone())
                            }
                            _ => unreachable!(),
                        };
                        let _ = self.dispatch_semantic(semantic);
                    }
                    result
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::Copy | SystemEvent::Cut | SystemEvent::Paste { .. } => {
                if let Some(t) = self.managers().focus.focused_component() {
                    self.invalidate_paint(t);
                    if matches!(event, SystemEvent::Copy) && self.try_copy_cross_text_selection(t) {
                        let _ = self.dispatch_semantic(SemanticEvent::copy(t));
                        return EventResult::Handled;
                    }
                    let result = self.dispatch_to(t, event);
                    if result == EventResult::Handled {
                        let semantic = match event {
                            SystemEvent::Copy => SemanticEvent::copy(t),
                            SystemEvent::Cut => SemanticEvent::cut(t),
                            SystemEvent::Paste { text } => SemanticEvent::paste(t, text.clone()),
                            _ => unreachable!(),
                        };
                        let _ = self.dispatch_semantic(semantic);
                    }
                    result
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn | SystemEvent::FocusOut => {
                if let Some(t) = self.managers().focus.focused_component() {
                    self.dispatch_to(t, event)
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerEnter | SystemEvent::PointerLeave => EventResult::NotHandled,
            SystemEvent::ThemeChanged { .. } => {
                self.notify_theme_changed();
                if let Some(root) = self.root_id {
                    self.dispatch_to(root, event)
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::LocaleChanged { .. } => {
                if let Some(root) = self.root_id {
                    self.push_layout_invalidation(root);
                    self.invalidate_paint(root);
                    self.dispatch_to(root, event)
                } else {
                    EventResult::NotHandled
                }
            }
            // 窗口状态变化事件 → 统一分发给 root，让应用层处理
            SystemEvent::WindowMaximize
            | SystemEvent::WindowMinimize
            | SystemEvent::WindowRestore
            | SystemEvent::WindowFocus
            | SystemEvent::WindowBlur => self.dispatch_window_lifecycle(event),
            SystemEvent::Timer { .. } => {
                let target = self
                    .managers()
                    .interaction
                    .hovered_component()
                    .or(self.managers().focus.focused_component())
                    .or(self.root_id);
                if let Some(target) = target {
                    let result = self.dispatch_to(target, event);
                    self.rebuild_widget_overlays();
                    result
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FileDrop { files, position } => {
                // 文件拖放优先分发给 overlay 命中节点，否则交给主树/root。
                if let Some(target) = self
                    .overlay_target_at(*position)
                    .or_else(|| self.hit_test(*position))
                    .or(self.root_id)
                {
                    let result = self.dispatch_to(target, event);
                    let _ = self.dispatch_semantic(SemanticEvent::file_drop(
                        target,
                        files.clone(),
                        *position,
                    ));
                    result
                } else {
                    EventResult::NotHandled
                }
            }
            // 拖拽组合事件：由 dispatch_event 内部合成并直接 dispatch_to，
            // 不会从外部传入 dispatch_event。
            SystemEvent::DragStart { .. }
            | SystemEvent::DragMove { .. }
            | SystemEvent::DragEnd { .. } => EventResult::NotHandled,
            SystemEvent::Resize { width, height } => {
                if let Some(root) = self.root_id {
                    if *width > 0.0 && *height > 0.0 {
                        let resized_root = if let Some(root_mut) = self.get_mut(root) {
                            root_mut.set_frame(Rect::new(0.0, 0.0, *width, *height));
                            true
                        } else {
                            false
                        };
                        if resized_root {
                            self.push_layout_invalidation(root);
                            self.invalidate_paint(root);
                            // 递增 tree_version 使 LayerTree 重建
                            // LayerTree 缓存了 ClipRect（如 ScrollView 的裁剪矩形），
                            // 不重建则子节点位置更新了但裁剪区还是旧尺寸 → 内容被裁剪。
                            self.tree_version += 1;
                        }
                    }
                    self.dispatch_to(root, event)
                } else {
                    EventResult::NotHandled
                }
            }
        }
    }
}

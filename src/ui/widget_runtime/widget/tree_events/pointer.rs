use super::*;

impl WidgetTree {
    pub(super) fn dispatch_pointer_press(
        &mut self,
        event: &SystemEvent,
        pos: Point,
        button: MouseButton,
        mods: KeyMod,
    ) -> EventResult {
        if button != MouseButton::Right {
            self.set_keyboard_focus_visible(false);
        }
        if let Some(result) = self.intercept_top_overlay_outside_pointer_down(pos) {
            return result;
        }
        let target = self.pointer_target_at(pos);
        if target.is_some_and(|target| {
            self.get(target)
                .is_none_or(|node| !node.is_interaction_enabled())
        }) {
            if button != MouseButton::Right {
                self.set_focus(None);
            }
            self.rebuild_widget_overlays();
            return EventResult::NotHandled;
        }
        let begins_pointer = self
            .managers_mut()
            .interaction
            .begin_pressed_pointer(target, button);
        if begins_pointer {
            self.managers_mut()
                .drag
                .begin_gesture(target, pos, button, mods);
        }
        let Some(target) = target else {
            if button != MouseButton::Right {
                self.set_focus(None);
            }
            self.rebuild_widget_overlays();
            return EventResult::NotHandled;
        };

        let actions_before_dispatch = self.pending_window_actions.len();
        self.invalidate_paint(target);
        let pointer_down;
        let capture_event = if matches!(event, SystemEvent::PointerDoubleClick { .. }) {
            pointer_down = SystemEvent::PointerDown { pos, button, mods };
            &pointer_down
        } else {
            event
        };
        if let Some(capture_target) = self.capture_to(target, capture_event) {
            if begins_pointer {
                let _ = self
                    .managers_mut()
                    .interaction
                    .release_pressed_pointer(button);
                if self.managers().drag.is_gesture_button(button) {
                    self.managers_mut().drag.end_drag();
                }
            }
            if button != MouseButton::Right
                && self
                    .get(capture_target)
                    .is_some_and(|node| node.is_focusable())
            {
                self.set_focus(Some(capture_target));
            }
            self.rebuild_widget_overlays();
            return EventResult::Handled;
        }
        let result = if matches!(event, SystemEvent::PointerDoubleClick { .. }) {
            self.dispatch_double_click_to(target, event)
        } else {
            self.dispatch_to(target, event)
        };
        if result == EventResult::Handled {
            // all 在普通文字按下后扩展为最近声明子树的完整选择。
            let selected_all = self.select_all_user_select_subtree(target);
            // 普通 auto/text 拖选继续清理同级旧范围；all 已整体重建范围。
            if !selected_all
                && self
                    .get(target)
                    .is_some_and(crate::ui::text_selection::participates)
            {
                self.clear_sibling_cross_text_selections(target);
            }
            self.invalidate_nav_siblings(target);
            let preserves_keyboard_focus = button == MouseButton::Right
                || self.pending_window_actions[actions_before_dispatch..]
                    .iter()
                    .any(|action| action.preserves_keyboard_focus());
            if !preserves_keyboard_focus {
                // 组合 owner 可通过 System 私有端口把焦点交还真实 trigger 子树。
                let focus_target = crate::ui::tree_widget_hooks::pointer_focus_target(self, target);
                // 触发完整焦点生命周期并保持具体组件边界不泄漏到通用事件模块。
                self.set_focus(Some(focus_target));
            }
        } else if button != MouseButton::Right {
            self.set_focus(None);
        }
        self.rebuild_widget_overlays();
        result
    }

    /// 捕获阶段：从 root 到 target 的路径上依次分发事件（不含 target 自身）。
    /// 任意节点返回 `Handled` 则终止捕获并阻止后续冒泡阶段。
    /// 用于 Modal 外部点击拦截、ScrollView 滚动拦截、全局快捷键等场景。
    pub(super) fn pointer_inside_widget_hit_frame(&self, target: WidgetId, pos: Point) -> bool {
        let Some(node) = self.get(target) else {
            return false;
        };
        if !node.visible() {
            return false;
        }

        // 捕获期间也必须遵守目标祖先链上的不连续父级片段。
        if !self.point_inside_parent_clip_regions(target, pos) {
            // 指针进入片段间隙时视为移出目标。
            return false;
        }

        self.point_to_node_layout(target, pos)
            .is_some_and(|pos| node.hit_test_frame(node.frame()).contains(pos))
    }

    pub(super) fn capture_wheel_to(
        &mut self,
        target: WidgetId,
        event: &SystemEvent,
    ) -> EventResult {
        // 从树级工作区取出独占路径，使回调重入只能使用另一份容器，不能改写当前快照。
        let mut path = std::mem::take(&mut self.wheel_capture_path_scratch);
        path.clear();
        let mut current = Some(target);
        while let Some(id) = current {
            path.push(id);
            current = self.get(id).and_then(|node| node.parent());
        }
        path.reverse();
        if path.last() == Some(&target) {
            path.pop();
        }

        // 按索引复制身份，避免消费容器并保持回调前建立的路径快照不变。
        for path_index in 0..path.len() {
            let id = path[path_index];
            let Some(translated) = self.localize_spatial_event(id, event) else {
                continue;
            };
            let result = {
                let node = match self.get_mut(id) {
                    Some(node) => node,
                    None => continue,
                };
                node.on_event(&translated)
            };
            if result == EventResult::Handled {
                // 完成处理期间仍隔离当前快照，避免语义回调重入覆盖外层路径。
                let result = self.finish_scroll_aware_dispatch(id, &translated);
                self.restore_wheel_capture_path_scratch(path);
                return result;
            }
        }
        self.restore_wheel_capture_path_scratch(path);
        EventResult::NotHandled
    }

    // 归还容量最大的空路径容器，兼容捕获回调重入时产生的嵌套工作区。
    fn restore_wheel_capture_path_scratch(&mut self, mut path: Vec<WidgetId>) {
        path.clear();
        if path.capacity() > self.wheel_capture_path_scratch.capacity() {
            self.wheel_capture_path_scratch = path;
        }
    }

    pub(super) fn dispatch_wheel_to(
        &mut self,
        target: WidgetId,
        event: &SystemEvent,
    ) -> EventResult {
        let mut current = Some(target);
        while let Some(id) = current {
            let Some(localized) = self.localize_spatial_event(id, event) else {
                return EventResult::NotHandled;
            };
            let (result, parent_id) = {
                let node = match self.get_mut(id) {
                    Some(node) => node,
                    None => return EventResult::NotHandled,
                };
                let result = node.on_event(&localized);
                (result, node.parent())
            };
            if result == EventResult::Handled {
                return self.finish_scroll_aware_dispatch(id, &localized);
            }
            current = parent_id;
        }
        EventResult::NotHandled
    }

    pub(super) fn finish_scroll_aware_dispatch(
        &mut self,
        id: WidgetId,
        event: &SystemEvent,
    ) -> EventResult {
        let requires_extended_finish = self
            .get(id)
            .is_none_or(BoxedWidget::requires_extended_event_finish);
        if !requires_extended_finish {
            // 普通事件组件只可能请求布局，随后按既有语义标记自身重绘。
            self.apply_event_layout_request(id);
            self.invalidate_paint(id);
            return EventResult::Handled;
        }
        if let Some(action) = self.get_mut(id).and_then(|node| node.take_window_action()) {
            self.pending_window_actions.push(action);
        }
        self.apply_event_layout_request(id);
        // 表格 capability 启用时才刷新扩展行与泛型单元格子树。
        #[cfg(feature = "table")]
        let table_children_changed =
            self.refresh_table_expand_widget(id) | self.refresh_table_cell_widget(id);
        // 表格 capability 关闭时不保留专属动态刷新分支。
        #[cfg(not(feature = "table"))]
        let table_children_changed = false;
        // 使用事件完成时的真实组件高度计算 VirtualScroll 当前物化窗口。
        let virtual_scroll_viewport_height = self.get(id).map(|node| node.frame().h);
        // 合并可选表格刷新与常驻动态组件刷新结果。
        let dynamic_children_changed = table_children_changed
            | self.refresh_select_option_widget(id)
            // Transfer 的选择或跨 pane 移动必须在同一事件内重建动态条目身份。
            | self.refresh_transfer_item_widget(id)
            | self.refresh_calendar_cell_widget(id)
            // Wheel 改变偏移后必须在同一事件中物化新窗口，不能等待无关布局。
            | self.refresh_virtual_scroll_widget(id, virtual_scroll_viewport_height);
        if dynamic_children_changed {
            self.push_layout_invalidation(id);
            self.propagate_layout_invalidation(id);
            // A rebuilt dynamic subtree invalidates the old viewport pixels.
            // Consume any queued delta so a later stable scroll cannot replay
            // movement that was already covered by this conservative repaint.
            let _ = self.get(id).and_then(|node| node.scroll_delta_for_dirty());
        }
        if dynamic_children_changed || !self.register_scroll_composite(id) {
            self.invalidate_paint(id);
        }
        let semantic = self.get(id).and_then(|node| node.semantic_event(id, event));
        if let Some(event) = semantic {
            let _ = self.dispatch_semantic(event);
        }
        EventResult::Handled
    }

    pub(super) fn register_scroll_composite(&mut self, id: WidgetId) -> bool {
        let transformed = self.path_has_visual_transform(id);
        let Some((dx, dy)) = self.get(id).and_then(|node| node.scroll_delta_for_dirty()) else {
            return false;
        };
        if transformed {
            return false;
        }
        // 由树级唯一入口完成视口投影、整数门禁与滚动条 chrome 失效。
        self.push_node_scroll_composite(id, dx, dy)
    }

    pub(crate) fn reveal_focused_target(&mut self, target: WidgetId) -> bool {
        let mut ancestors = Vec::new();
        let mut current = self.get(target).and_then(|node| node.parent());
        while let Some(id) = current {
            ancestors.push(id);
            current = self.get(id).and_then(|node| node.parent());
        }

        let mut changed = false;
        for viewport_id in ancestors {
            let Some((target_frame, viewport_frame)) = self.get(target).and_then(|target_node| {
                let target_visual = self.node_visual_rect(target, target_node.frame())?;
                let viewport_node = self.get(viewport_id)?;
                viewport_node.viewport_scroll_offset()?;
                let viewport_clip = viewport_node
                    .children_clip(viewport_node.frame())
                    .unwrap_or_else(|| viewport_node.frame());
                let inverse = self.node_visual_transform(viewport_id)?.inverse()?;
                Some((inverse.transform_rect(target_visual), viewport_clip))
            }) else {
                continue;
            };

            let dx = reveal_axis_delta(
                target_frame.x,
                target_frame.x + target_frame.w,
                viewport_frame.x,
                viewport_frame.x + viewport_frame.w,
            );
            let dy = reveal_axis_delta(
                target_frame.y,
                target_frame.y + target_frame.h,
                viewport_frame.y,
                viewport_frame.y + viewport_frame.h,
            );
            if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
                continue;
            }

            let scrolled = self
                .get_mut(viewport_id)
                .is_some_and(|node| node.scroll_descendant_by(dx, dy));
            if !scrolled {
                continue;
            }
            changed = true;
            if !self.register_scroll_composite(viewport_id) {
                self.invalidate_paint(viewport_id);
            }
        }
        changed
    }

    pub(super) fn capture_to(&mut self, target: WidgetId, event: &SystemEvent) -> Option<WidgetId> {
        // 暂时取走树级工作区；回调重入时内层会获得独立空槽，不别名外层路径。
        let mut path = std::mem::take(&mut self.event_capture_path_scratch);
        path.clear();
        // 收集从 root 到 target 的祖先路径（不含 target）。
        let mut current = self.get(target).and_then(|n| n.parent());
        while let Some(id) = current {
            path.push(id);
            current = self.get(id).and_then(|n| n.parent());
        }
        path.reverse(); // 现在是从 root → ... → target.parent

        let mut captured = None;
        for &id in &path {
            if !self.get(id).is_some_and(|node| node.wants_capture_phase()) {
                continue;
            }
            let Some(localized) = self.localize_spatial_event(id, event) else {
                continue;
            };
            let handled = {
                let node = match self.get_mut(id) {
                    Some(n) => n,
                    None => continue,
                };
                node.on_event(&localized) == EventResult::Handled
            };
            if handled {
                self.on_widget_handled_in_capture(id);
                let semantic = self
                    .get(id)
                    .and_then(|node| node.semantic_event(id, &localized));
                if let Some(event) = semantic {
                    let _ = self.dispatch_semantic(event);
                }
                captured = Some(id);
                break;
            }
        }
        // 保存已经增长的容量供下一次普通捕获分发复用。
        self.event_capture_path_scratch = path;
        captured
    }

    /// capture 阶段拦截事件后：标记拦截节点重绘。
    pub(super) fn on_widget_handled_in_capture(&mut self, id: WidgetId) {
        if let Some(action) = self.get_mut(id).and_then(|node| node.take_window_action()) {
            self.pending_window_actions.push(action);
        }
        self.apply_event_layout_request(id);
        self.invalidate_paint(id);
    }

    pub(super) fn apply_event_layout_request(&mut self, id: WidgetId) {
        let requested = self
            .get_mut(id)
            .is_some_and(|node| node.take_layout_request());
        if requested {
            self.push_layout_invalidation(id);
            self.propagate_layout_invalidation(id);
        }
    }

    pub(crate) fn dispatch_to(&mut self, target: WidgetId, event: &SystemEvent) -> EventResult {
        // 停止树不得通过定向分发直接调用节点事件回调。
        if !self.accepts_external_work() {
            // 对外保持事件未处理，避免泄露半提交节点状态。
            return EventResult::NotHandled;
        }
        if self.is_pending_removal_subtree(target) {
            return EventResult::NotHandled;
        }
        let mut current = Some(target);
        let secondary_drag_boundary = self.secondary_pointer_drag_boundary(target, event);
        // 每个冒泡节点都按自身的完整 visual/scroll 链反变换到局部坐标。
        while let Some(id) = current {
            if secondary_drag_boundary == Some(id) {
                return EventResult::Handled;
            }
            // 先以共享借用生成局部事件，再获取可变节点调用 on_event。
            let Some(localized) = self.localize_spatial_event(id, event) else {
                return EventResult::NotHandled;
            };

            // 处理 widget 自身的 on_event
            let (result, parent_id) = {
                let node = match self.get_mut(id) {
                    Some(n) => n,
                    None => return EventResult::NotHandled,
                };
                let r = node.on_event(&localized);
                (r, node.parent())
            };
            if result == EventResult::Handled {
                return self.finish_scroll_aware_dispatch(id, &localized);
            }

            // Bubbled 或 NotHandled → 继续向父节点传播
            current = parent_id;
        }
        EventResult::NotHandled
    }

    pub(super) fn dispatch_double_click_to(
        &mut self,
        target: WidgetId,
        event: &SystemEvent,
    ) -> EventResult {
        let mut current = Some(target);
        while let Some(id) = current {
            let Some(double_click) = self.localize_spatial_event(id, event) else {
                return EventResult::NotHandled;
            };
            let SystemEvent::PointerDoubleClick { pos, button, mods } = double_click else {
                return EventResult::NotHandled;
            };
            let double_click = SystemEvent::PointerDoubleClick { pos, button, mods };
            let pointer_down = SystemEvent::PointerDown { pos, button, mods };
            let (handled_event, parent) = {
                let Some(node) = self.get_mut(id) else {
                    return EventResult::NotHandled;
                };
                let handled = if node.on_event(&double_click) == EventResult::Handled {
                    Some(&double_click)
                } else if node.on_event(&pointer_down) == EventResult::Handled {
                    Some(&pointer_down)
                } else {
                    None
                };
                (handled.cloned(), node.parent())
            };
            if let Some(handled_event) = handled_event {
                return self.finish_scroll_aware_dispatch(id, &handled_event);
            }
            current = parent;
        }
        EventResult::NotHandled
    }

    pub(super) fn localize_spatial_event(
        &self,
        id: WidgetId,
        event: &SystemEvent,
    ) -> Option<SystemEvent> {
        let frame = self.get(id)?.frame();
        let map_point = |point: Point| {
            self.point_to_node_layout(id, point)
                .map(|point| Point::new(point.x - frame.x, point.y - frame.y))
        };
        let map_delta = |delta: Point| {
            let inverse = self.node_visual_transform(id)?.inverse()?;
            let origin = inverse.transform_point(Point::new(0.0, 0.0));
            let endpoint = inverse.transform_point(delta);
            Some(Point::new(endpoint.x - origin.x, endpoint.y - origin.y))
        };

        Some(match event {
            SystemEvent::PointerDown { pos, button, mods } => SystemEvent::PointerDown {
                pos: map_point(*pos)?,
                button: *button,
                mods: *mods,
            },
            SystemEvent::PointerDoubleClick { pos, button, mods } => {
                SystemEvent::PointerDoubleClick {
                    pos: map_point(*pos)?,
                    button: *button,
                    mods: *mods,
                }
            }
            SystemEvent::PointerUp { pos, button, mods } => SystemEvent::PointerUp {
                pos: map_point(*pos)?,
                button: *button,
                mods: *mods,
            },
            SystemEvent::PointerMove { pos, mods } => SystemEvent::PointerMove {
                pos: map_point(*pos)?,
                mods: *mods,
            },
            SystemEvent::Wheel { pos, delta } => SystemEvent::Wheel {
                pos: map_point(*pos)?,
                delta: map_delta(*delta)?,
            },
            SystemEvent::FileDrop { files, position } => SystemEvent::FileDrop {
                files: files.clone(),
                position: map_point(*position)?,
            },
            SystemEvent::DragStart { pos, button, mods } => SystemEvent::DragStart {
                pos: map_point(*pos)?,
                button: *button,
                mods: *mods,
            },
            SystemEvent::DragMove { pos, delta, mods } => SystemEvent::DragMove {
                pos: map_point(*pos)?,
                delta: map_delta(*delta)?,
                mods: *mods,
            },
            SystemEvent::DragEnd { pos, button, mods } => SystemEvent::DragEnd {
                pos: map_point(*pos)?,
                button: *button,
                mods: *mods,
            },
            other => other.clone(),
        })
    }

    pub(crate) fn set_focus(&mut self, new_focus: Option<WidgetId>) {
        // 停止树不得改变焦点管理器或触发焦点生命周期回调。
        if !self.accepts_external_work() {
            // 保持故障现场，等待所属窗口执行受控 teardown。
            return;
        }
        let old_focus = self.managers().focus.focused_widget();
        if new_focus == old_focus {
            return;
        }
        // A keyboard gesture belongs to the focus target that accepted its KeyDown.
        // Any real focus transition cancels it before FocusOut resets widget visuals.
        self.keyboard_activation = None;
        // 在任何用户回调前冻结两条包含路径，保持焦点事务观察同一份树结构。
        let mut paths = std::mem::take(&mut self.focus_transition_path_scratch);
        paths.clear();
        self.append_focus_containment_path(old_focus, &mut paths);
        let new_path_start = paths.len();
        self.append_focus_containment_path(new_focus, &mut paths);
        let new_path_len = paths.len() - new_path_start;
        // 两条 target→root 路径的共同后缀就是焦点仍位于其中的祖先链。
        let mut common_len = 0;
        while common_len < new_path_start
            && common_len < new_path_len
            && paths[new_path_start - common_len - 1] == paths[paths.len() - common_len - 1]
        {
            common_len += 1;
        }
        let old_unique_end = new_path_start - common_len;
        let new_unique_end = paths.len() - common_len;
        if let Some(old) = old_focus {
            self.invalidate_paint(old);
            if self.window_focused {
                let _ = self.dispatch_to(old, &SystemEvent::FocusOut);
            }
        }
        if self.window_focused {
            // 旧目标到共同祖先之前保持既有由内向外的离开顺序。
            for index in 0..old_unique_end {
                self.dispatch_focus_within(paths[index], false);
            }
        }
        self.managers_mut().focus.set_focused_widget(new_focus);
        if let Some(new) = new_focus {
            self.invalidate_paint(new);
            if self.window_focused {
                let _ = self.dispatch_to(new, &SystemEvent::FocusIn);
            }
        }
        if self.window_focused {
            // 新目标独有路径反向遍历，保持由共同祖先向内进入的顺序。
            for index in (new_path_start..new_unique_end).rev() {
                self.dispatch_focus_within(paths[index], true);
            }
            // 生命周期协调可能继续使用树级工作区，先归还焦点路径所有权。
            paths.clear();
            self.focus_transition_path_scratch = paths;
            self.reconcile_lifecycle_after_layout();
        } else {
            // 窗口失焦时虽然不发送 focus-within，也必须保留已扩容工作区。
            paths.clear();
            self.focus_transition_path_scratch = paths;
        }
    }

    pub(super) fn focus_containment_path(&self, target: Option<WidgetId>) -> Vec<WidgetId> {
        let mut path = Vec::new();
        self.append_focus_containment_path(target, &mut path);
        path
    }

    // 把目标到根的包含链追加到调用方拥有的快照，供焦点事务与窗口生命周期复用。
    fn append_focus_containment_path(&self, target: Option<WidgetId>, path: &mut Vec<WidgetId>) {
        let mut current = target;
        while let Some(id) = current {
            path.push(id);
            current = self.get(id).and_then(|node| node.parent());
        }
    }

    pub(super) fn dispatch_focus_within(&mut self, id: WidgetId, focused: bool) {
        let handled = self
            .get_mut(id)
            .is_some_and(|node| node.on_focus_within(focused) == EventResult::Handled);
        if handled {
            self.on_widget_handled_in_capture(id);
        }
    }

    /// NavItem 共享 active 索引时，刷新整组导航项（取消/选中态联动；
    /// 组件语义见 System 私有边界 tree_widget_hooks）。
    pub(super) fn invalidate_nav_siblings(&mut self, clicked: WidgetId) {
        crate::ui::tree_widget_hooks::invalidate_nav_siblings(self, clicked);
    }
}

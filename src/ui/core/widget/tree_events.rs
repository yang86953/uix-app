use super::tree_core::WidgetTree;
use super::*;
use crate::ui::event::{ClickEvent, SemanticEvent, WindowAction};

mod keyboard_routing;
mod pointer_routing;
mod window_lifecycle;

impl WidgetTree {
    /// 2D 命中测试：根据屏幕坐标找到最深的 widget。
    pub fn hit_test(&self, pos: Point) -> Option<ComponentId> {
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

    fn hit_test_internal(&self, id: WidgetId, pos: Point) -> Option<WidgetId> {
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

    pub fn dispatch_event(&mut self, event: &SystemEvent) -> EventResult {
        self.begin_invalidation_batch();
        let result = self.dispatch_event_inner(event);
        self.cancel_hidden_interaction();
        self.finish_invalidation_batch();
        result
    }

    fn dispatch_event_inner(&mut self, event: &SystemEvent) -> EventResult {
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
                        .is_some_and(super::text_selection::is_dragging)
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

    fn dispatch_pointer_press(
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
            if self
                .get(target)
                .is_some_and(super::text_selection::participates)
            {
                self.clear_sibling_cross_text_selections(target);
            }
            self.invalidate_nav_siblings(target);
            let preserves_keyboard_focus = button == MouseButton::Right
                || self.pending_window_actions[actions_before_dispatch..]
                    .iter()
                    .any(|action| action.preserves_keyboard_focus());
            if !preserves_keyboard_focus {
                self.set_focus(Some(target));
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
    fn pointer_inside_widget_hit_frame(&self, target: WidgetId, pos: Point) -> bool {
        let Some(node) = self.get(target) else {
            return false;
        };
        if !node.visible() {
            return false;
        }

        self.point_to_node_layout(target, pos)
            .is_some_and(|pos| node.hit_test_frame(node.frame()).contains(pos))
    }

    fn capture_wheel_to(&mut self, target: WidgetId, event: &SystemEvent) -> EventResult {
        let mut path = Vec::new();
        let mut current = Some(target);
        while let Some(id) = current {
            path.push(id);
            current = self.get(id).and_then(|node| node.parent());
        }
        path.reverse();
        if path.last() == Some(&target) {
            path.pop();
        }

        for id in path {
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
                return self.finish_scroll_aware_dispatch(id, &translated);
            }
        }
        EventResult::NotHandled
    }

    fn dispatch_wheel_to(&mut self, target: WidgetId, event: &SystemEvent) -> EventResult {
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

    fn finish_scroll_aware_dispatch(&mut self, id: WidgetId, event: &SystemEvent) -> EventResult {
        if let Some(action) = self.get_mut(id).and_then(|node| node.take_window_action()) {
            self.pending_window_actions.push(action);
        }
        self.apply_event_layout_request(id);
        let dynamic_children_changed = self.refresh_table_expand_component(id)
            | self.refresh_table_cell_component(id)
            | self.refresh_select_option_component(id)
            | self.refresh_calendar_cell_component(id);
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

    fn register_scroll_composite(&mut self, id: WidgetId) -> bool {
        let transformed = self.path_has_visual_transform(id);
        let Some((viewport, dx, dy)) = self.get(id).and_then(|node| {
            let frame = node.frame();
            node.scroll_delta_for_dirty().map(|(dx, dy)| {
                let viewport = node.scroll_composite_viewport(frame);
                (viewport, dx, dy)
            })
        }) else {
            return false;
        };
        if transformed {
            return false;
        }
        self.push_scroll_composite(viewport, dx, dy)
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

    fn capture_to(&mut self, target: WidgetId, event: &SystemEvent) -> Option<WidgetId> {
        // 收集从 root 到 target 的祖先路径（不含 target）
        let mut path = Vec::new();
        let mut current = self.get(target).and_then(|n| n.parent());
        while let Some(id) = current {
            path.push(id);
            current = self.get(id).and_then(|n| n.parent());
        }
        path.reverse(); // 现在是从 root → ... → target.parent

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
                return Some(id);
            }
        }
        None
    }

    /// capture 阶段拦截事件后：标记拦截节点重绘。
    fn on_widget_handled_in_capture(&mut self, id: WidgetId) {
        if let Some(action) = self.get_mut(id).and_then(|node| node.take_window_action()) {
            self.pending_window_actions.push(action);
        }
        self.apply_event_layout_request(id);
        self.invalidate_paint(id);
    }

    fn apply_event_layout_request(&mut self, id: WidgetId) {
        let requested = self
            .get_mut(id)
            .is_some_and(|node| node.take_layout_request());
        if requested {
            self.push_layout_invalidation(id);
            self.propagate_layout_invalidation(id);
        }
    }

    pub(crate) fn dispatch_to(&mut self, target: WidgetId, event: &SystemEvent) -> EventResult {
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

    fn dispatch_double_click_to(&mut self, target: WidgetId, event: &SystemEvent) -> EventResult {
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

    fn localize_spatial_event(&self, id: WidgetId, event: &SystemEvent) -> Option<SystemEvent> {
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
        let old_focus = self.managers().focus.focused_component();
        if new_focus == old_focus {
            return;
        }
        // A keyboard gesture belongs to the focus target that accepted its KeyDown.
        // Any real focus transition cancels it before FocusOut resets widget visuals.
        self.keyboard_activation = None;
        let old_path = self.focus_containment_path(old_focus);
        let new_path = self.focus_containment_path(new_focus);
        if let Some(old) = old_focus {
            self.invalidate_paint(old);
            if self.window_focused {
                let _ = self.dispatch_to(old, &SystemEvent::FocusOut);
            }
        }
        if self.window_focused {
            for &id in &old_path {
                if !new_path.contains(&id) {
                    self.dispatch_focus_within(id, false);
                }
            }
        }
        self.managers_mut().focus.set_focused_component(new_focus);
        if let Some(new) = new_focus {
            self.invalidate_paint(new);
            if self.window_focused {
                let _ = self.dispatch_to(new, &SystemEvent::FocusIn);
            }
        }
        if self.window_focused {
            for &id in new_path.iter().rev() {
                if !old_path.contains(&id) {
                    self.dispatch_focus_within(id, true);
                }
            }
            self.reconcile_lifecycle_after_layout();
        }
    }

    fn focus_containment_path(&self, target: Option<WidgetId>) -> Vec<WidgetId> {
        let mut path = Vec::new();
        let mut current = target;
        while let Some(id) = current {
            path.push(id);
            current = self.get(id).and_then(|node| node.parent());
        }
        path
    }

    fn dispatch_focus_within(&mut self, id: WidgetId, focused: bool) {
        let handled = self
            .get_mut(id)
            .is_some_and(|node| node.on_focus_within(focused) == EventResult::Handled);
        if handled {
            self.on_widget_handled_in_capture(id);
        }
    }

    /// NavItem 共享 active 索引时，刷新整组导航项（取消/选中态联动）。
    fn invalidate_nav_siblings(&mut self, clicked: WidgetId) {
        use crate::ui::widgets::nav::NavItem;
        let is_nav = self
            .get(clicked)
            .is_some_and(|n| n.component().as_any().type_id() == std::any::TypeId::of::<NavItem>());
        if !is_nav {
            return;
        }
        if let Some(parent) = self.get(clicked).and_then(|n| n.parent()) {
            self.invalidate_paint_subtree(parent);
        }
    }
}

fn reveal_axis_delta(target_start: f32, target_end: f32, view_start: f32, view_end: f32) -> f32 {
    if target_start < view_start {
        target_start - view_start
    } else if target_end > view_end {
        target_end - view_end
    } else {
        0.0
    }
}

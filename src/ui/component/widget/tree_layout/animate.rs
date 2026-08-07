use super::super::super::*;
use super::super::WidgetTree;
use crate::core::Rect;
use crate::ui::animation::AnimatedRegistration;
use std::time::Instant;

impl WidgetTree {

    pub fn update(&mut self, dt: f64) -> bool {
        self.update_animations(dt)
            .into_iter()
            .any(|(_, still_active)| still_active)
    }

    pub(crate) fn update_animations(&mut self, dt: f64) -> Vec<(WidgetId, bool)> {
        self.update_animations_at(Instant::now(), dt)
    }

    pub(crate) fn update_animations_at(&mut self, now: Instant, dt: f64) -> Vec<(WidgetId, bool)> {
        let ids = self.take_animation_node_ids();
        let updates = self.update_animation_nodes_at(ids.iter().copied(), now, dt);
        self.animation_ids_scratch = ids;
        updates
    }

    pub(crate) fn update_animations_except_at(
        &mut self,
        excluded_ids: &[WidgetId],
        now: Instant,
        dt: f64,
    ) -> Vec<(WidgetId, bool)> {
        let mut ids = self.take_animation_node_ids();
        ids.retain(|id| !excluded_ids.contains(id));
        let updates = self.update_animation_nodes_at(ids.iter().copied(), now, dt);
        self.animation_ids_scratch = ids;
        updates
    }

    pub(crate) fn take_animation_node_ids(&mut self) -> Vec<WidgetId> {
        let mut ids = std::mem::take(&mut self.animation_ids_scratch);
        ids.clear();
        ids.extend(
            self.traverse()
                .iter()
                .copied()
                .filter(|&id| self.active_animation_frame(id).is_some()),
        );
        ids.extend(
            self.animated_sources
                .iter()
                .filter(|(_, source)| {
                    source.source.registration() != AnimatedRegistration::Inactive
                })
                .map(|(id, _)| *id),
        );
        ids
    }

    pub(crate) fn animated_source_registrations(&self) -> Vec<(WidgetId, Option<Instant>)> {
        self.animated_sources
            .iter()
            .filter_map(|(&id, source)| match source.source.registration() {
                AnimatedRegistration::Inactive => None,
                AnimatedRegistration::Open => Some((id, None)),
                AnimatedRegistration::Deadline(deadline) => Some((id, Some(deadline))),
            })
            .collect()
    }

    pub(crate) fn active_animation_frame(&self, id: WidgetId) -> Option<Rect> {
        let node = self.get(id)?;
        let visible_and_active = self.is_effectively_visible(id) && node.active();
        let view_transition =
            node.view_transition_active() && (node.pending_removal() || visible_and_active);
        let component_animation = visible_and_active
            && !self.is_pending_removal_subtree(id)
            && node
                .capabilities()
                .contains(crate::ui::component::traits::WidgetCapabilities::ANIMATION);
        (view_transition || component_animation).then_some(node.frame())
    }

    // 测试目标保留活动过渡 id 观测入口，供动画生命周期测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn active_view_transition_ids(&self) -> Vec<WidgetId> {
        self.traverse()
            .iter()
            .copied()
            .filter(|&id| {
                self.get(id)
                    .is_some_and(BoxedWidget::view_transition_active)
                    && (self.get(id).is_some_and(BoxedWidget::pending_removal)
                        || self.is_effectively_visible(id))
            })
            .collect()
    }

    // 测试目标保留过渡注册观测入口，供动画生命周期测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn view_transition_registrations(&self) -> Vec<(WidgetId, Option<Instant>)> {
        let mut registrations = Vec::new();
        self.extend_view_transition_registrations(&mut registrations);
        registrations
    }

    pub(crate) fn extend_view_transition_registrations(
        &self,
        registrations: &mut Vec<(WidgetId, Option<Instant>)>,
    ) {
        registrations.extend(self.traverse().iter().copied().filter_map(|id| {
            let node = self.get(id)?;
            (node.view_transition_active()
                && (node.pending_removal() || self.is_effectively_visible(id)))
            .then_some((id, node.view_transition_deadline()))
        }));
    }

    // 测试目标保留动画节点更新便捷入口，供时间推进测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn update_animation_nodes<I>(&mut self, ids: I, dt: f64) -> Vec<(WidgetId, bool)>
    where
        I: IntoIterator<Item = WidgetId>,
    {
        self.update_animation_nodes_at(ids, Instant::now(), dt)
    }

    pub(crate) fn update_animation_nodes_at<I>(
        &mut self,
        ids: I,
        now: Instant,
        dt: f64,
    ) -> Vec<(WidgetId, bool)>
    where
        I: IntoIterator<Item = WidgetId>,
    {
        let mut updates = Vec::new();
        let mut widget_overlays_changed = false;
        let mut completed_removals = Vec::new();
        for id in ids {
            if let Some(source) = self.animated_sources.get(&id) {
                updates.push((id, source.source.advance(now, dt)));
                continue;
            }
            let Some(frame) = self.active_animation_frame(id) else {
                self.active_component_animations.remove(&id);
                updates.push((id, false));
                continue;
            };

            let modal_was_present = crate::ui::tree_widget_hooks::modal_was_present(self, id);

            let view_was_active = self
                .get(id)
                .is_some_and(BoxedWidget::view_transition_active);
            let view_is_waiting = self
                .get(id)
                .and_then(BoxedWidget::view_transition_deadline)
                .is_some_and(|deadline| deadline > now);
            let old_visual_bounds = (view_was_active && !view_is_waiting)
                .then(|| self.visual_subtree_bounds(id))
                .flatten();
            let (view_still_active, remove_now) = if view_was_active && !view_is_waiting {
                self.get_mut(id)
                    .map_or((false, false), |node| node.advance_view_transition(now, dt))
            } else {
                (false, false)
            };
            let new_visual_bounds = (view_was_active && !view_is_waiting)
                .then(|| self.visual_subtree_bounds(id))
                .flatten();
            for rect in [old_visual_bounds, new_visual_bounds].into_iter().flatten() {
                if rect.w > 0.0 && rect.h > 0.0 {
                    self.push_paint_invalidation(id, Some(rect));
                }
            }

            let (component_still_active, dirty) = self
                .get_mut(id)
                .and_then(|node| {
                    let animation = node.component_mut().as_animation_mut()?;
                    let still_active = animation.update_animation(dt);
                    let dirty = animation.dirty_bounds(frame);
                    Some((still_active, dirty))
                })
                .unwrap_or((false, Rect::zero()));
            let dynamic_children_changed = self.refresh_select_option_component(id);
            if dynamic_children_changed {
                self.push_layout_invalidation(id);
                self.propagate_layout_invalidation(id);
                self.invalidate_paint(id);
            }
            let animation_layout_requested = self
                .get_mut(id)
                .is_some_and(|node| node.take_layout_request());
            if animation_layout_requested {
                self.push_layout_invalidation(id);
                self.propagate_layout_invalidation(id);
            }
            if component_still_active {
                self.active_component_animations.insert(id);
            } else {
                self.active_component_animations.remove(&id);
            }

            if dirty.w > 0.0 && dirty.h > 0.0 {
                self.invalidate_paint_rect(id, dirty);
            }
            widget_overlays_changed |= !self.widget_overlay_is_current(id);
            updates.push((id, view_still_active || component_still_active));
            if remove_now {
                completed_removals.push(id);
            }

            let modal_closed_for_destruction = modal_was_present
                && crate::ui::tree_widget_hooks::modal_closed_for_destruction(self, id);
            if modal_closed_for_destruction {
                completed_removals.push(id);
            }
        }
        for id in completed_removals {
            if self.get(id).is_some() {
                self.remove(id);
                widget_overlays_changed = true;
            }
        }
        self.cancel_hidden_interaction();
        if widget_overlays_changed || updates.iter().any(|(_, still_active)| !still_active) {
            self.rebuild_widget_overlays();
        }
        updates
    }

    pub(crate) fn component_animation_ids(&self) -> impl Iterator<Item = WidgetId> + '_ {
        self.active_component_animations
            .iter()
            .copied()
            .filter(|id| self.get(*id).is_some())
    }

    pub(crate) fn widget_overlay_is_current(&self, id: WidgetId) -> bool {
        let desired = (self.is_effectively_visible(id) && !self.is_pending_removal_subtree(id))
            .then(|| self.get(id))
            .flatten()
            .and_then(|node| node.overlay_entry(id, node.frame()));
        let mut current = self
            .overlay_stack
            .iter()
            .filter(|entry| !entry.is_managed() && entry.owner() == id);

        match (desired, current.next()) {
            (None, None) => true,
            (Some(desired), Some(current_entry)) if current.next().is_none() => {
                desired.kind() == current_entry.kind()
                    && desired.bounds_rect() == current_entry.bounds_rect()
                    && desired.z_index_value() == current_entry.z_index_value()
                    && desired.is_modal() == current_entry.is_modal()
                    && desired.dismisses_on_outside() == current_entry.dismisses_on_outside()
                    && desired.traps_focus() == current_entry.traps_focus()
            }
            _ => false,
        }
    }
}



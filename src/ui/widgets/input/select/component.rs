use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::native::windowing::input::ControlSize;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::animation::TransitionPlayer;
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::{
    ComponentId, EventResult, KeyCode, LayoutChild, MouseButton, SemanticEvent, SystemEvent, WidgetTree,
};
use std::cell::{Cell, RefCell};

use super::search::VisibleRow;
use super::{OptGroup, SelectValueBinding, select_dirty_rect, select_popup_rect};

const DROPDOWN_ROW_HEIGHT: f32 = 28.0;

component! {
    pub struct Select {
        pub(crate) options: Vec<String>,
        pub(crate) optgroups: Vec<OptGroup>,
        pub(crate) selected: usize,
        pub(crate) selected_multi: Vec<usize>,
        pub(crate) value_binding: Option<SelectValueBinding>,
        pub(crate) open: bool,
        pub(crate) disabled: bool,
        pub(crate) loading: bool,
        pub(crate) loading_phase: f32,
        pub(crate) loading_dirty: bool,
        pub(crate) select_size: ControlSize,
        pub(crate) hovered: bool,
        pub(crate) focused: bool,
        pub(crate) transition: TransitionPlayer,
        pub(crate) closing: bool,
        pub(crate) transition_dirty: bool,
        pub(crate) placeholder: String,
        pub(crate) hovered_option: Option<usize>,
        pub(crate) highlighted_option: Option<usize>,
        pub(crate) pending_change: RefCell<Option<String>>,
        pub(crate) multiple: bool,
        pub(crate) search: bool,
        pub(crate) search_query: String,
        pub(crate) custom_option_views: bool,
        pub(crate) materialized_custom_options: RefCell<Vec<usize>>,
        pub(crate) search_cursor_rect: Cell<Rect>,
        pub(crate) control_rect: Cell<Rect>,
        pub(crate) dropdown_rect: Cell<Rect>,
        pub(crate) multi_remove_rects: RefCell<Vec<(usize, Rect)>>,
        pub(crate) dropdown_scroll: VirtualListScroll,
        pub(crate) scroll_delta_strip: Cell<(f32, f32)>,
    }

    tab_index => (&self) -> i32 { 1 }

    accepts_text_input => (&self) -> bool { self.search && !self.disabled }

    text_input_cursor_rect => (&self) -> Rect { self.search_cursor_rect.get() }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        let rows = self.visible_rows();
        let popup = self.dropdown_damage_rect();
        let list_y = frame.y + popup.y;
        let text_left = if self.multiple { 32.0 } else { 10.0 };
        let content_width = (frame.w - text_left - 32.0).max(0.0);
        let indices = self.materialized_custom_options.borrow();
        children
            .iter()
            .zip(indices.iter().copied())
            .filter_map(|(child, option_index)| {
                let row_index = rows.iter().position(
                    |row| matches!(row, VisibleRow::Option(index) if *index == option_index),
                )?;
                Some((
                    child.id,
                    Rect::new(
                        frame.x + text_left,
                        list_y + row_index as f32 * DROPDOWN_ROW_HEIGHT
                            - self.dropdown_scroll.scroll_offset(),
                        content_width,
                        DROPDOWN_ROW_HEIGHT,
                    ),
                ))
            })
            .collect()
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        (self.custom_option_views && self.is_present()).then(|| {
            let popup = self.dropdown_damage_rect();
            Rect::new(frame.x + popup.x, frame.y + popup.y, popup.w, popup.h)
        })
    }

    hit_test_children => (&self) -> bool { false }

    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        let delta = self.scroll_delta_strip.get();
        if delta.0.abs() > 0.01 || delta.1.abs() > 0.01 {
            self.scroll_delta_strip.set((0.0, 0.0));
            Some(delta)
        } else {
            None
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if self.disabled {
            return EventResult::NotHandled;
        }
        self.sync_bound_selection();

        let visible_options = self.visible_option_indices();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if self.control_rect.get().contains(*pos) {
                    if self.multiple {
                        let remove = self
                            .multi_remove_rects
                            .borrow()
                            .iter()
                            .find_map(|(index, rect)| rect.contains(*pos).then_some(*index));
                        if let Some(index) = remove {
                            if let Some(position) = self
                                .selected_multi
                                .iter()
                                .position(|selected| *selected == index)
                            {
                                self.selected_multi.remove(position);
                                self.publish_multi_change();
                            }
                            return EventResult::Handled;
                        }
                    }
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    self.hovered_option = None;
                    return EventResult::Handled;
                }

                if self.is_present() && self.dropdown_rect.get().contains(*pos) {
                    if let Some(flat_idx) = self.dropdown_row_at_y(pos.y) {
                        if let Some(opt_idx) = self.flat_row_option_index(flat_idx) {
                            self.highlighted_option = Some(opt_idx);
                            if self.multiple {
                                if let Some(multi_idx) =
                                    self.selected_multi.iter().position(|&i| i == opt_idx)
                                {
                                    self.selected_multi.remove(multi_idx);
                                } else {
                                    self.selected_multi.push(opt_idx);
                                }
                                self.publish_multi_change();
                            } else {
                                self.select_single(opt_idx);
                                self.close();
                            }
                            return EventResult::Handled;
                        }
                    }
                    return EventResult::Handled;
                }

                self.close();
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.is_present() && self.dropdown_rect.get().contains(*pos) {
                    self.hovered_option = self.dropdown_row_at_y(pos.y);
                } else {
                    self.hovered_option = None;
                }
                self.hovered = self.control_rect.get().contains(*pos);
                EventResult::Handled
            }
            SystemEvent::PointerEnter => {
                self.hovered = true;
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.hovered = false;
                self.hovered_option = None;
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.close();
                EventResult::Handled
            }
            SystemEvent::Wheel { delta, pos, .. } => {
                if self.is_present() && self.dropdown_rect.get().contains(*pos) {
                    let row_count = self.dropdown_row_count();
                    let viewport_h = self.dropdown_viewport_height(row_count);
                    let dy = self.dropdown_scroll.scroll_by_wheel(
                        delta.y,
                        row_count,
                        DROPDOWN_ROW_HEIGHT,
                        viewport_h,
                    );
                    if dy.abs() > 0.01 {
                        self.push_scroll_delta(0.0, dy);
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Down => {
                    if self.open {
                        if self.search || self.multiple {
                            self.move_highlight(true);
                        } else {
                            let position = visible_options
                                .iter()
                                .position(|index| *index == self.selected);
                            let next = match position {
                                Some(position) => visible_options.get(position + 1),
                                None => visible_options.first(),
                            }
                            .copied();
                            if let Some(next) = next {
                                self.select_single(next);
                            }
                        }
                    } else {
                        self.open();
                    }
                    EventResult::Handled
                }
                KeyCode::Up => {
                    if self.open && !visible_options.is_empty() {
                        if self.search || self.multiple {
                            self.move_highlight(false);
                        } else {
                            let position = visible_options
                                .iter()
                                .position(|index| *index == self.selected);
                            let prev = match position {
                                Some(position) => {
                                    visible_options.get(position.saturating_sub(1))
                                }
                                None => visible_options.last(),
                            }
                            .copied()
                            .unwrap_or(self.selected);
                            self.select_single(prev);
                        }
                    }
                    EventResult::Handled
                }
                KeyCode::Enter => {
                    if self.open {
                        if self.multiple {
                            self.toggle_highlighted_multi();
                        } else if self.search {
                            let selection = self
                                .highlighted_option
                                .filter(|index| visible_options.contains(index))
                                .or_else(|| visible_options.first().copied());
                            if let Some(index) = selection {
                                self.select_single(index);
                            }
                            self.close();
                        } else {
                            self.close();
                        }
                    } else {
                        self.open();
                    }
                    EventResult::Handled
                }
                KeyCode::Space if self.search => EventResult::NotHandled,
                KeyCode::Space => {
                    if self.open {
                        if self.multiple {
                            self.toggle_highlighted_multi();
                        } else {
                            self.close();
                        }
                    } else {
                        self.open();
                    }
                    EventResult::Handled
                }
                KeyCode::Escape => {
                    self.close();
                    EventResult::Handled
                }
                KeyCode::Backspace => {
                    if self.search && !self.search_query.is_empty() {
                        self.search_query.pop();
                        self.refresh_search_results();
                    } else if self.multiple && !self.selected_multi.is_empty() {
                        self.selected_multi.pop();
                        self.publish_multi_change();
                    }
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            SystemEvent::TextInput { text } if self.search => {
                if text.is_empty() || text.chars().any(char::is_control) {
                    return EventResult::NotHandled;
                }
                self.search_query.push_str(text);
                self.refresh_search_results();
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|value| SemanticEvent::change(id, value))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.is_present() {
            select_dirty_rect(frame, self.dropdown_damage_rect())
        } else {
            frame
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.render_select(frame, ctx);
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        select_dirty_rect(frame, self.dropdown_damage_rect())
    }

    overlay_entry => (&self, id: ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.is_present() {
            return None;
        }

        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                .bounds(select_popup_rect(frame, self.dropdown_damage_rect()))
                .z_index(900),
        )
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() {
            self.transition_dirty = false;
            self.loading_dirty = false;
            return false;
        }

        let transition_active = if self.transition.finished {
            if self.closing {
                self.open = false;
                self.closing = false;
                self.search_query.clear();
                self.highlighted_option = None;
            }
            self.transition_dirty = false;
            false
        } else {
            self.transition.update(dt);
            self.transition_dirty = true;

            if self.closing && self.transition.finished {
                self.open = false;
                self.closing = false;
                self.search_query.clear();
                self.highlighted_option = None;
            }

            self.is_present() && !self.transition.finished
        };

        self.loading_dirty = false;
        let loading_active = self.loading && self.is_present();
        if loading_active {
            let before = self.loading_phase;
            self.loading_phase = (self.loading_phase
                + dt.max(0.0) as f32 * std::f32::consts::TAU / 0.8)
                .rem_euclid(std::f32::consts::TAU);
            self.loading_dirty = (self.loading_phase - before).abs() > f32::EPSILON;
        }

        transition_active || loading_active
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty || self.loading_dirty {
            select_dirty_rect(frame, self.dropdown_damage_rect())
        } else {
            Rect::zero()
        }
    }
}


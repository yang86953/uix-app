use crate::core::{Rect, Size};

use super::search::VisibleRow;
use super::{DROPDOWN_ROW_HEIGHT, Select};


impl Select {

    pub(crate) fn control_height(&self) -> f32 {
        crate::ui::component::config::control_height(self.select_size)
    }

    pub(crate) fn intrinsic_size(&self) -> Size {
        let mut max_text_width = crate::draw::resources::font::text_backend::estimate_text_metrics(
            &self.placeholder,
            f32::INFINITY,
            13.0,
        )
        .max_line_width;
        for option in &self.options {
            max_text_width = max_text_width.max(
                crate::draw::resources::font::text_backend::estimate_text_metrics(
                    option,
                    f32::INFINITY,
                    13.0,
                )
                .max_line_width,
            );
        }
        for group in &self.optgroups {
            max_text_width = max_text_width.max(
                crate::draw::resources::font::text_backend::estimate_text_metrics(
                    &group.label,
                    f32::INFINITY,
                    12.0,
                )
                .max_line_width,
            );
            for option in &group.options {
                max_text_width = max_text_width.max(
                    crate::draw::resources::font::text_backend::estimate_text_metrics(
                        option,
                        f32::INFINITY,
                        13.0,
                    )
                    .max_line_width,
                );
            }
        }
        Size::new((max_text_width + 40.0).max(120.0), self.control_height())
    }

    pub(crate) fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    pub(crate) fn refresh_search_results(&mut self) {
        self.hovered_option = None;
        self.dropdown_scroll.set_scroll_offset(0.0);
        self.highlighted_option = self.visible_option_indices().first().copied();
        if !self.open {
            self.open();
        } else {
            self.transition_dirty = true;
        }
    }

    pub(crate) fn selected_multi_payload(&self) -> String {
        self.selected_multi
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub(crate) fn sync_bound_selection(&mut self) {
        let Some(binding) = self.value_binding.as_ref() else {
            return;
        };
        let multiple = binding.multiple;
        let selected = {
            let options = self.all_options();
            (binding.read)(&options)
        };
        if multiple {
            self.selected_multi = selected;
        } else {
            self.selected = selected.first().copied().unwrap_or(usize::MAX);
        }
    }

    pub(crate) fn capture_bound_value_dependency(&self) {
        if let Some(binding) = self.value_binding.as_ref() {
            (binding.capture)();
        }
    }

    pub(crate) fn write_bound_selection(&self) {
        let Some(binding) = self.value_binding.as_ref() else {
            return;
        };
        let options = self.all_options();
        if binding.multiple {
            (binding.write)(&options, &self.selected_multi);
        } else {
            (binding.write)(&options, &[self.selected]);
        }
    }

    pub(crate) fn select_single(&mut self, index: usize) {
        self.highlighted_option = Some(index);
        if self.selected == index {
            return;
        }
        self.selected = index;
        self.write_bound_selection();
        self.pending_change.replace(Some(index.to_string()));
    }

    pub(crate) fn publish_multi_change(&self) {
        self.write_bound_selection();
        self.pending_change
            .replace(Some(self.selected_multi_payload()));
    }

    pub(crate) fn move_highlight(&mut self, forward: bool) {
        let visible = self.visible_option_indices();
        if visible.is_empty() {
            self.highlighted_option = None;
            return;
        }
        let current = self
            .highlighted_option
            .and_then(|index| visible.iter().position(|visible| *visible == index));
        let position = match (current, forward) {
            (Some(position), true) => (position + 1).min(visible.len() - 1),
            (Some(position), false) => position.saturating_sub(1),
            (None, true) => 0,
            (None, false) => visible.len() - 1,
        };
        let option_index = visible[position];
        self.highlighted_option = Some(option_index);
        self.reveal_option(option_index);
    }

    pub(crate) fn toggle_highlighted_multi(&mut self) {
        let Some(option_index) = self
            .highlighted_option
            .or_else(|| self.visible_option_indices().first().copied())
        else {
            return;
        };
        if let Some(position) = self
            .selected_multi
            .iter()
            .position(|selected| *selected == option_index)
        {
            self.selected_multi.remove(position);
        } else {
            self.selected_multi.push(option_index);
        }
        self.highlighted_option = Some(option_index);
        self.publish_multi_change();
    }

    pub(crate) fn reveal_option(&mut self, option_index: usize) {
        let visible_rows = self.visible_rows();
        let Some(row_index) = visible_rows
            .iter()
            .position(|row| matches!(row, VisibleRow::Option(index) if *index == option_index))
        else {
            return;
        };
        let row_count = self.dropdown_row_count();
        let viewport_height = self.dropdown_viewport_height(row_count);
        let old_offset = self.dropdown_scroll.scroll_offset();
        let row_top = row_index as f32 * DROPDOWN_ROW_HEIGHT;
        let row_bottom = row_top + DROPDOWN_ROW_HEIGHT;
        let new_offset = if row_top < old_offset {
            row_top
        } else if row_bottom > old_offset + viewport_height {
            row_bottom - viewport_height
        } else {
            old_offset
        };
        self.dropdown_scroll.set_scroll_offset(new_offset);
        self.dropdown_scroll
            .clamp_to_content(row_count, DROPDOWN_ROW_HEIGHT, viewport_height);
        let applied = self.dropdown_scroll.scroll_offset() - old_offset;
        if applied.abs() > 0.01 {
            self.push_scroll_delta(0.0, applied);
        }
    }

    pub(crate) fn dropdown_damage_rect(&self) -> Rect {
        let control = self.control_rect.get();
        let height = self.dropdown_viewport_height(self.dropdown_damage_row_count());
        let y = if self.dropdown_rect.get().y < 0.0 {
            -height
        } else {
            control.h
        };
        Rect::new(0.0, y, control.w, height)
    }

    pub(crate) fn custom_option_indices(&self) -> Vec<usize> {
        if !self.custom_option_views || !self.is_present() || self.loading {
            return Vec::new();
        }
        let rows = self.visible_rows();
        let row_count = self.dropdown_row_count();
        let viewport_height = self.dropdown_viewport_height(row_count);
        let (start, end) =
            self.dropdown_scroll
                .scroll_range(row_count, DROPDOWN_ROW_HEIGHT, viewport_height);
        rows[start.min(rows.len())..end.min(rows.len())]
            .iter()
            .filter_map(|row| match row {
                VisibleRow::Option(index) => Some(*index),
                VisibleRow::Group(_) => None,
            })
            .collect()
    }

    pub(crate) fn custom_option_labels(&self, indices: &[usize]) -> Vec<String> {
        indices
            .iter()
            .filter_map(|index| self.option_label(*index).map(str::to_owned))
            .collect()
    }

    pub(crate) fn needs_custom_option_refresh(
        &self,
        indices: &[usize],
        child_count: usize,
    ) -> bool {
        self.materialized_custom_options.borrow().as_slice() != indices
            || child_count != indices.len()
    }

    pub(crate) fn mark_custom_options_materialized(&self, indices: Vec<usize>) {
        *self.materialized_custom_options.borrow_mut() = indices;
    }

    pub(crate) fn invalidate_custom_option_materialization(&self) {
        self.materialized_custom_options.borrow_mut().clear();
    }

}



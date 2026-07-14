use super::{Select, DROPDOWN_ROW_HEIGHT, DROPDOWN_TRIGGER_HEIGHT};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VisibleRow {
    Group(usize),
    Option(usize),
}

impl Select {
    pub(super) fn all_options(&self) -> Vec<&str> {
        if !self.optgroups.is_empty() {
            self.optgroups
                .iter()
                .flat_map(|group| group.options.iter().map(String::as_str))
                .collect()
        } else {
            self.options.iter().map(String::as_str).collect()
        }
    }

    pub(super) fn visible_rows(&self) -> Vec<VisibleRow> {
        let query = self.search_query.to_lowercase();
        let matches = |option: &str| {
            !self.search || query.is_empty() || option.to_lowercase().contains(&query)
        };

        if self.optgroups.is_empty() {
            return self
                .options
                .iter()
                .enumerate()
                .filter_map(|(index, option)| matches(option).then_some(VisibleRow::Option(index)))
                .collect();
        }

        let mut rows = Vec::new();
        let mut option_index = 0;
        for (group_index, group) in self.optgroups.iter().enumerate() {
            let matching_options = group
                .options
                .iter()
                .enumerate()
                .filter_map(|(index, option)| {
                    matches(option).then_some(VisibleRow::Option(option_index + index))
                })
                .collect::<Vec<_>>();
            if !matching_options.is_empty() {
                rows.push(VisibleRow::Group(group_index));
                rows.extend(matching_options);
            }
            option_index += group.options.len();
        }
        rows
    }

    pub(crate) fn visible_option_indices(&self) -> Vec<usize> {
        self.visible_rows()
            .into_iter()
            .filter_map(|row| match row {
                VisibleRow::Option(index) => Some(index),
                VisibleRow::Group(_) => None,
            })
            .collect()
    }

    pub(super) fn option_label(&self, option_index: usize) -> Option<&str> {
        if self.optgroups.is_empty() {
            return self.options.get(option_index).map(String::as_str);
        }

        let mut remaining = option_index;
        for group in &self.optgroups {
            if let Some(option) = group.options.get(remaining) {
                return Some(option);
            }
            remaining = remaining.saturating_sub(group.options.len());
        }
        None
    }

    pub(super) fn group_label(&self, group_index: usize) -> Option<&str> {
        self.optgroups
            .get(group_index)
            .map(|group| group.label.as_str())
    }

    pub(crate) fn dropdown_row_count(&self) -> usize {
        let count = self.visible_rows().len();
        if self.search && !self.search_query.is_empty() {
            count.max(1)
        } else {
            count
        }
    }

    pub(super) fn dropdown_damage_row_count(&self) -> usize {
        let unfiltered = if self.optgroups.is_empty() {
            self.options.len()
        } else {
            self.optgroups
                .iter()
                .map(|group| 1 + group.options.len())
                .sum()
        };
        unfiltered.max(self.dropdown_row_count())
    }

    pub(crate) fn dropdown_viewport_height(&self, row_count: usize) -> f32 {
        (row_count as f32 * DROPDOWN_ROW_HEIGHT).min(super::MAX_DROPDOWN_VIEWPORT_HEIGHT)
    }

    pub(crate) fn dropdown_row_at_y(&self, pos_y: f32) -> Option<usize> {
        if pos_y <= DROPDOWN_TRIGGER_HEIGHT {
            return None;
        }
        let local_y = pos_y - DROPDOWN_TRIGGER_HEIGHT + self.dropdown_scroll.scroll_offset();
        if local_y < 0.0 {
            return None;
        }
        let index = (local_y / DROPDOWN_ROW_HEIGHT) as usize;
        (index < self.dropdown_row_count()).then_some(index)
    }

    pub(super) fn flat_row_option_index(&self, flat_index: usize) -> Option<usize> {
        match self.visible_rows().get(flat_index) {
            Some(VisibleRow::Option(index)) => Some(*index),
            Some(VisibleRow::Group(_)) | None => None,
        }
    }
}

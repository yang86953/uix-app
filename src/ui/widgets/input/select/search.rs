use super::Select;
use std::borrow::Cow;

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
            // 状态绑定只读取稳定值，不依赖可变的显示文案。
            self.options
                .iter()
                .map(|option| option.value.as_str())
                .collect()
        }
    }

    pub(super) fn visible_rows(&self) -> Vec<VisibleRow> {
        if self.loading {
            return Vec::new();
        }
        let capacity = if self.optgroups.is_empty() {
            self.options.len()
        } else {
            self.optgroups
                .iter()
                .map(|group| 1 + group.options.len())
                .sum()
        };
        let mut rows = Vec::with_capacity(capacity);
        self.for_each_visible_row(|row| rows.push(row));
        rows
    }

    // 依声明顺序遍历当前可见行，不物化中间集合。
    pub(super) fn for_each_visible_row(&self, mut visit: impl FnMut(VisibleRow)) {
        if self.loading {
            return;
        }

        let query = self.normalized_search_query();
        if self.optgroups.is_empty() {
            for (index, option) in self.options.iter().enumerate() {
                if self.label_matches_search(&option.label, query.as_ref()) {
                    visit(VisibleRow::Option(index));
                }
            }
            return;
        }

        let mut option_index = 0_usize;
        for (group_index, group) in self.optgroups.iter().enumerate() {
            let has_matching_option = group
                .options
                .iter()
                .any(|option| self.label_matches_search(option, query.as_ref()));
            if has_matching_option {
                visit(VisibleRow::Group(group_index));
                for (index, option) in group.options.iter().enumerate() {
                    if self.label_matches_search(option, query.as_ref()) {
                        visit(VisibleRow::Option(option_index + index));
                    }
                }
            }
            option_index += group.options.len();
        }
    }

    // 计算可见行数而不创建仅供读取的临时数组。
    pub(super) fn visible_row_count(&self) -> usize {
        let mut count = 0_usize;
        self.for_each_visible_row(|_| count += 1);
        count
    }

    // 搜索词已稳定或仅含 ASCII 时保持借用，避免热路径临时字符串。
    fn normalized_search_query(&self) -> Cow<'_, str> {
        let query = self.search_query.as_str();
        if query.is_ascii() || Self::search_lowercase_is_identity(query) {
            Cow::Borrowed(query)
        } else {
            Cow::Owned(query.to_lowercase())
        }
    }

    // 判定字符串的小写映射是否保持逐字符不变。
    fn search_lowercase_is_identity(value: &str) -> bool {
        value.chars().all(|ch| {
            let mut lowercase = ch.to_lowercase();
            lowercase.next() == Some(ch) && lowercase.next().is_none()
        })
    }

    // 保持既有 Unicode 小写包含语义，并为 ASCII 搜索提供零分配路径。
    fn contains_normalized_search(value: &str, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        if query.is_ascii() {
            let query = query.as_bytes();
            let contains_ascii_query = |candidate: &str| {
                candidate
                    .as_bytes()
                    .windows(query.len())
                    .any(|window| window.eq_ignore_ascii_case(query))
            };
            if value.is_ascii() || Self::search_lowercase_is_identity(value) {
                return contains_ascii_query(value);
            }
            return contains_ascii_query(&value.to_lowercase());
        }
        if Self::search_lowercase_is_identity(value) {
            value.contains(query)
        } else {
            value.to_lowercase().contains(query)
        }
    }

    // 搜索只匹配用户可见文案，不读取内部稳定值。
    fn label_matches_search(&self, label: &str, query: &str) -> bool {
        !self.search || Self::contains_normalized_search(label, query)
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
            // 平铺结构始终返回面向用户的显示文案。
            return self
                .options
                .get(option_index)
                .map(|option| option.label.as_str());
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
        self.visible_row_count().max(1)
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
        (row_count as f32 * self.visual.layout.row_height)
            .min(self.visual.layout.max_dropdown_height)
    }

    pub(crate) fn dropdown_row_at_y(&self, pos_y: f32) -> Option<usize> {
        let popup = self.dropdown_rect.get();
        let visible_y = pos_y - popup.y;
        if visible_y < 0.0 || visible_y >= popup.h {
            return None;
        }
        let content_y = visible_y + self.dropdown_scroll.scroll_offset();
        let index = (content_y / self.visual.layout.row_height) as usize;
        (index < self.dropdown_row_count()).then_some(index)
    }

    pub(super) fn flat_row_option_index(&self, flat_index: usize) -> Option<usize> {
        let mut current = 0_usize;
        let mut option_index = None;
        self.for_each_visible_row(|row| {
            if current == flat_index {
                option_index = match row {
                    VisibleRow::Option(index) => Some(index),
                    VisibleRow::Group(_) => None,
                };
            }
            current += 1;
        });
        option_index
    }
}

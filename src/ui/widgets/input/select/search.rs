use super::Select;
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VisibleRow {
    Group(usize),
    Option(usize),
}

/// 可见行计数的稳态缓存值：以精确查询快照作键。
///
/// 失效契约：事件路径修改 `search_query` 由快照比对自然捕获；`sync_from`
/// 整体替换 options/optgroups/loading/search 时显式置空。构造期链式设置
/// 发生在树布局首次读取之前，不存在已填充后被覆盖的路径。
#[derive(Debug)]
pub(super) struct VisibleRowCountEntry {
    query: String,
    count: usize,
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

    // 计算可见行数而不创建仅供读取的临时数组；稳态帧经查询快照直接快返。
    pub(super) fn visible_row_count(&self) -> usize {
        let query = self.search_query.as_str();
        if let Some(entry) = self.visible_row_count_cache.borrow().as_ref()
            && entry.query == query
        {
            return entry.count;
        }
        let mut count = 0_usize;
        self.for_each_visible_row(|_| count += 1);
        *self.visible_row_count_cache.borrow_mut() = Some(VisibleRowCountEntry {
            query: query.to_owned(),
            count,
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::EventResult;
    use crate::ui::widget_runtime::traits::EventHandler;
    use crate::ui::widgets::input::select::SelectOption;

    fn searchable_select(option_count: usize) -> Select {
        let mut select = Select::searchable();
        select.options = (0..option_count)
            .map(|index| SelectOption::new(format!("Option {index}"), format!("{index}")))
            .collect();
        select.open();
        select
    }

    fn type_query(select: &mut Select, text: &str) {
        assert_eq!(
            select.on_event(&crate::ui::SystemEvent::TextInput {
                text: text.to_owned()
            }),
            EventResult::Handled
        );
    }

    // 查询保持稳定时重复读取必须返回同一计数。
    #[test]
    fn repeated_count_reads_stay_stable() {
        let mut select = searchable_select(8);
        type_query(&mut select, "OPTION 3");
        assert_eq!(select.visible_row_count(), 1);
        assert_eq!(select.visible_row_count(), 1);
    }

    // 事件路径修改搜索词后，计数必须反映新查询而不是旧快照。
    #[test]
    fn query_edits_invalidate_cached_count() {
        let mut select = searchable_select(8);
        type_query(&mut select, "OPTION 3");
        assert_eq!(select.visible_row_count(), 1);
        // 回格删除尾部数字后查询回到词级前缀，可见集合重新扩大到全体。
        select.search_query.pop();
        assert_eq!(select.visible_row_count(), 8);
    }

    // sync_from 整体替换行集合输入后必须重新计数。
    #[test]
    fn sync_replacement_invalidate_cached_count() {
        let mut mounted = searchable_select(4);
        assert_eq!(mounted.visible_row_count(), 4);
        let replacement = searchable_select(9);
        mounted.sync_from(replacement);
        assert_eq!(mounted.visible_row_count(), 9);
    }

    // loading 切换属于行集合输入变化，缓存不得继续提供旧计数。
    #[test]
    fn loading_transition_invalidate_cached_count() {
        let mut mounted = searchable_select(6);
        assert_eq!(mounted.visible_row_count(), 6);
        let mut loading_next = searchable_select(6);
        loading_next.loading = true;
        mounted.sync_from(loading_next);
        assert_eq!(mounted.visible_row_count(), 0);
    }

    // 尺寸观测只服务于性能记录登记，忽略入口不参与常规验证。
    #[test]
    #[ignore = "手工尺寸观测"]
    fn probe_select_struct_size() {
        eprintln!("SELECT_SIZE={}", std::mem::size_of::<Select>());
    }
}

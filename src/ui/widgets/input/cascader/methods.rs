use crate::core::{Point, Rect, Size};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::SnapshotFields;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use super::{
    Cascader, CascaderOption, CascaderValue, ITEM_HEIGHT, POPUP_HEIGHT, TRIGGER_HEIGHT,
    byte_index_for_char, cascader_column_width, cascader_popup_rect, collect_search_results,
    first_enabled_index, next_enabled_index, point_in_half_open_rect,
};

impl Cascader {
    pub(crate) fn intrinsic_size(&self) -> Size {
        Size::new(120.0, 32.0)
    }

    pub fn new(options: Vec<CascaderOption>, placeholder: impl Into<String>) -> Self {
        Self {
            options: options.clone(),
            selected: CascaderValue {
                labels: vec![],
                values: vec![],
            },
            current_levels: vec![options],
            level_indices: vec![0],
            scroll_offsets: vec![0.0],
            hovered_option: None,
            open: false,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            loading_children: HashSet::new(),
            loading_phase: 0.0,
            loading_dirty: false,
            searchable: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_index: 0,
            search_scroll_offset: 0.0,
            search_cursor_char: 0,
            search_cursor_rect: Cell::new(Rect::zero()),
            search_glyph_xs: RefCell::new(vec![0.0]),
            search_text_scroll_x: Cell::new(0.0),
            placeholder: placeholder.into(),
            focused: false,
            last_frame: Cell::new(None),
            pending_change: RefCell::new(None),
        }
    }

    pub(crate) fn init_levels(&mut self) {
        self.current_levels = vec![self.options.clone()];
        self.level_indices = vec![first_enabled_index(&self.options).unwrap_or(0)];
        self.scroll_offsets = vec![0.0];
        self.hovered_option = None;
    }

    pub fn select_option(&mut self, level: usize, index: usize) {
        if level >= self.current_levels.len() {
            return;
        }
        let opt = match self.current_levels[level].get(index) {
            Some(o) => o.clone(),
            None => return,
        };
        if opt.disabled {
            return;
        }

        self.level_indices.truncate(level + 1);
        self.scroll_offsets.truncate(level + 1);
        if let Some(highlighted) = self.level_indices.get_mut(level) {
            *highlighted = index;
        } else {
            self.level_indices.push(index);
        }
        self.selected.labels.truncate(level);
        self.selected.values.truncate(level);
        self.selected.labels.push(opt.label.clone());
        self.selected.values.push(opt.value.clone());

        self.current_levels.truncate(level + 1);
        if self.loading_children.contains(&opt.value) {
            return;
        }
        if !opt.children.is_empty() {
            let child_highlight = first_enabled_index(&opt.children).unwrap_or(0);
            self.current_levels.push(opt.children);
            self.level_indices.push(child_highlight);
            self.scroll_offsets.push(0.0);
        } else {
            self.pending_change
                .replace(Some(self.selected.values.join("/")));
            self.close();
        }
    }

    pub(crate) fn activate_highlight(&mut self) {
        let Some(level) = self.current_levels.len().checked_sub(1) else {
            return;
        };
        let Some(index) = self.level_indices.get(level).copied() else {
            return;
        };
        self.select_option(level, index);
    }

    pub(crate) fn move_highlight(&mut self, forward: bool) {
        let Some(level) = self.current_levels.len().checked_sub(1) else {
            return;
        };
        let Some(options) = self.current_levels.get(level) else {
            return;
        };
        let current = self.level_indices.get(level).copied().unwrap_or(0);
        if let Some(next) = next_enabled_index(options, current, forward) {
            if let Some(highlighted) = self.level_indices.get_mut(level) {
                *highlighted = next;
            }
            self.ensure_highlight_visible(level);
        }
    }

    pub(crate) fn return_to_parent(&mut self) {
        if self.current_levels.len() <= 1 {
            return;
        }
        self.current_levels.pop();
        self.level_indices.pop();
        self.scroll_offsets.pop();
        self.hovered_option = None;
    }

    pub fn selected(&self) -> &CascaderValue {
        &self.selected
    }

    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }

    /// 标记指定 value 的子级正在加载；加载项保留高亮与路径，但不进入或提交。
    pub fn loading_child(mut self, value: impl Into<String>, loading: bool) -> Self {
        let value = value.into();
        if loading {
            self.loading_children.insert(value);
        } else {
            self.loading_children.remove(&value);
            if self.loading_children.is_empty() {
                self.loading_phase = 0.0;
                self.loading_dirty = false;
            }
        }
        self
    }

    /// 启用跨层级路径搜索；查询只过滤候选，提交时仍返回完整 value 路径。
    pub fn searchable(mut self, searchable: bool) -> Self {
        self.searchable = searchable;
        if !searchable {
            self.clear_search();
        }
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_present(&self) -> bool {
        self.open || self.closing
    }

    pub fn open(&mut self) {
        self.init_levels();
        self.refresh_search_results();
        self.open = true;
        self.closing = false;
        self.hovered_option = None;
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    pub fn close(&mut self) {
        if !self.is_present() {
            self.open = false;
            self.closing = false;
            self.transition_dirty = false;
            self.clear_search();
            return;
        }

        self.open = false;
        self.closing = true;
        self.hovered_option = None;
        self.clear_search();
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        let mut loading_children = self.loading_children.iter().cloned().collect::<Vec<_>>();
        loading_children.sort();
        SnapshotFields::Cascader {
            options: self.options.clone(),
            placeholder: self.placeholder.clone(),
            selected_labels: self.selected.labels.clone(),
            selected_values: self.selected.values.clone(),
            open: self.open,
            loading_children,
            searchable: self.searchable,
            search_query: self.search_query.clone(),
            search_results: self
                .search_results
                .iter()
                .map(|result| result.value.clone())
                .collect(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let options_changed = self.options != next.options;
        let searchable_changed = self.searchable != next.searchable;
        let loading_changed = self.loading_children != next.loading_children;
        self.options = next.options;
        self.placeholder = next.placeholder;
        self.loading_children = next.loading_children;
        self.searchable = next.searchable;
        if self.loading_children.is_empty() {
            self.loading_phase = 0.0;
            self.loading_dirty = false;
        }
        if self.is_present() && options_changed {
            self.init_levels();
        }
        if !self.searchable {
            self.clear_search();
        } else if self.is_present() && (options_changed || searchable_changed || loading_changed) {
            self.refresh_search_results();
        }
    }

    pub(crate) fn has_visible_loading_child(&self) -> bool {
        if self.search_active() {
            return self.search_results.iter().any(|result| result.loading);
        }
        self.current_levels
            .iter()
            .flatten()
            .any(|option| self.loading_children.contains(&option.value))
    }

    pub(crate) fn search_active(&self) -> bool {
        self.searchable && !self.search_query.is_empty()
    }

    pub(crate) fn visible_column_count(&self) -> usize {
        if self.search_active() {
            1
        } else {
            self.current_levels.len()
        }
    }

    pub(crate) fn damage_column_count(&self) -> usize {
        self.visible_column_count().max(self.current_levels.len())
    }

    pub(crate) fn refresh_search_results(&mut self) {
        self.search_results.clear();
        self.search_index = 0;
        self.search_scroll_offset = 0.0;
        self.hovered_option = None;
        if !self.search_active() {
            return;
        }

        let query = self.search_query.to_lowercase();
        let mut path = CascaderValue {
            labels: Vec::new(),
            values: Vec::new(),
        };
        collect_search_results(
            &self.options,
            &self.loading_children,
            &query,
            &mut path,
            false,
            &mut self.search_results,
        );
        self.search_index = self
            .search_results
            .iter()
            .position(|result| !result.disabled)
            .unwrap_or(0);
    }

    pub(crate) fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_results.clear();
        self.search_index = 0;
        self.search_scroll_offset = 0.0;
        self.search_cursor_char = 0;
        self.search_glyph_xs.replace(vec![0.0]);
        self.search_text_scroll_x.set(0.0);
        self.search_cursor_rect.set(Rect::zero());
    }

    pub(crate) fn select_search_result(&mut self, index: usize) -> bool {
        let Some(result) = self.search_results.get(index).cloned() else {
            return false;
        };
        if result.disabled {
            return false;
        }

        self.search_index = index;
        self.selected = result.value;
        if result.loading {
            return true;
        }
        self.pending_change
            .replace(Some(self.selected.values.join("/")));
        self.close();
        true
    }

    pub(crate) fn move_search_highlight(&mut self, forward: bool) {
        let len = self.search_results.len();
        if len == 0 {
            return;
        }
        for step in 1..=len {
            let index = if forward {
                (self.search_index + step) % len
            } else {
                (self.search_index + len - (step % len)) % len
            };
            if !self.search_results[index].disabled {
                self.search_index = index;
                self.ensure_search_highlight_visible();
                return;
            }
        }
    }

    pub(crate) fn ensure_search_highlight_visible(&mut self) {
        if self.search_index >= self.search_results.len() {
            return;
        }
        let row_top = self.search_index as f32 * ITEM_HEIGHT;
        let row_bottom = row_top + ITEM_HEIGHT;
        if row_top < self.search_scroll_offset {
            self.search_scroll_offset = row_top;
        } else if row_bottom > self.search_scroll_offset + POPUP_HEIGHT {
            self.search_scroll_offset = row_bottom - POPUP_HEIGHT;
        }
        let max_scroll = (self.search_results.len() as f32 * ITEM_HEIGHT - POPUP_HEIGHT).max(0.0);
        self.search_scroll_offset = self.search_scroll_offset.clamp(0.0, max_scroll);
    }

    pub(crate) fn scroll_search_results(&mut self, delta: f32) -> bool {
        if !delta.is_finite() {
            return false;
        }
        let max_scroll = (self.search_results.len() as f32 * ITEM_HEIGHT - POPUP_HEIGHT).max(0.0);
        let next = (self.search_scroll_offset + delta).clamp(0.0, max_scroll);
        if (next - self.search_scroll_offset).abs() <= f32::EPSILON {
            false
        } else {
            self.search_scroll_offset = next;
            self.hovered_option = None;
            true
        }
    }

    pub(crate) fn search_result_at(&self, frame: Rect, pos: Point) -> Option<usize> {
        let popup = cascader_popup_rect(frame, 1);
        if !point_in_half_open_rect(popup, pos) {
            return None;
        }
        let index = ((pos.y - popup.y + self.search_scroll_offset) / ITEM_HEIGHT).floor() as usize;
        (index < self.search_results.len()).then_some(index)
    }

    pub(crate) fn set_search_cursor_from_x(&mut self, x: f32) {
        let glyph_xs = self.search_glyph_xs.borrow();
        if glyph_xs.len() != self.search_query.chars().count() + 1 {
            self.search_cursor_char = self.search_query.chars().count();
            return;
        }
        let scale = (self.interaction_frame().h / TRIGGER_HEIGHT).clamp(0.0, 1.0);
        let target = (x - 12.0 * scale + self.search_text_scroll_x.get()).max(0.0);
        let mut index = glyph_xs.len().saturating_sub(1);
        for candidate in 0..glyph_xs.len().saturating_sub(1) {
            let midpoint = (glyph_xs[candidate] + glyph_xs[candidate + 1]) * 0.5;
            if target < midpoint {
                index = candidate;
                break;
            }
        }
        self.search_cursor_char = index;
    }

    pub(crate) fn insert_search_text(&mut self, text: &str) -> bool {
        if text.is_empty() || text.chars().any(char::is_control) {
            return false;
        }
        let byte_index = byte_index_for_char(&self.search_query, self.search_cursor_char);
        self.search_query.insert_str(byte_index, text);
        self.search_cursor_char += text.chars().count();
        if self.open {
            self.refresh_search_results();
        } else {
            self.open();
        }
        true
    }

    pub(crate) fn delete_previous_search_char(&mut self) -> bool {
        if self.search_cursor_char == 0 || self.search_query.is_empty() {
            return false;
        }
        let end = byte_index_for_char(&self.search_query, self.search_cursor_char);
        let start = byte_index_for_char(&self.search_query, self.search_cursor_char - 1);
        self.search_query.replace_range(start..end, "");
        self.search_cursor_char -= 1;
        self.refresh_search_results();
        true
    }

    pub(crate) fn delete_next_search_char(&mut self) -> bool {
        let char_count = self.search_query.chars().count();
        if self.search_cursor_char >= char_count {
            return false;
        }
        let start = byte_index_for_char(&self.search_query, self.search_cursor_char);
        let end = byte_index_for_char(&self.search_query, self.search_cursor_char + 1);
        self.search_query.replace_range(start..end, "");
        self.refresh_search_results();
        true
    }

    pub(crate) fn interaction_frame(&self) -> Rect {
        self.last_frame.get().unwrap_or_else(|| {
            Rect::new(0.0, 0.0, self.intrinsic_size().w, self.intrinsic_size().h)
        })
    }

    pub(crate) fn option_at(&self, frame: Rect, pos: Point) -> Option<(usize, usize)> {
        let popup = cascader_popup_rect(frame, self.current_levels.len());
        if !point_in_half_open_rect(popup, pos) {
            return None;
        }
        let column_width = cascader_column_width(frame);
        let level = ((pos.x - popup.x) / column_width).floor() as usize;
        let options = self.current_levels.get(level)?;
        let scroll = self.scroll_offsets.get(level).copied().unwrap_or(0.0);
        let index = ((pos.y - popup.y + scroll) / ITEM_HEIGHT).floor() as usize;
        (index < options.len()).then_some((level, index))
    }

    pub(crate) fn scroll_level(&mut self, level: usize, delta: f32) -> bool {
        if !delta.is_finite() {
            return false;
        }
        let Some(options) = self.current_levels.get(level) else {
            return false;
        };
        let Some(offset) = self.scroll_offsets.get_mut(level) else {
            return false;
        };
        let max_scroll = (options.len() as f32 * ITEM_HEIGHT - POPUP_HEIGHT).max(0.0);
        let next = (*offset + delta).clamp(0.0, max_scroll);
        if (next - *offset).abs() <= f32::EPSILON {
            false
        } else {
            *offset = next;
            self.hovered_option = None;
            true
        }
    }

    pub(crate) fn ensure_highlight_visible(&mut self, level: usize) {
        let Some(index) = self.level_indices.get(level).copied() else {
            return;
        };
        let Some(options) = self.current_levels.get(level) else {
            return;
        };
        let Some(offset) = self.scroll_offsets.get_mut(level) else {
            return;
        };
        let row_top = index as f32 * ITEM_HEIGHT;
        let row_bottom = row_top + ITEM_HEIGHT;
        if row_top < *offset {
            *offset = row_top;
        } else if row_bottom > *offset + POPUP_HEIGHT {
            *offset = row_bottom - POPUP_HEIGHT;
        }
        let max_scroll = (options.len() as f32 * ITEM_HEIGHT - POPUP_HEIGHT).max(0.0);
        *offset = (*offset).clamp(0.0, max_scroll);
    }

    pub(crate) fn move_to_edge(&mut self, first: bool) {
        let Some(level) = self.current_levels.len().checked_sub(1) else {
            return;
        };
        let Some(options) = self.current_levels.get(level) else {
            return;
        };
        let next = if first {
            first_enabled_index(options)
        } else {
            options.iter().rposition(|option| !option.disabled)
        };
        if let Some(next) = next {
            if let Some(highlighted) = self.level_indices.get_mut(level) {
                *highlighted = next;
            }
            self.ensure_highlight_visible(level);
        }
    }
}


use crate::core::Rect;
use crate::native::windowing::input::ControlSize;
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::reactive::state::State;
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::SnapshotFields;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use super::{
    OptGroup, Select, SelectOptionGroup, SelectOptionView, SelectValue, SelectValueBinding,
};

const DROPDOWN_ROW_HEIGHT: f32 = 28.0;

impl Default for Select {
    fn default() -> Self {
        Self::new()
    }
}

impl Select {
    pub fn new() -> Self {
        let config = crate::ui::component::config::use_config();
        let control_height = crate::ui::component::config::control_height(config.size);
        Self {
            options: Vec::new(),
            optgroups: Vec::new(),
            selected: 0,
            selected_multi: Vec::new(),
            value_binding: None,
            open: false,
            disabled: config.disabled,
            loading: false,
            loading_phase: 0.0,
            loading_dirty: false,
            select_size: config.size,
            hovered: false,
            focused: false,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            placeholder: String::new(),
            hovered_option: None,
            highlighted_option: None,
            pending_change: RefCell::new(None),
            multiple: false,
            search: config.overrides.select.allow_search.unwrap_or(false),
            search_query: String::new(),
            custom_option_views: false,
            materialized_custom_options: RefCell::new(Vec::new()),
            search_cursor_rect: Cell::new(Rect::zero()),
            control_rect: Cell::new(Rect::new(0.0, 0.0, 120.0, control_height)),
            dropdown_rect: Cell::new(Rect::zero()),
            // 新选择器尚未接收布局或绘制表面。
            surface_rect: Cell::new(None),
            multi_remove_rects: RefCell::new(Vec::new()),
            dropdown_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
        }
    }

    /// 创建多选选择器。
    pub fn multiple() -> Self {
        let mut select = Self::new();
        select.multiple = true;
        select
    }

    /// 创建可搜索的单选选择器。
    pub fn searchable() -> Self {
        let mut select = Self::new();
        select.search = true;
        select
    }

    pub fn options<I, S>(mut self, opts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.options = opts
            .into_iter()
            .map(|option| option.as_ref().to_owned())
            .collect();
        self.sync_bound_selection();
        self
    }

    /// Render each dropdown option with an arbitrary View while preserving Select interaction.
    pub fn render_option<F, V>(mut self, renderer: F) -> SelectOptionView
    where
        F: Fn(&str) -> V + 'static,
        V: crate::ui::view::View,
    {
        self.custom_option_views = true;
        SelectOptionView {
            select: self,
            renderer: Box::new(move |option| crate::ui::view::View::build(renderer(option))),
        }
    }

    pub fn optgroups(mut self, groups: Vec<OptGroup>) -> Self {
        self.optgroups = groups;
        self.sync_bound_selection();
        self
    }

    /// 设置分组选项；组标题不可选择，组内选项按声明顺序形成值索引。
    pub fn option_groups<I>(mut self, groups: I) -> Self
    where
        I: IntoIterator<Item = SelectOptionGroup>,
    {
        self.optgroups = groups.into_iter().map(OptGroup::from).collect();
        self.sync_bound_selection();
        self
    }

    /// 下拉展开时用加载旋转器替代候选项，并暂停选项提交。
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        if loading {
            self.hovered_option = None;
            self.highlighted_option = None;
        } else {
            self.loading_phase = 0.0;
            self.loading_dirty = false;
        }
        self
    }

    /// 设置非受控选择器的初始索引。
    pub fn default_selected(mut self, idx: usize) -> Self {
        self.value_binding = None;
        self.selected = idx;
        self
    }

    /// 将选择值绑定到外部 State；支持 `String` 单选与 `HashSet<String>` 多选。
    pub fn value<T: SelectValue>(mut self, state: &State<T>) -> Self {
        self.value_binding = Some(SelectValueBinding::new(state));
        self.multiple = T::MULTIPLE;
        self.sync_bound_selection();
        self
    }

    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.select_size = size;
        let control = self.control_rect.get();
        self.control_rect.set(Rect::new(
            control.x,
            control.y,
            control.w,
            self.control_height(),
        ));
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_present(&self) -> bool {
        self.open || self.closing
    }

    pub fn current_value(&self) -> Option<String> {
        self.all_options()
            .get(self.selected)
            .map(|value| (*value).to_owned())
    }

    pub fn current_values(&self) -> HashSet<String> {
        let options = self.all_options();
        self.selected_multi
            .iter()
            .filter_map(|index| options.get(*index))
            .map(|value| (*value).to_owned())
            .collect()
    }

    // 测试目标保留多选移除区域观测入口，供选择器交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn first_multi_remove_rect(&self) -> Option<Rect> {
        self.multi_remove_rects
            .borrow()
            .first()
            .map(|(_, rect)| *rect)
    }

    pub fn open(&mut self) {
        if self.closing {
            self.search_query.clear();
            self.hovered_option = None;
        }
        self.open = true;
        self.closing = false;
        self.dropdown_scroll.set_scroll_offset(0.0);
        let control = self.control_rect.get();
        let row_count = self.dropdown_row_count();
        self.dropdown_rect.set(Rect::new(
            0.0,
            control.h,
            control.w,
            self.dropdown_viewport_height(row_count),
        ));
        let visible = self.visible_option_indices();
        self.highlighted_option = visible
            .contains(&self.selected)
            .then_some(self.selected)
            .or_else(|| visible.first().copied());
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    pub fn close(&mut self) {
        if !self.is_present() {
            self.open = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }

        self.open = false;
        self.closing = true;
        self.dropdown_scroll.set_scroll_offset(0.0);
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Select {
            options: self.options.clone(),
            optgroups: self.optgroups.clone(),
            selected: self.selected,
            selected_multi: self.selected_multi.clone(),
            open: self.open,
            disabled: self.disabled,
            loading: self.loading,
            placeholder: self.placeholder.clone(),
            multiple: self.multiple,
            search: self.search,
            search_query: self.search_query.clone(),
            custom_options: self.custom_option_views,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_selection = next
            .value_binding
            .as_ref()
            .map(|_| (next.selected, next.selected_multi.clone()));
        self.options = next.options;
        self.optgroups = next.optgroups;
        self.value_binding = next.value_binding;
        self.disabled = next.disabled;
        self.loading = next.loading;
        if self.loading {
            self.hovered_option = None;
        } else {
            self.loading_phase = 0.0;
            self.loading_dirty = false;
        }
        self.select_size = next.select_size;
        self.placeholder = next.placeholder;
        self.multiple = next.multiple;
        self.search = next.search;
        self.custom_option_views = next.custom_option_views;
        if !self.custom_option_views {
            self.materialized_custom_options.borrow_mut().clear();
        }
        if !self.search {
            self.search_query.clear();
        }
        if let Some((selected, selected_multi)) = controlled_selection {
            self.selected = selected;
            self.selected_multi = selected_multi;
        }
        let row_count = self.dropdown_row_count();
        self.dropdown_scroll.clamp_to_content(
            row_count,
            DROPDOWN_ROW_HEIGHT,
            self.dropdown_viewport_height(row_count),
        );
        let popup = self.dropdown_rect.get();
        let popup_height = self.dropdown_viewport_height(row_count);
        self.dropdown_rect.set(Rect::new(
            0.0,
            if popup.y < 0.0 {
                -popup_height
            } else {
                self.control_rect.get().h
            },
            self.control_rect.get().w,
            popup_height,
        ));
        let visible = self.visible_option_indices();
        if self
            .highlighted_option
            .is_some_and(|index| !visible.contains(&index))
        {
            self.highlighted_option = visible.first().copied();
        }
    }
}

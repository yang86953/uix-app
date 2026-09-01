use crate::core::Rect;
use crate::platform::windowing::ControlSize;
use crate::ui::SnapshotFields;
use crate::ui::animation::{TransitionPlayer, presets};
use crate::ui::reactive::state::State;
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use super::{
    OptGroup, SELECT_VISUAL_REF, Select, SelectOption, SelectOptionGroup, SelectOptionView,
    SelectValue, SelectValueBinding, SelectValueMode,
};

impl Default for Select {
    fn default() -> Self {
        Self::new()
    }
}

impl Select {
    /// 创建继承全局尺寸、禁用和搜索配置的单选选择器。
    pub fn new() -> Self {
        let config = crate::ui::widget_runtime::config::use_config();
        let visual = SELECT_VISUAL_REF;
        let control_height = visual.layout.control_height(config.size);
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
            view_width: None,
            view_height: None,
            view_flex_grow: 0.0,
            view_flex_shrink: 1.0,
            intrinsic_width: Cell::new(None),
            visible_row_count_cache: RefCell::new(None),
            search_cursor_rect: Cell::new(Rect::zero()),
            control_rect: Cell::new(Rect::new(
                0.0,
                0.0,
                visual.layout.natural_min_width,
                control_height,
            )),
            dropdown_rect: Cell::new(Rect::zero()),
            // 新选择器尚未接收布局或绘制表面。
            surface_rect: Cell::new(None),
            multi_remove_rects: RefCell::new(Vec::new()),
            dropdown_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            visual,
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

    /// 按声明式布尔值启用或关闭搜索能力。
    pub fn searchable_enabled(mut self, enabled: bool) -> Self {
        // 保存调用侧解析后的搜索开关。
        self.search = enabled;
        // 返回更新后的流式构造器。
        self
    }

    /// 设置显示文案与稳定值相同的未分组选项。
    pub fn options<I, S>(mut self, opts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.options = opts
            .into_iter()
            // 兼容既有字符串选项：显示文案与稳定值保持一致。
            .map(|option| SelectOption::new(option.as_ref(), option.as_ref()))
            .collect();
        self.intrinsic_width.set(None);
        self.sync_bound_selection();
        self
    }

    /// 设置具有独立显示文案与稳定值的选择项。
    pub fn select_options<I>(mut self, options: I) -> Self
    where
        // 接受数组、向量和其他任意可迭代选项集合。
        I: IntoIterator<Item = SelectOption>,
    {
        // 保留调用方声明顺序作为稳定的交互索引顺序。
        self.options = options.into_iter().collect();
        self.intrinsic_width.set(None);
        // 新选项集合写入后立即同步外部绑定的当前值。
        self.sync_bound_selection();
        // 返回更新后的流式构造器。
        self
    }

    /// 使用任意 View 渲染下拉选项，同时保留选择器交互。
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

    /// 设置兼容格式的分组选项，并同步外部绑定值。
    pub fn optgroups(mut self, groups: Vec<OptGroup>) -> Self {
        self.optgroups = groups;
        self.intrinsic_width.set(None);
        self.sync_bound_selection();
        self
    }

    /// 设置分组选项；组标题不可选择，组内选项按声明顺序形成值索引。
    pub fn option_groups<I>(mut self, groups: I) -> Self
    where
        I: IntoIterator<Item = SelectOptionGroup>,
    {
        self.optgroups = groups.into_iter().map(OptGroup::from).collect();
        self.intrinsic_width.set(None);
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

    /// 绑定状态并在编译期验证声明的单选或多选模式。
    pub fn value_mode<const MULTIPLE: bool, T>(self, state: &State<T>) -> Self
    where
        // 只有与声明模式匹配的公开状态类型才能进入绑定入口。
        T: SelectValueMode<MULTIPLE>,
    {
        // 复用统一状态绑定和首次同步生命周期。
        self.value(state)
    }

    /// 设置未选择内容时显示的占位文本。
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self.intrinsic_width.set(None);
        self
    }

    /// 设置选择器是否禁用。
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }

    /// 设置控件尺寸，并立即同步控件区域高度。
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

    /// 返回下拉列表是否处于打开阶段。
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// 返回下拉列表是否打开或仍在执行关闭过渡。
    pub fn is_present(&self) -> bool {
        self.open || self.closing
    }

    /// 返回当前单选项的稳定值；索引无效时返回 `None`。
    pub fn current_value(&self) -> Option<String> {
        self.all_options()
            .get(self.selected)
            .map(|value| (*value).to_owned())
    }

    /// 返回当前单选项面向用户的显示文案；索引无效时返回 `None`。
    pub fn current_label(&self) -> Option<&str> {
        self.option_label(self.selected)
    }

    /// 返回当前全部多选项的稳定值集合。
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

    /// 打开下拉列表、重置滚动位置并建立初始高亮项。
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

    /// 关闭下拉列表，并在列表存在时启动离场过渡。
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
            // 快照继续暴露实际显示文案，保持既有观测契约。
            options: self
                .options
                .iter()
                .map(|option| option.label.clone())
                .collect(),
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
        let intrinsic_width_changed = !std::ptr::eq(self.visual, next.visual)
            || self.options != next.options
            || self.optgroups != next.optgroups
            || self.placeholder != next.placeholder;
        if intrinsic_width_changed {
            self.intrinsic_width.set(None);
        }
        // 行集合相关输入被整体替换时同步作废可见行计数缓存。
        let visible_rows_inputs_changed = self.options != next.options
            || self.optgroups != next.optgroups
            || self.loading != next.loading
            || self.search != next.search;
        // UIX 视觉引用变化时先更新几何事实并清除依赖旧视觉的缓存。
        if !std::ptr::eq(self.visual, next.visual) {
            self.visual = next.visual;
            self.dropdown_rect.set(Rect::zero());
            self.surface_rect.set(None);
            self.multi_remove_rects.borrow_mut().clear();
            self.invalidate_custom_option_materialization();
        }
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
        // 声明树重建时同步最新叶控件布局覆盖，避免固有宽度重新覆盖直接尺寸。
        self.view_width = next.view_width;
        self.view_height = next.view_height;
        self.view_flex_grow = next.view_flex_grow;
        self.view_flex_shrink = next.view_flex_shrink;
        if !self.custom_option_views {
            self.materialized_custom_options.borrow_mut().clear();
        }
        if !self.search {
            self.search_query.clear();
        }
        if visible_rows_inputs_changed {
            *self.visible_row_count_cache.borrow_mut() = None;
        }
        if let Some((selected, selected_multi)) = controlled_selection {
            self.selected = selected;
            self.selected_multi = selected_multi;
        }
        let row_count = self.dropdown_row_count();
        self.dropdown_scroll.clamp_to_content(
            row_count,
            self.visual.layout.row_height,
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

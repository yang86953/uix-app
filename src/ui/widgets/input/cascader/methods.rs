use crate::core::{Point, Rect, Size};
use crate::ui::SnapshotFields;
use crate::ui::animation::{AnimationConfig, TransitionPlayer};
// 引入公开双向状态句柄。
use crate::ui::reactive::state::State;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use super::{
    CASCADER_VISUAL_REF, Cascader, CascaderOption, CascaderPopupGeometry, CascaderValue,
    byte_index_for_char, cascader_fallback_surface, collect_search_results, first_enabled_index,
    local_cascader_popup_geometry, next_enabled_index, normalize_cascader_rect,
    point_in_half_open_rect, resolve_cascader_popup_geometry,
};

impl Cascader {
    pub(crate) fn intrinsic_size(&self) -> Size {
        Size::new(
            self.visual.defaults.intrinsic_width,
            self.visual.layout.trigger_height,
        )
    }

    /// 创建持有候选树、空选择路径且未展开的级联选择器。
    pub fn new(options: Vec<CascaderOption>, placeholder: impl Into<String>) -> Self {
        Self {
            options: options.clone(),
            selected: CascaderValue {
                labels: vec![],
                values: vec![],
            },
            // 未调用 value 前保持非受控模式。
            value_binding: None,
            current_levels: vec![options],
            level_indices: vec![0],
            scroll_offsets: vec![0.0],
            hovered_option: None,
            open: false,
            transition: TransitionPlayer::new(AnimationConfig::fade_in(
                CASCADER_VISUAL_REF.motion.enter_duration,
            )),
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
            // 首次表面解析前弹层缓存为空。
            popup_rect: Cell::new(Rect::zero()),
            // 首次表面解析前没有实际列宽。
            popup_column_width: Cell::new(0.0),
            // 零列标记尚未生成有效弹层缓存。
            popup_column_count: Cell::new(0),
            // 首次登记或绘制前尚未取得当前逻辑表面。
            surface_rect: Cell::new(None),
            pending_change: RefCell::new(None),
            // 默认实例直接引用 UIX 生成的唯一静态视觉表。
            visual: CASCADER_VISUAL_REF,
        }
    }

    /// 将选中路径双向绑定到外部状态。
    pub fn value(mut self, state: &State<CascaderValue>) -> Self {
        // 首次构造直接读取业务状态作为显示真值。
        self.selected = state.get();
        // 保存句柄供叶选项提交时回写。
        self.value_binding = Some(state.clone());
        // 返回更新后的流式构造器。
        self
    }

    pub(crate) fn init_levels(&mut self) {
        self.current_levels = vec![self.options.clone()];
        self.level_indices = vec![first_enabled_index(&self.options).unwrap_or(0)];
        self.scroll_offsets = vec![0.0];
        self.hovered_option = None;
    }

    /// 选择指定层级的可用选项；进入子级，或在叶项处提交完整路径并关闭。
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
            // 只有完整叶路径才提交业务状态。
            self.publish_bound_value();
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

    /// 返回当前逐级选择形成的标签与稳定值路径。
    pub fn selected(&self) -> &CascaderValue {
        &self.selected
    }

    // 将当前完整路径提交给声明式外部状态。
    fn publish_bound_value(&self) {
        // 非受控模式不产生额外副作用。
        let Some(state) = self.value_binding.as_ref() else {
            // 直接返回并保留内部选择行为。
            return;
        };
        // 避免相同路径触发无意义状态版本更新。
        if state.get() != self.selected {
            // 克隆拥有所有权的标签和值路径写入状态。
            state.set(self.selected.clone());
        }
    }

    /// 设置尚未选中路径时显示的占位文本。
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

    /// 返回弹层当前是否处于逻辑展开状态。
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// 返回弹层当前是否仍需呈现，包括退出过渡阶段。
    pub fn is_present(&self) -> bool {
        self.open || self.closing
    }

    /// 展开弹层，并从根级选项重建导航与搜索结果。
    pub fn open(&mut self) {
        self.init_levels();
        self.refresh_search_results();
        self.open = true;
        self.closing = false;
        self.hovered_option = None;
        self.transition =
            TransitionPlayer::new(AnimationConfig::fade_in(self.visual.motion.enter_duration));
        self.transition_dirty = true;
    }

    /// 清除搜索状态并关闭弹层，已呈现时启动退出过渡。
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
        self.transition =
            TransitionPlayer::new(AnimationConfig::fade_out(self.visual.motion.exit_duration));
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
        let visual_changed = !std::ptr::eq(self.visual, next.visual);
        self.options = next.options;
        // 声明式重建时以外部状态快照覆盖内部显示值。
        self.selected = next.selected;
        // 更新下一轮叶路径提交使用的状态句柄。
        self.value_binding = next.value_binding;
        self.placeholder = next.placeholder;
        self.loading_children = next.loading_children;
        self.searchable = next.searchable;
        self.visual = next.visual;
        if visual_changed {
            // 静态视觉几何变化后丢弃上一份弹层与表面缓存。
            self.popup_rect.set(Rect::zero());
            self.popup_column_width.set(0.0);
            self.popup_column_count.set(0);
            self.surface_rect.set(None);
        }
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
        // 搜索结果叶路径与逐列选择使用同一状态提交端口。
        self.publish_bound_value();
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
        let item_height = self.visual.layout.item_height;
        let row_top = self.search_index as f32 * item_height;
        let row_bottom = row_top + item_height;
        // 键盘显露使用受当前表面缩高后的实际视口。
        let viewport_height = self.effective_popup_height();
        if row_top < self.search_scroll_offset {
            self.search_scroll_offset = row_top;
        } else if row_bottom > self.search_scroll_offset + viewport_height {
            self.search_scroll_offset = row_bottom - viewport_height;
        }
        // 最大滚动距离同样使用实际视口高度。
        let max_scroll =
            (self.search_results.len() as f32 * item_height - viewport_height).max(0.0);
        self.search_scroll_offset = self.search_scroll_offset.clamp(0.0, max_scroll);
    }

    pub(crate) fn scroll_search_results(&mut self, delta: f32) -> bool {
        if !delta.is_finite() {
            return false;
        }
        // 搜索结果滚动使用受当前表面约束后的实际视口。
        let viewport_height = self.effective_popup_height();
        // 按实际视口计算最大滚动距离。
        let max_scroll = (self.search_results.len() as f32 * self.visual.layout.item_height
            - viewport_height)
            .max(0.0);
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
        // 搜索命中复用当前表面解析后的单列弹层。
        let popup = self.interaction_popup_geometry(frame, 1).rect;
        if !point_in_half_open_rect(popup, pos) {
            return None;
        }
        let index = ((pos.y - popup.y + self.search_scroll_offset) / self.visual.layout.item_height)
            .floor() as usize;
        (index < self.search_results.len()).then_some(index)
    }

    pub(crate) fn set_search_cursor_from_x(&mut self, x: f32) {
        let glyph_xs = self.search_glyph_xs.borrow();
        if glyph_xs.len() != self.search_query.chars().count() + 1 {
            self.search_cursor_char = self.search_query.chars().count();
            return;
        }
        let scale =
            (self.interaction_frame().h / self.visual.layout.trigger_height).clamp(0.0, 1.0);
        let target = (x - self.visual.layout.trigger_horizontal_padding * scale
            + self.search_text_scroll_x.get())
        .max(0.0);
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

    // 解析并缓存当前实际级联弹层几何。
    pub(crate) fn remember_popup_geometry(
        // 借用组件状态。
        &self,
        // 接收触发器绝对布局矩形。
        frame: Rect,
        // 接收当前逻辑表面。
        surface: Rect,
        // 接收当前可见列数。
        level_count: usize,
        // 返回相对触发器原点的最终弹层几何。
    ) -> CascaderPopupGeometry {
        // 空列集合仍按单列缓存处理。
        let level_count = level_count.max(1);
        // 归一化并缓存当前逻辑表面。
        let surface = normalize_cascader_rect(surface);
        // 使用共享解析器生成绝对弹层几何。
        let absolute = resolve_cascader_popup_geometry(frame, level_count, surface);
        // 转换为组件事件路径可复用的相对几何。
        let local = local_cascader_popup_geometry(frame, absolute);
        // 缓存最终相对弹层矩形。
        self.popup_rect.set(local.rect);
        // 缓存最终实际列宽。
        self.popup_column_width.set(local.column_width);
        // 记录缓存对应的可见列数。
        self.popup_column_count.set(level_count);
        // 记录当前逻辑表面供 dirty、命中和旧入口复用。
        self.surface_rect.set(Some(surface));
        // 返回同一最终几何。
        local
    }

    // 返回最近记录的表面，首次登记前使用有限回退。
    pub(crate) fn surface_or_fallback(&self, frame: Rect) -> Rect {
        // 优先读取显式登记或绘制记录的真实表面。
        self.surface_rect
            // 读取可复制的可选表面。
            .get()
            // 首次使用时按保守列数构造有限表面。
            .unwrap_or_else(|| cascader_fallback_surface(frame, self.damage_column_count()))
    }

    // 返回事件路径应使用的实际弹层几何。
    pub(crate) fn interaction_popup_geometry(
        // 借用组件状态。
        &self,
        // 接收组件本地触发器 frame。
        frame: Rect,
        // 接收事件所需的当前列数。
        level_count: usize,
        // 返回相对组件原点的最终几何。
    ) -> CascaderPopupGeometry {
        // 空列集合仍按单列弹层处理。
        let level_count = level_count.max(1);
        // 同列数缓存可直接保证事件与登记、绘制一致。
        if self.popup_column_count.get() == level_count {
            // 返回最近解析的最终相对矩形与列宽。
            return CascaderPopupGeometry {
                // 读取缓存弹层矩形。
                rect: self.popup_rect.get(),
                // 读取缓存实际列宽。
                column_width: self.popup_column_width.get(),
            };
        }
        // 列数变化时使用最近表面重新解析。
        let surface = self.surface_or_fallback(frame);
        // 更新并返回事件路径的最终几何。
        self.remember_popup_geometry(frame, surface, level_count)
    }

    // 返回状态切换期间需要覆盖的保守弹层矩形。
    pub(crate) fn damage_popup_rect(&self, frame: Rect, surface: Rect) -> Rect {
        // 先解析并缓存当前实际可见弹层。
        let current = self
            // 使用实际可见列数。
            .remember_popup_geometry(frame, surface, self.visible_column_count())
            // 读取相对弹层矩形。
            .rect;
        // 使用过滤前后最大列数解析保守绝对几何。
        let damage = resolve_cascader_popup_geometry(frame, self.damage_column_count(), surface);
        // 将保守几何转换为同一相对坐标空间。
        let damage = local_cascader_popup_geometry(frame, damage).rect;
        // 当列数或方向变化时同时覆盖两个矩形。
        current.union(&damage)
    }

    // 返回滚动与键盘显露应使用的实际弹层高度。
    pub(crate) fn effective_popup_height(&self) -> f32 {
        // 已完成表面解析时使用最终受约束高度。
        if self.popup_column_count.get() > 0 {
            // 防止缓存高度超过自然规格。
            self.popup_rect
                .get()
                .h
                .clamp(0.0, self.visual.layout.popup_height)
        } else {
            // 首次表面解析前保持既有自然视口。
            self.visual.layout.popup_height
        }
    }

    pub(crate) fn interaction_frame(&self) -> Rect {
        self.last_frame.get().unwrap_or_else(|| {
            Rect::new(0.0, 0.0, self.intrinsic_size().w, self.intrinsic_size().h)
        })
    }

    pub(crate) fn option_at(&self, frame: Rect, pos: Point) -> Option<(usize, usize)> {
        // 普通选项命中复用当前列数的最终弹层几何。
        let geometry = self.interaction_popup_geometry(frame, self.current_levels.len());
        // 读取最终弹层矩形。
        let popup = geometry.rect;
        if !point_in_half_open_rect(popup, pos) {
            return None;
        }
        // 使用最终总宽均分后的实际列宽。
        let column_width = geometry.column_width;
        // 空列宽无法映射到有效层级。
        if column_width <= 0.0 {
            // 返回无命中。
            return None;
        }
        let level = ((pos.x - popup.x) / column_width).floor() as usize;
        let options = self.current_levels.get(level)?;
        let scroll = self.scroll_offsets.get(level).copied().unwrap_or(0.0);
        let index = ((pos.y - popup.y + scroll) / self.visual.layout.item_height).floor() as usize;
        (index < options.len()).then_some((level, index))
    }

    pub(crate) fn scroll_level(&mut self, level: usize, delta: f32) -> bool {
        if !delta.is_finite() {
            return false;
        }
        // 列滚动使用受当前表面约束后的实际视口。
        let viewport_height = self.effective_popup_height();
        let Some(options) = self.current_levels.get(level) else {
            return false;
        };
        let Some(offset) = self.scroll_offsets.get_mut(level) else {
            return false;
        };
        // 按实际视口计算最大滚动距离。
        let max_scroll =
            (options.len() as f32 * self.visual.layout.item_height - viewport_height).max(0.0);
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
        // 键盘显露使用受当前表面缩高后的实际视口。
        let viewport_height = self.effective_popup_height();
        let Some(options) = self.current_levels.get(level) else {
            return;
        };
        let Some(offset) = self.scroll_offsets.get_mut(level) else {
            return;
        };
        let item_height = self.visual.layout.item_height;
        let row_top = index as f32 * item_height;
        let row_bottom = row_top + item_height;
        if row_top < *offset {
            *offset = row_top;
        } else if row_bottom > *offset + viewport_height {
            *offset = row_bottom - viewport_height;
        }
        // 最大滚动距离同样使用实际视口高度。
        let max_scroll = (options.len() as f32 * item_height - viewport_height).max(0.0);
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

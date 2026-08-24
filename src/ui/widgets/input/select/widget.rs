use crate::core::{Constraints, Rect, Size};
use crate::platform::windowing::ControlSize;
use crate::ui::animation::TransitionPlayer;
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, LayoutChild, MouseButton, SemanticEvent, SystemEvent, WidgetId,
    WidgetTree,
};
use crate::widget;
use std::cell::{Cell, RefCell};

use super::search::VisibleRow;
use super::{
    OptGroup, SelectOption, SelectValueBinding, presentation::SelectVisual, select_dirty_rect,
    select_popup_rect,
};

widget! {
    /// 支持分组、单选或多选、搜索与弹层导航的选择组件。
    pub struct Select {
        pub(crate) options: Vec<SelectOption>,
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
        // 缓存只由选项文案与静态视觉决定的固有宽度。
        #[snapshot(skip)]
        pub(crate) intrinsic_width: Cell<Option<f32>>,
        pub(crate) search_cursor_rect: Cell<Rect>,
        pub(crate) control_rect: Cell<Rect>,
        pub(crate) dropdown_rect: Cell<Rect>,
        // 缓存当前逻辑表面，统一布局、绘制、命中与浮层登记。
        pub(crate) surface_rect: Cell<Option<Rect>>,
        pub(crate) multi_remove_rects: RefCell<Vec<(usize, Rect)>>,
        pub(crate) dropdown_scroll: VirtualListScroll,
        pub(crate) scroll_delta_strip: Cell<(f32, f32)>,
        // 同目录 UIX 注入的完整静态视觉表。
        #[snapshot(skip)]
        pub(crate) visual: &'static SelectVisual,
    }

    tab_index => (&self) -> i32 { 1 }

    accepts_text_input => (&self) -> bool { self.search && !self.disabled }

    text_input_cursor_rect => (&self) -> Rect { self.search_cursor_rect.get() }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    measure_children_into => (
        &self,
        _frame: Rect,
        children: &[WidgetId],
        _tree: &WidgetTree,
        output: &mut Vec<LayoutChild>
    ) {
        output.clear();
        output.reserve(children.len());
        output.extend(
            children
                .iter()
                .copied()
                .map(|id| LayoutChild::new(id, Size::zero())),
        );
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        let mut output = Vec::with_capacity(children.len());
        self.layout_select_children_into(frame, children, tree, &mut output);
        output
    }

    layout_children_into => (
        &self,
        frame: Rect,
        children: &[LayoutChild],
        tree: &WidgetTree,
        _scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>
    ) {
        self.layout_select_children_into(frame, children, tree, output);
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        (self.custom_option_views && self.is_present()).then(|| {
            // 子树裁剪直接复用最近解析的实际弹层矩形。
            let popup = self.dropdown_rect.get();
            // 转换为窗口绝对坐标。
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
                    // 滚轮范围使用受表面缩高后的实际视口高度。
                    let viewport_h = self.effective_dropdown_viewport_height(row_count);
                    let dy = self.dropdown_scroll.scroll_by_wheel(
                        delta.y,
                        row_count,
                        self.visual.layout.row_height,
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

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|value| SemanticEvent::change(id, value))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.is_present() {
            // 读取最近布局或绘制记录的逻辑表面。
            let surface = self.surface_or_fallback(frame);
            // 解析实际可交互弹层，而不是保守 damage 高度。
            let popup = self.remember_dropdown_rect(frame, surface, self.dropdown_row_count());
            // 命中框与最终弹层共享同一表面约束。
            select_dirty_rect(frame, popup, surface, self.visual)
        } else {
            frame
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.render_select(frame, ctx);
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // 读取最近布局或绘制记录的逻辑表面。
        let surface = self.surface_or_fallback(frame);
        // 使用保守行数解析受表面约束的重绘弹层。
        let popup = self.dropdown_damage_rect(frame, surface);
        // 控件、弹层与阴影脏区全部收敛到当前表面。
        select_dirty_rect(frame, popup, surface, self.visual)
    }

    overlay_entry => (&self, id: WidgetId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.is_present() {
            return None;
        }

        // 读取最近布局或绘制记录的逻辑表面。
        let surface = self.surface_or_fallback(frame);
        // 解析并缓存当前实际选项弹层矩形。
        let popup = self.remember_dropdown_rect(frame, surface, self.dropdown_row_count());
        // 创建与绘制和命中一致的浮层登记。
        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                .bounds(select_popup_rect(frame, popup))
                .z_index(self.visual.chrome.overlay_z),
        )
    }

    // 使用组件树提供的同帧表面创建选择弹层登记。
    overlay_entry_for_surface => (&self, id: WidgetId, frame: Rect, surface: Rect) -> Option<crate::ui::OverlayEntry> {
        // 在登记前更新表面与实际弹层缓存。
        self.remember_dropdown_rect(frame, surface, self.dropdown_row_count());
        // 复用统一的选择弹层登记逻辑。
        self.overlay_entry(id, frame)
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
                + dt.max(0.0) as f32 * std::f32::consts::TAU
                    / self.visual.motion.loading_cycle_seconds)
                .rem_euclid(std::f32::consts::TAU);
            self.loading_dirty = (self.loading_phase - before).abs() > f32::EPSILON;
        }

        transition_active || loading_active
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty || self.loading_dirty {
            // 动画脏区使用最近记录的当前逻辑表面。
            let surface = self.surface_or_fallback(frame);
            // 使用保守行数解析动画可能覆盖的弹层区域。
            let popup = self.dropdown_damage_rect(frame, surface);
            // 将动画脏区限制在当前表面。
            select_dirty_rect(frame, popup, surface, self.visual)
        } else {
            Rect::zero()
        }
    }
}

impl Select {
    // 将当前物化自定义选项的位置写入布局树拥有的跨帧数组。
    fn layout_select_children_into(
        &self,
        frame: Rect,
        children: &[LayoutChild],
        tree: &WidgetTree,
        output: &mut Vec<(WidgetId, Rect)>,
    ) {
        let surface = self.surface_from_tree(frame, tree);
        let popup = self.remember_dropdown_rect(frame, surface, self.visible_row_count().max(1));
        let list_y = frame.y + popup.y;
        let text_left = if self.multiple {
            self.visual.layout.custom_multi_left
        } else {
            self.visual.layout.row_horizontal_padding
        };
        let content_width = (popup.w - text_left - self.visual.layout.custom_right_inset).max(0.0);
        let indices = self.materialized_custom_options.borrow();
        let pair_count = children.len().min(indices.len());
        let mut next_pair = 0_usize;
        let mut row_index = 0_usize;

        output.clear();
        output.reserve(pair_count);
        self.for_each_visible_row(|row| {
            if next_pair < pair_count {
                if let VisibleRow::Option(option_index) = row {
                    if indices[next_pair] == option_index {
                        output.push((
                            children[next_pair].id,
                            Rect::new(
                                frame.x + popup.x + text_left,
                                list_y + row_index as f32 * self.visual.layout.row_height
                                    - self.dropdown_scroll.scroll_offset(),
                                content_width,
                                self.visual.layout.row_height,
                            ),
                        ));
                        next_pair += 1;
                    }
                }
            }
            row_index += 1;
        });
        if next_pair < pair_count {
            // 异常的非声明顺序物化快照仍保持旧入口逐项查找语义。
            output.clear();
            for (child, option_index) in children.iter().zip(indices.iter().copied()) {
                let mut current_row = 0_usize;
                let mut matched_row = None;
                self.for_each_visible_row(|row| {
                    if matched_row.is_none()
                        && matches!(row, VisibleRow::Option(index) if index == option_index)
                    {
                        matched_row = Some(current_row);
                    }
                    current_row += 1;
                });
                if let Some(row_index) = matched_row {
                    output.push((
                        child.id,
                        Rect::new(
                            frame.x + popup.x + text_left,
                            list_y + row_index as f32 * self.visual.layout.row_height
                                - self.dropdown_scroll.scroll_offset(),
                            content_width,
                            self.visual.layout.row_height,
                        ),
                    ));
                }
            }
        }
    }
}

// 验证选择弹层使用组件树提供的当前逻辑表面。
#[cfg(test)]
// 将打开状态与内部几何构造限制在当前模块测试中。
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/input/select/widget__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;

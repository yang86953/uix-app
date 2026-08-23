//! Cascader widget - linked multi-level popup selection.

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::ui::animation::TransitionPlayer;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::widget;
// 引入级联选中路径的双向状态句柄。
use crate::ui::reactive::state::State;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetId, WidgetTree,
};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use super::{
    CascaderOption, CascaderSearchResult, CascaderValue, absolute_cascader_popup_rect,
    cascader_dirty_rect, fade_color, paint_loading_spinner, point_in_half_open_rect,
};

const TRIGGER_HEIGHT: f32 = 32.0;
const ITEM_HEIGHT: f32 = 32.0;
const WHEEL_STEP: f32 = 40.0;

widget! {
    /// 按层级浏览并以完整稳定路径提交叶节点选择的级联组件。
    pub struct Cascader {
        pub(crate) options: Vec<CascaderOption>,
        pub(crate) selected: CascaderValue,
        // 保存声明式选中路径的双向绑定端口。
        pub(crate) value_binding: Option<State<CascaderValue>>,
        pub(crate) current_levels: Vec<Vec<CascaderOption>>,
        pub(crate) level_indices: Vec<usize>,
        pub(crate) scroll_offsets: Vec<f32>,
        pub(crate) hovered_option: Option<(usize, usize)>,
        pub(crate) open: bool,
        pub(crate) transition: TransitionPlayer,
        pub(crate) closing: bool,
        pub(crate) transition_dirty: bool,
        pub(crate) loading_children: HashSet<String>,
        pub(crate) loading_phase: f32,
        pub(crate) loading_dirty: bool,
        pub(crate) searchable: bool,
        pub(crate) search_query: String,
        pub(crate) search_results: Vec<CascaderSearchResult>,
        pub(crate) search_index: usize,
        pub(crate) search_scroll_offset: f32,
        pub(crate) search_cursor_char: usize,
        pub(crate) search_cursor_rect: Cell<Rect>,
        pub(crate) search_glyph_xs: RefCell<Vec<f32>>,
        pub(crate) search_text_scroll_x: Cell<f32>,
        pub(crate) placeholder: String,
        pub(crate) focused: bool,
        pub(crate) last_frame: Cell<Option<Rect>>,
        // 缓存相对触发器原点的最终弹层矩形。
        pub(crate) popup_rect: Cell<Rect>,
        // 缓存受表面约束后的实际列宽。
        pub(crate) popup_column_width: Cell<f32>,
        // 记录当前缓存对应的可见列数。
        pub(crate) popup_column_count: Cell<usize>,
        // 缓存布局或绘制阶段取得的当前逻辑表面。
        pub(crate) surface_rect: Cell<Option<Rect>>,
        pub(crate) pending_change: RefCell<Option<String>>,
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|value| SemanticEvent::change(id, value))
    }

    tab_index => (&self) -> i32 { 1 }

    accepts_text_input => (&self) -> bool { self.searchable }

    text_input_cursor_rect => (&self) -> Rect { self.search_cursor_rect.get() }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                self.focused = true;
                let frame = self.interaction_frame();
                if frame.contains(*pos) {
                    if self.searchable {
                        self.set_search_cursor_from_x(pos.x);
                        if self.open {
                            return EventResult::Handled;
                        }
                    }
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    return EventResult::Handled;
                }

                if self.open {
                    if self.search_active() {
                        if let Some(index) = self.search_result_at(frame, *pos) {
                            self.hovered_option = Some((0, index));
                            self.select_search_result(index);
                            return EventResult::Handled;
                        }
                    } else if let Some((level, index)) = self.option_at(frame, *pos) {
                        self.hovered_option = Some((level, index));
                        self.select_option(level, index);
                        return EventResult::Handled;
                    }
                    if point_in_half_open_rect(
                        // 复用当前表面解析后的实际弹层。
                        self.interaction_popup_geometry(frame, self.visible_column_count())
                            // 只读取最终弹层矩形。
                            .rect,
                        *pos,
                    ) {
                        return EventResult::Handled;
                    }
                }

                if self.is_present() {
                    self.close();
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if !self.open {
                    return EventResult::NotHandled;
                }
                let frame = self.interaction_frame();
                let next = if self.search_active() {
                    self.search_result_at(frame, *pos).map(|index| (0, index))
                } else {
                    self.option_at(frame, *pos)
                };
                if self.hovered_option != next {
                    self.hovered_option = next;
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerLeave => {
                if self.hovered_option.take().is_some() {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
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
            SystemEvent::Wheel { pos, delta } => {
                if !self.open || !delta.y.is_finite() {
                    return EventResult::NotHandled;
                }
                let frame = self.interaction_frame();
                // 读取当前表面约束后的事件弹层几何。
                let geometry = self.interaction_popup_geometry(frame, self.visible_column_count());
                // 命中判断复用最终弹层矩形。
                let popup = geometry.rect;
                if !point_in_half_open_rect(popup, *pos) {
                    return EventResult::NotHandled;
                }
                if self.search_active() {
                    return if self.scroll_search_results(delta.y * WHEEL_STEP) {
                        EventResult::Handled
                    } else {
                        EventResult::NotHandled
                    };
                }
                // 多列命中使用最终总宽均分后的实际列宽。
                let column_width = geometry.column_width;
                // 空列宽无法确定滚动目标列。
                if column_width <= 0.0 {
                    // 不消费落在空弹层中的滚轮事件。
                    return EventResult::NotHandled;
                }
                let level = ((pos.x - popup.x) / column_width).floor() as usize;
                if self.scroll_level(level, delta.y * WHEEL_STEP) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::KeyDown { key, .. } => {
                if !self.open {
                    return match key {
                        KeyCode::Down | KeyCode::Enter | KeyCode::Space => {
                            self.open();
                            EventResult::Handled
                        }
                        _ => EventResult::NotHandled,
                    };
                }
                match key {
                    KeyCode::Escape => {
                        self.close();
                        EventResult::Handled
                    }
                    KeyCode::Down => {
                        if self.search_active() {
                            self.move_search_highlight(true);
                        } else {
                            self.move_highlight(true);
                        }
                        EventResult::Handled
                    }
                    KeyCode::Up => {
                        if self.search_active() {
                            self.move_search_highlight(false);
                        } else {
                            self.move_highlight(false);
                        }
                        EventResult::Handled
                    }
                    KeyCode::Enter if self.search_active() => {
                        self.select_search_result(self.search_index);
                        EventResult::Handled
                    }
                    KeyCode::Backspace if self.searchable => {
                        if self.delete_previous_search_char() {
                            EventResult::Handled
                        } else {
                            EventResult::NotHandled
                        }
                    }
                    KeyCode::Delete if self.searchable => {
                        if self.delete_next_search_char() {
                            EventResult::Handled
                        } else {
                            EventResult::NotHandled
                        }
                    }
                    KeyCode::Left if self.search_active() => {
                        self.search_cursor_char = self.search_cursor_char.saturating_sub(1);
                        EventResult::Handled
                    }
                    KeyCode::Right if self.search_active() => {
                        self.search_cursor_char = (self.search_cursor_char + 1)
                            .min(self.search_query.chars().count());
                        EventResult::Handled
                    }
                    KeyCode::Home if self.search_active() => {
                        self.search_cursor_char = 0;
                        EventResult::Handled
                    }
                    KeyCode::End if self.search_active() => {
                        self.search_cursor_char = self.search_query.chars().count();
                        EventResult::Handled
                    }
                    KeyCode::Right | KeyCode::Enter | KeyCode::Space
                        if !self.search_active() =>
                    {
                        self.activate_highlight();
                        EventResult::Handled
                    }
                    KeyCode::Left => {
                        self.return_to_parent();
                        EventResult::Handled
                    }
                    KeyCode::Home => {
                        self.move_to_edge(true);
                        EventResult::Handled
                    }
                    KeyCode::End => {
                        self.move_to_edge(false);
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            SystemEvent::TextInput { text } | SystemEvent::Paste { text }
                if self.searchable =>
            {
                if self.insert_search_text(text) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        let loc = crate::ui::widget_runtime::locale::use_locale();
        let primary = ctx.tokens().color_primary();
        let border_color = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let bg_elevated = ctx.tokens().color_bg_elevated();
        let nominal_height = TRIGGER_HEIGHT;
        let scale = if nominal_height > 0.0 {
            (frame.h / nominal_height).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let font_size = 14.0 * scale;
        let horizontal_padding = 12.0 * scale;
        let arrow_gap = 4.0 * scale;
        let arrow_slot_width = 24.0 * scale;
        let arrow_right_inset = 4.0 * scale;
        let trigger_radius = Some(Radius::uniform(
            (ctx.tokens().border_radius_sm() * scale).min(frame.h.max(0.0) * 0.5),
        ));

        ctx.push_clip(frame);
        ctx.fill_rect(frame, ctx.tokens().color_bg_container(), trigger_radius);
        ctx.stroke_rect(
            frame,
            if self.focused { primary } else { border_color },
            if self.focused { 2.0 } else { 1.0 },
            trigger_radius,
        );

        if font_size > 0.0 && frame.w > 0.0 {
            let arrow_frame = Rect::new(
                frame.x + frame.w - arrow_right_inset - arrow_slot_width,
                frame.y,
                arrow_slot_width,
                frame.h,
            );
            let text_left = frame.x + horizontal_padding;
            let text_right = (arrow_frame.x - arrow_gap).max(text_left);
            let text_area = Rect::new(text_left, frame.y, text_right - text_left, frame.h);
            if text_area.w > 0.0 {
                let mut glyph_xs = Vec::with_capacity(self.search_query.chars().count() + 1);
                let mut prefix = String::new();
                glyph_xs.push(0.0);
                for ch in self.search_query.chars() {
                    prefix.push(ch);
                    glyph_xs.push(ctx.measure_text(&prefix, font_size).w);
                }
                let cursor_index = self
                    .search_cursor_char
                    .min(glyph_xs.len().saturating_sub(1));
                let total_width = glyph_xs.last().copied().unwrap_or(0.0);
                let cursor_offset = glyph_xs.get(cursor_index).copied().unwrap_or(0.0);
                let max_scroll = (total_width - text_area.w).max(0.0);
                let mut scroll = self.search_text_scroll_x.get().clamp(0.0, max_scroll);
                if cursor_offset < scroll {
                    scroll = cursor_offset;
                } else if cursor_offset > scroll + text_area.w {
                    scroll = cursor_offset - text_area.w;
                }
                scroll = scroll.clamp(0.0, max_scroll);
                self.search_text_scroll_x.set(scroll);
                *self.search_glyph_xs.borrow_mut() = glyph_xs;

                ctx.push_clip(text_area);
                let draw_y = ctx.visual_center_y(frame, font_size);
                let showing_query = self.search_active();
                if showing_query {
                    ctx.draw_text(
                        &self.search_query,
                        Point::new(text_left - scroll, draw_y),
                        text_color,
                        font_size,
                    );
                } else if self.selected.labels.is_empty() {
                    ctx.draw_text(
                        &self.placeholder,
                        Point::new(text_left, draw_y),
                        text_tertiary,
                        font_size,
                    );
                } else {
                    let display_text = self.selected.labels.join(loc.cascader_separator);
                    ctx.draw_text(
                        &display_text,
                        Point::new(text_left, draw_y),
                        text_color,
                        font_size,
                    );
                }
                let caret_height = (18.0 * scale).min(text_area.h);
                let caret_x = (text_area.x + cursor_offset - scroll)
                    .clamp(text_area.x, text_area.x + text_area.w);
                let caret = Rect::new(
                    caret_x,
                    text_area.y + (text_area.h - caret_height) * 0.5,
                    1.0,
                    caret_height,
                );
                self.search_cursor_rect.set(caret);
                if self.searchable && self.focused && self.open {
                    ctx.fill_rect(caret, primary, None);
                }
                ctx.pop_clip();
            } else {
                self.search_glyph_xs.replace(vec![0.0]);
                self.search_text_scroll_x.set(0.0);
                self.search_cursor_rect
                    .set(Rect::new(text_area.x, text_area.y, 0.0, text_area.h));
            }
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                if self.is_present() {
                    "chevron-up"
                } else {
                    "chevron-down"
                },
                arrow_frame,
                text_secondary,
                font_size,
            );
        }
        ctx.pop_clip();

        if !self.is_present() {
            return;
        }

        // 将逻辑表面映射到当前组件坐标，兼容被提升的滚动浮层。
        let surface = ctx.logical_surface_rect();
        // 在绘制弹层前解析并缓存同帧最终几何。
        let geometry = self.remember_popup_geometry(frame, surface, self.visible_column_count());
        // 将相对触发器缓存转换为窗口绝对弹层矩形。
        let popup = absolute_cascader_popup_rect(frame, geometry.rect);
        // 空表面不生成可见级联弹层。
        if popup.w <= 0.0 || popup.h <= 0.0 {
            // 保留触发器绘制结果并跳过弹层。
            return;
        }

        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        // 所有列绘制使用表面约束后的实际列宽。
        let column_width = geometry.column_width;
        let bg_elevated = fade_color(bg_elevated, opacity);
        let border_color = fade_color(border_color, opacity);
        let text_color = fade_color(text_color, opacity);
        let text_secondary = fade_color(text_secondary, opacity);
        let text_tertiary = fade_color(text_tertiary, opacity);
        let primary_bg = fade_color(ctx.tokens().color_primary_bg(), opacity);
        let panel_radius = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        // 将整个级联弹层裁剪到当前逻辑表面。
        ctx.push_clip(surface);
        // 再按最终弹层矩形裁剪行与边框。
        ctx.push_clip(popup);
        ctx.fill_rect(popup, bg_elevated, panel_radius);

        if self.search_active() {
            let column = Rect::new(popup.x, popup.y, column_width, popup.h);
            ctx.push_clip(column);
            if self.search_results.is_empty() {
                // 级联下拉字号：统一使用主题 font_size token。
                ctx.text_center(loc.no_data, column, text_tertiary, ctx.tokens().font_size());
            }
            for (index, result) in self.search_results.iter().enumerate() {
                let y = column.y + index as f32 * ITEM_HEIGHT - self.search_scroll_offset;
                if y + ITEM_HEIGHT <= column.y || y >= column.y + column.h {
                    continue;
                }
                let row = Rect::new(column.x, y, column.w, ITEM_HEIGHT);
                if self.hovered_option == Some((0, index)) || self.search_index == index {
                    ctx.fill_rect(row, primary_bg, None);
                }
                let loading_width = if result.loading { 24.0 } else { 0.0 };
                let text_area = Rect::new(
                    row.x + 12.0,
                    row.y,
                    (row.w - 24.0 - loading_width).max(0.0),
                    row.h,
                );
                if text_area.w > 0.0 {
                    let label = result.value.labels.join(loc.cascader_separator);
                    ctx.push_clip(text_area);
                    let text_y = ctx.visual_center_y(row, ctx.tokens().font_size());
                    ctx.draw_text(
                        &label,
                        Point::new(text_area.x, text_y),
                        if result.disabled { text_tertiary } else { text_color },
                        14.0,
                    );
                    ctx.pop_clip();
                }
                if result.loading {
                    paint_loading_spinner(ctx, row, self.loading_phase, text_secondary);
                }
            }
            ctx.pop_clip();
        } else {
        for (level, options) in self.current_levels.iter().enumerate() {
            let column = Rect::new(
                popup.x + level as f32 * column_width,
                popup.y,
                column_width,
                popup.h,
            );
            ctx.push_clip(column);
            if options.is_empty() {
                // 级联下拉字号：统一使用主题 font_size token。
                ctx.text_center(loc.no_data, column, text_tertiary, ctx.tokens().font_size());
            }
            let scroll = self.scroll_offsets.get(level).copied().unwrap_or(0.0);
            for (index, option) in options.iter().enumerate() {
                let y = column.y + index as f32 * ITEM_HEIGHT - scroll;
                if y + ITEM_HEIGHT <= column.y || y >= column.y + column.h {
                    continue;
                }
                let row = Rect::new(column.x, y, column.w, ITEM_HEIGHT);
                let highlighted = self.hovered_option == Some((level, index))
                    || self.level_indices.get(level) == Some(&index);
                if highlighted {
                    ctx.fill_rect(row, primary_bg, None);
                }

                let loading = self.loading_children.contains(&option.value);
                let arrow_width = if option.children.is_empty() && !loading {
                    0.0
                } else {
                    24.0
                };
                let text_area = Rect::new(
                    row.x + 12.0,
                    row.y,
                    (row.w - 24.0 - arrow_width).max(0.0),
                    row.h,
                );
                if text_area.w > 0.0 {
                    ctx.push_clip(text_area);
                    let text_y = ctx.visual_center_y(row, ctx.tokens().font_size());
                    ctx.draw_text(
                        &option.label,
                        Point::new(text_area.x, text_y),
                        if option.disabled { text_tertiary } else { text_color },
                        14.0,
                    );
                    ctx.pop_clip();
                }
                if loading {
                    paint_loading_spinner(ctx, row, self.loading_phase, text_secondary);
                } else if !option.children.is_empty() {
                    let arrow = Rect::new(row.x + row.w - 24.0, row.y, 24.0, row.h);
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        "chevron-right",
                        arrow,
                        text_secondary,
                        14.0,
                    );
                }
            }
            ctx.pop_clip();
            if level > 0 {
                ctx.fill_rect(
                    Rect::new(column.x, column.y, 1.0, column.h),
                    border_color,
                    None,
                );
            }
        }
        }
        ctx.stroke_rect(popup, border_color, 1.0, panel_radius);
        ctx.pop_clip();
        // 恢复弹层外层的逻辑表面裁剪。
        ctx.pop_clip();
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // 读取最近登记或绘制记录的当前逻辑表面。
        let surface = self.surface_or_fallback(frame);
        // 合并实际列数与保守列数可能覆盖的弹层。
        let popup = self.damage_popup_rect(frame, surface);
        // 将触发器和弹层脏区限制在当前表面内。
        cascader_dirty_rect(frame, popup, surface)
    }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.is_present() {
            // 读取最近登记或绘制记录的当前逻辑表面。
            let surface = self.surface_or_fallback(frame);
            // 命中只使用当前实际可见列数解析弹层。
            let popup = self
                // 记录同一最终几何供事件路径复用。
                .remember_popup_geometry(frame, surface, self.visible_column_count())
                // 读取相对触发器矩形。
                .rect;
            // 触发器与弹层命中框共同收敛到当前表面。
            cascader_dirty_rect(frame, popup, surface)
        } else {
            frame
        }
    }

    overlay_entry => (&self, id: crate::ui::WidgetId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        self.is_present().then(|| {
            // 读取最近记录的表面或首次有限回退。
            let surface = self.surface_or_fallback(frame);
            // 解析并缓存当前实际可见弹层。
            let popup = self
                // 所有登记消费者复用同一最终几何。
                .remember_popup_geometry(frame, surface, self.visible_column_count())
                // 读取相对触发器矩形。
                .rect;
            // 将最终弹层转换为窗口绝对坐标。
            let bounds = absolute_cascader_popup_rect(frame, popup);
            // 创建只覆盖实际弹层的浮层登记。
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                // 登记边界与绘制、命中共用同一矩形。
                .bounds(bounds)
                .z_index(900)
        })
    }

    // 使用组件树提供的同帧表面创建级联选择弹层登记。
    overlay_entry_for_surface => (&self, id: crate::ui::WidgetId, frame: Rect, surface: Rect) -> Option<crate::ui::OverlayEntry> {
        // 在旧登记入口执行前刷新表面与实际弹层缓存。
        self.remember_popup_geometry(frame, surface, self.visible_column_count());
        // 复用统一的级联弹层登记逻辑。
        self.overlay_entry(id, frame)
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() {
            self.transition_dirty = false;
            self.loading_dirty = false;
            return false;
        }

        let transition_active = if self.transition.finished {
            self.transition_dirty = false;
            false
        } else {
            self.transition.update(dt);
            self.transition_dirty = true;

            if self.closing && self.transition.finished {
                self.open = false;
                self.closing = false;
            }

            self.is_present() && !self.transition.finished
        };

        self.loading_dirty = false;
        let loading_active = self.is_present() && self.has_visible_loading_child();
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
            // 动画脏区使用最近记录的当前逻辑表面。
            let surface = self.surface_or_fallback(frame);
            // 动画期间同时覆盖实际与保守列数的最终弹层。
            let popup = self.damage_popup_rect(frame, surface);
            // 将动画脏区限制在当前表面。
            cascader_dirty_rect(frame, popup, surface)
        } else {
            Rect::zero()
        }
    }
}

// 验证级联选择弹层使用组件树提供的当前逻辑表面。
#[cfg(test)]
// 将打开状态与内部滚动几何限制在当前模块测试中。
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/input/cascader/widget__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;

//! Cascader widget - linked multi-level popup selection.

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::ui::animation::TransitionPlayer;
use crate::ui::widget_runtime::paint_context::PaintContext;
// 引入级联选中路径的双向状态句柄。
use crate::ui::reactive::state::State;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetTree,
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

component! {
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

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
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

        // 从绘制上下文读取当前逻辑表面尺寸。
        let surface_size = ctx.logical_surface_size();
        // 将窗口原点与逻辑尺寸组合为当前表面矩形。
        let surface = Rect::new(0.0, 0.0, surface_size.w, surface_size.h);
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

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
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
    overlay_entry_for_surface => (&self, id: crate::ui::ComponentId, frame: Rect, surface: Rect) -> Option<crate::ui::OverlayEntry> {
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
mod tests {
    // 复用被测级联选择组件和选项类型。
    use super::{Cascader, CascaderOption};
    // 引入命中断言所需的点与矩形基础类型。
    use crate::core::{Point, Rect};
    // 引入绑定测试所需的状态和值模型。
    use crate::ui::{CascaderValue, State};

    // 构造指定数量的稳定叶子选项。
    fn leaf_options(count: usize) -> Vec<CascaderOption> {
        // 为每个序号创建不同标签和值。
        (0..count)
            // 将序号映射为叶子选项。
            .map(|index| CascaderOption::new(format!("选项{index}"), format!("value-{index}")))
            // 收集为组件构造函数所需列表。
            .collect()
    }

    // 验证叶路径选择回写声明式业务状态。
    #[test]
    // 覆盖构造读取和叶选项提交两个方向。
    fn value_binding_reads_and_writes_complete_paths() {
        // 准备带既有路径的业务状态。
        let state = State::new(CascaderValue {
            // 初始显示标签由业务状态提供。
            labels: vec!["旧标签".to_owned()],
            // 初始稳定值由业务状态提供。
            values: vec!["old".to_owned()],
        });
        // 创建包含一个可提交叶项的受控级联选择器。
        let mut cascader = Cascader::new(
            // 叶项标签和值相互独立。
            vec![CascaderOption::new("新标签", "new")],
            // 占位文本不影响绑定验证。
            "请选择",
        )
        // 绑定业务状态句柄。
        .value(&state);

        // 构造阶段必须读取既有业务状态。
        assert_eq!(cascader.selected().values, vec!["old".to_owned()]);
        // 选择完整叶路径并触发状态提交。
        cascader.select_option(0, 0);
        // 外部状态必须收到新的稳定值路径。
        assert_eq!(state.get().values, vec!["new".to_owned()]);
        // 外部状态同时保留对应显示标签路径。
        assert_eq!(state.get().labels, vec!["新标签".to_owned()]);
    }

    // 靠近表面底边时弹层必须翻转并按上方可用空间缩高。
    #[test]
    // 测试名称说明纵向表面约束职责。
    fn overlay_entry_constrains_popup_near_surface_bottom() {
        // 创建包含多行选项的级联选择器。
        let mut cascader = Cascader::new(leaf_options(10), "请选择");
        // 打开级联弹层参与登记。
        cascader.open();
        // 将触发器放在一百二十像素高表面的底部附近。
        let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
        // 构造小于固定弹层高度的当前逻辑表面。
        let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
        // 通过组件树使用的显式表面入口创建登记。
        let overlay = crate::ui::widget_runtime::traits::WidgetRender::overlay_entry_for_surface(
            // 传入被测级联选择组件。
            &cascader,
            // 使用稳定的测试组件标识。
            crate::core::ComponentId::new(8),
            // 传入靠近底边的触发器 frame。
            frame,
            // 传入当前逻辑表面。
            surface,
        )
        // 打开状态必须生成浮层登记。
        .expect("打开的级联选择器应生成浮层登记")
        // 读取登记的绝对弹层矩形。
        .bounds_rect()
        // 级联弹层登记必须声明边界。
        .expect("级联选择弹层应声明边界");

        // 下方空间不足时弹层应完整位于触发器上方。
        assert!(overlay.y + overlay.h <= frame.y);
        // 弹层顶边不得越出当前表面。
        assert!(overlay.y >= surface.y);
        // 弹层底边不得越出当前表面。
        assert!(overlay.y + overlay.h <= surface.y + surface.h);
    }

    // 触发器靠近窄表面右边缘时弹层必须横向收敛。
    #[test]
    // 测试名称说明横向表面约束职责。
    fn overlay_entry_constrains_popup_to_narrow_surface() {
        // 创建单列级联选择器。
        let mut cascader = Cascader::new(leaf_options(1), "请选择");
        // 打开级联弹层参与登记。
        cascader.open();
        // 构造宽于表面且靠近右边界的触发器。
        let frame = Rect::new(70.0, 20.0, 120.0, 32.0);
        // 使用一百像素宽的窄逻辑表面。
        let surface = Rect::new(0.0, 0.0, 100.0, 260.0);
        // 通过显式表面入口创建弹层登记。
        let overlay = crate::ui::widget_runtime::traits::WidgetRender::overlay_entry_for_surface(
            // 传入被测级联选择组件。
            &cascader,
            // 使用稳定的测试组件标识。
            crate::core::ComponentId::new(9),
            // 传入靠近右边界的触发器 frame。
            frame,
            // 传入当前窄表面。
            surface,
        )
        // 打开状态必须生成浮层登记。
        .expect("打开的级联选择器应生成浮层登记")
        // 读取登记的绝对弹层矩形。
        .bounds_rect()
        // 级联弹层登记必须声明边界。
        .expect("级联选择弹层应声明边界");

        // 弹层左边不得越出当前表面。
        assert!(overlay.x >= surface.x);
        // 弹层右边不得越出当前表面。
        assert!(overlay.x + overlay.w <= surface.x + surface.w);
        // 弹层宽度不得超过当前表面宽度。
        assert!(overlay.w <= surface.w);
    }

    // 弹层缩高后滚动范围必须使用实际视口而非固定二百像素。
    #[test]
    // 测试名称说明缩高视口与滚动账本的一致性。
    fn constrained_popup_height_drives_scroll_range() {
        // 创建十行选项使内容高度达到三百二十像素。
        let mut cascader = Cascader::new(leaf_options(10), "请选择");
        // 打开级联弹层参与表面解析。
        cascader.open();
        // 将触发器放在表面底部附近，使上方只有七十八像素。
        let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
        // 使用一百二十像素高表面触发向上缩高。
        let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
        // 先通过显式登记入口记录实际受约束视口。
        let _ = crate::ui::widget_runtime::traits::WidgetRender::overlay_entry_for_surface(
            // 传入被测级联选择组件。
            &cascader,
            // 使用稳定的测试组件标识。
            crate::core::ComponentId::new(10),
            // 传入靠近底边的触发器 frame。
            frame,
            // 传入当前逻辑表面。
            surface,
        );
        // 请求滚动到当前列末端。
        assert!(cascader.scroll_level(0, 1000.0));

        // 三百二十像素内容减七十八像素实际视口应留下二百四十二像素范围。
        assert_eq!(cascader.scroll_offsets[0], 242.0);
    }

    // 多列总宽被表面压缩后事件映射必须使用实际等分列宽。
    #[test]
    // 测试名称说明绘制列宽与第二列命中的一致性。
    fn constrained_multi_column_width_drives_hit_mapping() {
        // 创建带两项子级的根选项。
        let root = CascaderOption::new("根", "root")
            // 子级在选择根项后形成第二列。
            .children(leaf_options(2));
        // 创建只含该根项的级联选择器。
        let mut cascader = Cascader::new(vec![root], "请选择");
        // 打开级联弹层并初始化根列。
        cascader.open();
        // 选择根项以展开第二列但不关闭弹层。
        cascader.select_option(0, 0);
        // 使用宽一百二十像素的触发器。
        let frame = Rect::new(20.0, 20.0, 120.0, 32.0);
        // 三百像素宽表面不足以容纳两列各二百像素自然宽。
        let surface = Rect::new(0.0, 0.0, 300.0, 260.0);
        // 通过显式表面入口解析并缓存两列最终几何。
        let overlay = crate::ui::widget_runtime::traits::WidgetRender::overlay_entry_for_surface(
            // 传入被测级联选择组件。
            &cascader,
            // 使用稳定的测试组件标识。
            crate::core::ComponentId::new(11),
            // 传入当前触发器绝对 frame。
            frame,
            // 传入当前逻辑表面。
            surface,
        )
        // 打开状态必须生成浮层登记。
        .expect("打开的两列级联选择器应生成浮层登记")
        // 读取最终绝对弹层矩形。
        .bounds_rect()
        // 两列弹层必须声明边界。
        .expect("两列级联弹层应声明边界");

        // 两列总宽应收敛为三百像素表面宽度。
        assert_eq!(overlay.w, 300.0);
        // 使用组件本地坐标命中压缩后的第二列首行。
        let hit = cascader.option_at(
            // 事件路径中的触发器 frame 以组件原点为基准。
            Rect::new(0.0, 0.0, frame.w, frame.h),
            // 横坐标落在实际一百五十像素列宽的第二列。
            Point::new(160.0, 50.0),
        );
        // 命中必须映射到第二层首项而不是自然二百像素宽的第一列。
        assert_eq!(hit, Some((1, 0)));
    }
}

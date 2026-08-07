//! Mentions 提及输入组件——输入 `@` 触发候选列表。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};
use std::cell::{Cell, RefCell};

// 将表面约束与坐标转换隔离到私有几何模块。
mod geometry;
// 将弹层缓存与实际视口方法隔离到私有实现模块。
mod methods;

// 复用所有 Mentions 消费端共享的最终几何函数。
use geometry::{
    // 将相对弹层转换为窗口绝对坐标。
    absolute_mentions_popup_rect,
    // 合并触发器、弹层与当前表面。
    mentions_surface_rect,
};

const CONTROL_HEIGHT: f32 = 32.0;
const SUGGESTION_ROW_HEIGHT: f32 = 28.0;
const MAX_POPUP_HEIGHT: f32 = 280.0;
const MIN_POPUP_WIDTH: f32 = 200.0;
const FONT_SIZE: f32 = 13.0;

component! {
    /// Mentions——`@` 提及输入框。
    ///
    /// 输入 `@` 后按当前光标位置过滤候选，提交后替换活动查询并保留其余正文。
    pub struct Mentions {
        /// 当前输入文本。
        value: String,
        /// 占位文本。
        placeholder: String,
        /// 候选列表。
        options: Vec<String>,
        /// 过滤后的候选列表。
        filtered: Vec<String>,
        /// 是否正在显示建议。
        suggesting: bool,
        /// 触发文本，当前固定为 `@`。
        trigger: String,
        /// 当前光标前的活动查询文本。
        search_text: String,
        /// 键盘活动候选索引。
        selected_index: usize,
        /// 是否获得焦点。
        focused: bool,
        /// 指针是否位于输入框。
        hovered: bool,
        /// 指针悬浮的候选索引。
        hovered_option: Option<usize>,
        /// 以 Unicode 字符计数的光标位置。
        cursor_char: usize,
        /// 平台输入法候选窗锚点。
        cursor_rect: Cell<Rect>,
        /// 最近一次绘制所得的字形前缀横坐标。
        glyph_xs: RefCell<Vec<f32>>,
        /// 单行文本水平滚动位置。
        text_scroll_x: Cell<f32>,
        /// 等待发布的语义 Change 值。
        pending_change: RefCell<Option<String>>,
        /// 候选列表虚拟滚动状态。
        dropdown_scroll: VirtualListScroll,
        /// 可供局部脏区复用的滚动增量。
        scroll_delta_strip: Cell<(f32, f32)>,
        /// 最近一次实际布局尺寸对应的本地交互框。
        last_frame: Cell<Option<Rect>>,
        /// 相对触发器原点的最终弹层矩形。
        popup_rect: Cell<Rect>,
        /// 弹层缓存对应的显示行数。
        popup_row_count: Cell<usize>,
        /// 最近登记或绘制使用的逻辑表面。
        surface_rect: Cell<Option<Rect>>,
        /// 最近登记时的绝对触发器锚点。
        popup_anchor_frame: Cell<Option<Rect>>,
        /// 当前呈现周期内新旧绝对弹层脏区。
        popup_damage_rect: Cell<Rect>,
    }

    tab_index => (&self) -> i32 { 1 }

    accepts_text_input => (&self) -> bool { true }

    text_input_cursor_rect => (&self) -> Rect { self.cursor_rect.get() }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

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
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if point_in_half_open_rect(self.interaction_frame(), *pos) {
                    self.focused = true;
                    self.set_cursor_from_x(pos.x);
                    self.refresh_suggestion_from_value();
                    return EventResult::Handled;
                }
                if self.suggesting && point_in_half_open_rect(self.popup_local_rect(), *pos) {
                    if let Some(index) = self.dropdown_row_at(*pos) {
                        self.hovered_option = Some(index);
                        if self.select_index(index) {
                            return EventResult::Handled;
                        }
                    }
                    return EventResult::Handled;
                }
                self.stop_suggesting();
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let next_hovered = point_in_half_open_rect(self.interaction_frame(), *pos);
                let next_option = if self.suggesting {
                    self.dropdown_row_at(*pos)
                } else {
                    None
                };
                if self.hovered != next_hovered || self.hovered_option != next_option {
                    self.hovered = next_hovered;
                    self.hovered_option = next_option;
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerEnter => {
                if self.hovered {
                    EventResult::NotHandled
                } else {
                    self.hovered = true;
                    EventResult::Handled
                }
            }
            SystemEvent::PointerLeave => {
                let changed = self.hovered || self.hovered_option.is_some();
                self.hovered = false;
                self.hovered_option = None;
                if changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                self.refresh_suggestion_from_value();
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.stop_suggesting();
                EventResult::Handled
            }
            SystemEvent::Wheel { delta, pos } => {
                if self.suggesting && point_in_half_open_rect(self.popup_local_rect(), *pos) {
                    let dy = self.dropdown_scroll.scroll_by_wheel(
                        delta.y,
                        self.filtered.len(),
                        SUGGESTION_ROW_HEIGHT,
                        // 滚轮范围使用表面约束后的实际视口。
                        self.effective_popup_viewport_height(self.filtered.len()),
                    );
                    if dy.abs() > 0.01 {
                        self.push_scroll_delta(0.0, dy);
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Backspace => {
                        if !self.delete_previous_char() {
                            return EventResult::NotHandled;
                        }
                    }
                    KeyCode::Delete => {
                        if !self.delete_next_char() {
                            return EventResult::NotHandled;
                        }
                    }
                    KeyCode::Left => {
                        self.cursor_char = self.cursor_char.saturating_sub(1);
                        self.refresh_suggestion_from_value();
                    }
                    KeyCode::Right => {
                        self.cursor_char =
                            (self.cursor_char + 1).min(self.value.chars().count());
                        self.refresh_suggestion_from_value();
                    }
                    KeyCode::Home => {
                        self.cursor_char = 0;
                        self.refresh_suggestion_from_value();
                    }
                    KeyCode::End => {
                        self.cursor_char = self.value.chars().count();
                        self.refresh_suggestion_from_value();
                    }
                    KeyCode::Down => {
                        if !self.suggesting {
                            self.refresh_suggestion_from_value();
                        }
                        if !self.suggesting {
                            return EventResult::NotHandled;
                        }
                        if !self.filtered.is_empty() {
                            self.selected_index =
                                (self.selected_index + 1) % self.filtered.len();
                            self.reveal_selected();
                        }
                    }
                    KeyCode::Up => {
                        if !self.suggesting {
                            self.refresh_suggestion_from_value();
                        }
                        if !self.suggesting {
                            return EventResult::NotHandled;
                        }
                        if !self.filtered.is_empty() {
                            self.selected_index = (self.selected_index + self.filtered.len() - 1)
                                % self.filtered.len();
                            self.reveal_selected();
                        }
                    }
                    KeyCode::Enter if self.suggesting => {
                        if !self.select_index(self.selected_index) {
                            self.stop_suggesting();
                        }
                    }
                    KeyCode::Escape if self.suggesting => {
                        self.stop_suggesting();
                    }
                    _ => return EventResult::NotHandled,
                }
                EventResult::Handled
            }
            SystemEvent::TextInput { text } | SystemEvent::Paste { text } => {
                if self.insert_text(text) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|value| SemanticEvent::change(id, value))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.suggesting {
            // 读取最近登记或绘制记录的当前逻辑表面。
            let surface = self.surface_or_fallback(frame, self.filtered.len());
            // 命中只使用当前实际候选行数解析弹层。
            let popup = self.remember_popup_rect(frame, surface, self.filtered.len());
            // 将相对弹层转换为窗口绝对坐标。
            let popup = absolute_mentions_popup_rect(frame, popup);
            // 触发器与实际弹层命中框共同收敛到当前表面。
            mentions_surface_rect(frame, popup, surface)
        } else {
            frame
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w.max(0.0), frame.h.max(0.0))));
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_tertiary = ctx.tokens().color_text_quaternary();
        let scale = (frame.h / CONTROL_HEIGHT).clamp(0.0, 1.0);
        let font_size = FONT_SIZE * scale;
        let padding = 10.0 * scale;
        let input_rect = Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0));
        let radius = Some(Radius::uniform(
            (ctx.tokens().border_radius_sm() * scale).min(input_rect.h * 0.5),
        ));
        let border_color = if self.focused {
            primary
        } else if self.hovered {
            ctx.tokens().color_primary_hover()
        } else {
            border
        };
        ctx.push_clip(input_rect);
        ctx.fill_rect(input_rect, bg, radius);
        ctx.stroke_rect(
            input_rect,
            border_color,
            if self.focused { 2.0 } else { 1.0 },
            radius,
        );

        let text_area = Rect::new(
            input_rect.x + padding,
            input_rect.y,
            (input_rect.w - padding * 2.0).max(0.0),
            input_rect.h,
        );
        if font_size > 0.0 && text_area.w > 0.0 {
            let mut glyph_xs = Vec::with_capacity(self.value.chars().count() + 1);
            let mut prefix = String::new();
            glyph_xs.push(0.0);
            for ch in self.value.chars() {
                prefix.push(ch);
                glyph_xs.push(ctx.measure_text(&prefix, font_size).w);
            }
            let cursor_index = self.cursor_char.min(glyph_xs.len().saturating_sub(1));
            let total_width = glyph_xs.last().copied().unwrap_or(0.0);
            let cursor_offset = glyph_xs.get(cursor_index).copied().unwrap_or(0.0);
            let max_scroll = (total_width - text_area.w).max(0.0);
            let mut scroll = self.text_scroll_x.get().clamp(0.0, max_scroll);
            if cursor_offset < scroll {
                scroll = cursor_offset;
            } else if cursor_offset > scroll + text_area.w {
                scroll = cursor_offset - text_area.w;
            }
            scroll = scroll.clamp(0.0, max_scroll);
            self.text_scroll_x.set(scroll);
            *self.glyph_xs.borrow_mut() = glyph_xs;

            let display = if self.value.is_empty() {
                &self.placeholder
            } else {
                &self.value
            };
            let display_color = if self.value.is_empty() {
                text_tertiary
            } else {
                text
            };
            let draw_x = if self.value.is_empty() {
                text_area.x
            } else {
                text_area.x - scroll
            };
            let draw_y = ctx.visual_center_y(text_area, font_size);
            ctx.push_clip(text_area);
            ctx.draw_text(display, Point::new(draw_x, draw_y), display_color, font_size);

            let caret_h = (18.0 * scale).min(text_area.h);
            let caret_x = (text_area.x + cursor_offset - scroll)
                .clamp(text_area.x, text_area.x + text_area.w);
            let caret_y = text_area.y + (text_area.h - caret_h) * 0.5;
            let cursor_rect = Rect::new(caret_x, caret_y, 1.0, caret_h);
            self.cursor_rect.set(cursor_rect);
            if self.focused {
                ctx.fill_rect(cursor_rect, primary, None);
            }
            ctx.pop_clip();
        } else {
            self.glyph_xs.replace(vec![0.0]);
            self.text_scroll_x.set(0.0);
            self.cursor_rect
                .set(Rect::new(text_area.x, text_area.y, 0.0, text_area.h));
        }
        ctx.pop_clip();

        if self.suggesting {
            // 从绘制上下文读取当前逻辑表面尺寸。
            let surface_size = ctx.logical_surface_size();
            // 将窗口原点与逻辑尺寸组合为当前表面矩形。
            let surface = Rect::new(0.0, 0.0, surface_size.w, surface_size.h);
            // 在绘制弹层前解析并缓存同帧最终几何。
            let popup = self.remember_popup_rect(frame, surface, self.filtered.len());
            // 将相对触发器缓存转换为窗口绝对弹层矩形。
            let menu_rect = absolute_mentions_popup_rect(frame, popup);
            // 空表面不生成可见提及弹层。
            if menu_rect.w <= 0.0 || menu_rect.h <= 0.0 {
                // 保留输入框绘制结果并跳过弹层。
                return;
            }
            let panel_radius = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
            // 将整个提及弹层裁剪到当前逻辑表面。
            ctx.push_clip(surface);
            // 再按最终弹层矩形裁剪候选行与边框。
            ctx.push_clip(menu_rect);
            ctx.fill_rect(menu_rect, ctx.tokens().color_bg_elevated(), panel_radius);
            ctx.stroke_rect(menu_rect, border, 1.0, panel_radius);

            if self.filtered.is_empty() {
                let no_data_area = Rect::new(
                    menu_rect.x + 10.0,
                    menu_rect.y,
                    (menu_rect.w - 20.0).max(0.0),
                    menu_rect.h,
                );
                let y = ctx.visual_center_y(no_data_area, FONT_SIZE);
                ctx.push_clip(no_data_area);
                ctx.draw_text(
                    crate::ui::component::locale::use_locale().no_data,
                    Point::new(no_data_area.x, y),
                    text_secondary,
                    FONT_SIZE,
                );
                ctx.pop_clip();
                ctx.pop_clip();
                // 恢复弹层外层的逻辑表面裁剪。
                ctx.pop_clip();
                return;
            }

            let scroll_offset = self.dropdown_scroll.scroll_offset();
            let (start, end) = self.dropdown_scroll.scroll_range(
                self.filtered.len(),
                SUGGESTION_ROW_HEIGHT,
                menu_rect.h,
            );
            for (index, option) in self.filtered.iter().enumerate().take(end).skip(start) {
                let y = menu_rect.y + index as f32 * SUGGESTION_ROW_HEIGHT - scroll_offset;
                if y + SUGGESTION_ROW_HEIGHT <= menu_rect.y
                    || y >= menu_rect.y + menu_rect.h
                {
                    continue;
                }
                let item_rect = Rect::new(menu_rect.x, y, menu_rect.w, SUGGESTION_ROW_HEIGHT);
                if index == self.selected_index || self.hovered_option == Some(index) {
                    ctx.fill_rect(item_rect, ctx.tokens().color_fill_tertiary(), None);
                }
                let option_area = Rect::new(
                    item_rect.x + 10.0,
                    item_rect.y,
                    (item_rect.w - 20.0).max(0.0),
                    item_rect.h,
                );
                let text_y = ctx.visual_center_y(option_area, FONT_SIZE);
                ctx.push_clip(option_area);
                ctx.draw_text(option, Point::new(option_area.x, text_y), text, FONT_SIZE);
                ctx.pop_clip();
            }
            ctx.pop_clip();
            // 恢复弹层外层的逻辑表面裁剪。
            ctx.pop_clip();
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // 读取当前候选行数供表面回退与脏区解析共用。
        let row_count = self.filtered.len();
        // 读取最近登记或绘制记录的当前逻辑表面。
        let surface = self.surface_or_fallback(frame, row_count);
        // 合并当前与本次呈现周期历史弹层脏区。
        let popup = self.damage_popup_rect(frame, surface, row_count);
        // 将输入框和弹层脏区限制在当前表面内。
        mentions_surface_rect(frame, popup, surface)
    }

    overlay_entry => (&self, id: ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        self.suggesting.then(|| {
            // 读取当前候选行数供表面回退与弹层解析共用。
            let row_count = self.filtered.len();
            // 读取最近记录的表面或首次有限回退。
            let surface = self.surface_or_fallback(frame, row_count);
            // 解析并缓存当前实际弹层。
            let popup = self.remember_popup_rect(frame, surface, row_count);
            // 将相对弹层转换为窗口绝对坐标。
            let bounds = absolute_mentions_popup_rect(frame, popup);
            // 创建只覆盖实际弹层的浮层登记。
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                // 登记边界与绘制、命中共用同一矩形。
                .bounds(bounds)
                .z_index(900)
        })
    }

    // 使用组件树提供的同帧表面创建提及弹层登记。
    overlay_entry_for_surface => (&self, id: ComponentId, frame: Rect, surface: Rect) -> Option<crate::ui::OverlayEntry> {
        // 在旧登记入口执行前刷新表面与实际弹层缓存。
        self.remember_popup_rect(frame, surface, self.filtered.len());
        // 复用统一的提及弹层登记逻辑。
        self.overlay_entry(id, frame)
    }
}

impl Mentions {
    fn intrinsic_size(&self) -> Size {
        Size::new(200.0, CONTROL_HEIGHT)
    }

    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            value: String::new(),
            placeholder: placeholder.into(),
            options: Vec::new(),
            filtered: Vec::new(),
            suggesting: false,
            trigger: "@".to_owned(),
            search_text: String::new(),
            selected_index: 0,
            focused: false,
            hovered: false,
            hovered_option: None,
            cursor_char: 0,
            cursor_rect: Cell::new(Rect::zero()),
            glyph_xs: RefCell::new(vec![0.0]),
            text_scroll_x: Cell::new(0.0),
            pending_change: RefCell::new(None),
            dropdown_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            last_frame: Cell::new(None),
            // 首次表面解析前弹层缓存为空。
            popup_rect: Cell::new(Rect::zero()),
            // 零行标记尚未生成有效弹层缓存。
            popup_row_count: Cell::new(0),
            // 首次登记或绘制前尚未取得当前逻辑表面。
            surface_rect: Cell::new(None),
            // 首次登记前尚未取得绝对触发器锚点。
            popup_anchor_frame: Cell::new(None),
            // 首次呈现前没有历史弹层脏区。
            popup_damage_rect: Cell::new(Rect::zero()),
        }
    }

    /// 设置建议候选列表。
    pub fn options(mut self, opts: Vec<impl Into<String>>) -> Self {
        self.options = opts.into_iter().map(Into::into).collect();
        self
    }

    /// 返回当前完整输入文本。
    pub fn value(&self) -> &str {
        &self.value
    }

    // 测试目标保留 mention 建议状态观测入口，供输入交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn is_suggesting(&self) -> bool {
        self.suggesting
    }

    // 测试目标保留 mention 过滤选项观测入口，供输入交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn filtered_options(&self) -> &[String] {
        &self.filtered
    }

    fn interaction_frame(&self) -> Rect {
        self.last_frame.get().unwrap_or_else(|| {
            let size = self.intrinsic_size();
            Rect::new(0.0, 0.0, size.w, size.h)
        })
    }

    fn popup_local_rect(&self) -> Rect {
        // 读取事件路径使用的本地触发器 frame。
        let frame = self.interaction_frame();
        // 返回当前表面解析后的最终本地弹层。
        self.interaction_popup_rect(frame, self.filtered.len())
    }

    fn dropdown_row_at(&self, pos: Point) -> Option<usize> {
        let popup = self.popup_local_rect();
        if !point_in_half_open_rect(popup, pos) {
            return None;
        }
        let local_y = pos.y - popup.y + self.dropdown_scroll.scroll_offset();
        // 负向内容坐标不对应任何候选行。
        if local_y < 0.0 {
            // 避免负浮点转换为无意义索引。
            return None;
        }
        let index = (local_y / SUGGESTION_ROW_HEIGHT).floor() as usize;
        (index < self.filtered.len()).then_some(index)
    }

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    fn reveal_selected(&mut self) {
        if self.selected_index >= self.filtered.len() {
            return;
        }
        // 键盘显露使用表面约束后的实际视口高度。
        let viewport_height = self.effective_popup_viewport_height(self.filtered.len());
        let old_offset = self.dropdown_scroll.scroll_offset();
        let row_top = self.selected_index as f32 * SUGGESTION_ROW_HEIGHT;
        let row_bottom = row_top + SUGGESTION_ROW_HEIGHT;
        let new_offset = if row_top < old_offset {
            row_top
        } else if row_bottom > old_offset + viewport_height {
            row_bottom - viewport_height
        } else {
            old_offset
        };
        self.dropdown_scroll.set_scroll_offset(new_offset);
        self.dropdown_scroll.clamp_to_content(
            self.filtered.len(),
            SUGGESTION_ROW_HEIGHT,
            viewport_height,
        );
        let applied = self.dropdown_scroll.scroll_offset() - old_offset;
        if applied.abs() > 0.01 {
            self.push_scroll_delta(0.0, applied);
        }
    }

    fn set_cursor_from_x(&mut self, x: f32) {
        let glyph_xs = self.glyph_xs.borrow();
        if glyph_xs.len() != self.value.chars().count() + 1 {
            self.cursor_char = self.value.chars().count();
            return;
        }
        let scale = (self.interaction_frame().h / CONTROL_HEIGHT).clamp(0.0, 1.0);
        let target = (x - 10.0 * scale + self.text_scroll_x.get()).max(0.0);
        let mut index = glyph_xs.len().saturating_sub(1);
        for candidate in 0..glyph_xs.len().saturating_sub(1) {
            let midpoint = (glyph_xs[candidate] + glyph_xs[candidate + 1]) * 0.5;
            if target < midpoint {
                index = candidate;
                break;
            }
        }
        self.cursor_char = index;
    }

    fn insert_text(&mut self, text: &str) -> bool {
        if text.is_empty() || text.chars().any(char::is_control) {
            return false;
        }
        let byte_index = byte_index_for_char(&self.value, self.cursor_char);
        self.value.insert_str(byte_index, text);
        self.cursor_char += text.chars().count();
        self.refresh_suggestion_from_value();
        self.publish_change();
        true
    }

    fn delete_previous_char(&mut self) -> bool {
        if self.cursor_char == 0 || self.value.is_empty() {
            return false;
        }
        let end = byte_index_for_char(&self.value, self.cursor_char);
        let start = byte_index_for_char(&self.value, self.cursor_char - 1);
        self.value.replace_range(start..end, "");
        self.cursor_char -= 1;
        self.refresh_suggestion_from_value();
        self.publish_change();
        true
    }

    fn delete_next_char(&mut self) -> bool {
        let char_count = self.value.chars().count();
        if self.cursor_char >= char_count {
            return false;
        }
        let start = byte_index_for_char(&self.value, self.cursor_char);
        let end = byte_index_for_char(&self.value, self.cursor_char + 1);
        self.value.replace_range(start..end, "");
        self.refresh_suggestion_from_value();
        self.publish_change();
        true
    }

    fn refresh_suggestion_from_value(&mut self) {
        let cursor_byte = byte_index_for_char(&self.value, self.cursor_char);
        let prefix = &self.value[..cursor_byte];
        let Some(trigger_start) = prefix.rfind(&self.trigger) else {
            self.stop_suggesting();
            return;
        };
        let query_start = trigger_start + self.trigger.len();
        let query = &prefix[query_start..];
        if query.chars().any(char::is_whitespace) {
            self.stop_suggesting();
            return;
        }

        let query = query.to_owned();
        // 记录有效活动查询是否开始新的呈现周期。
        let starts_presentation = !self.suggesting;
        // 只有新呈现周期才丢弃上一周期的弹层历史。
        if starts_presentation {
            // 新呈现周期重新收集弹层脏区。
            self.popup_damage_rect.set(Rect::zero());
            // 新呈现周期等待当前帧重新解析弹层。
            self.popup_row_count.set(0);
            // 丢弃上一呈现周期的相对弹层缓存。
            self.popup_rect.set(Rect::zero());
            // 等待当前帧取得最新逻辑表面。
            self.surface_rect.set(None);
            // 等待当前帧取得最新绝对锚点。
            self.popup_anchor_frame.set(None);
        }
        // 新呈现或光标查询变化都需要刷新候选集合。
        let query_changed = starts_presentation || self.search_text != query;
        self.search_text = query;
        self.suggesting = true;
        if query_changed {
            self.update_filtered();
        }
    }

    fn stop_suggesting(&mut self) {
        self.suggesting = false;
        self.search_text.clear();
        self.filtered.clear();
        self.selected_index = 0;
        self.hovered_option = None;
        self.dropdown_scroll.set_scroll_offset(0.0);
        self.scroll_delta_strip.set((0.0, 0.0));
    }

    fn update_filtered(&mut self) {
        if self.search_text.is_empty() {
            self.filtered = self.options.clone();
        } else {
            let query = self.search_text.to_lowercase();
            self.filtered = self
                .options
                .iter()
                .filter(|option| option.to_lowercase().contains(&query))
                .cloned()
                .collect();
        }
        self.selected_index = 0;
        self.hovered_option = None;
        self.dropdown_scroll.set_scroll_offset(0.0);
        self.scroll_delta_strip.set((0.0, 0.0));
    }

    fn active_replacement_range(&self) -> Option<(usize, usize)> {
        let cursor_byte = byte_index_for_char(&self.value, self.cursor_char);
        let prefix = &self.value[..cursor_byte];
        let start = prefix.rfind(&self.trigger)?;
        let query_start = start + self.trigger.len();
        if prefix[query_start..].chars().any(char::is_whitespace) {
            return None;
        }

        let mut end = cursor_byte;
        for (offset, ch) in self.value[cursor_byte..].char_indices() {
            end = cursor_byte + offset + ch.len_utf8();
            if ch.is_whitespace() {
                break;
            }
        }
        Some((start, end))
    }

    fn select_index(&mut self, index: usize) -> bool {
        let Some(selected) = self.filtered.get(index).cloned() else {
            return false;
        };
        let Some((start, end)) = self.active_replacement_range() else {
            self.stop_suggesting();
            return false;
        };
        let prefix_chars = self.value[..start].chars().count();
        let replacement = format!("{}{} ", self.trigger, selected);
        let replacement_chars = replacement.chars().count();
        self.value.replace_range(start..end, &replacement);
        self.cursor_char = prefix_chars + replacement_chars;
        self.publish_change();
        self.stop_suggesting();
        true
    }

    fn publish_change(&self) {
        self.pending_change.replace(Some(self.value.clone()));
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Mentions {
            placeholder: self.placeholder.clone(),
            options: self.options.clone(),
            value: self.value.clone(),
            suggesting: self.suggesting,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let options_changed = self.options != next.options;
        self.placeholder = next.placeholder;
        self.options = next.options;
        self.cursor_char = self.cursor_char.min(self.value.chars().count());
        if self.suggesting && options_changed {
            self.update_filtered();
        }
    }
}

fn byte_index_for_char(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(value.len())
}

fn point_in_half_open_rect(rect: Rect, point: Point) -> bool {
    point.x >= rect.x && point.x < rect.x + rect.w && point.y >= rect.y && point.y < rect.y + rect.h
}

// 将 Mentions 表面约束契约放在独立测试文件中。
#[cfg(test)]
// 使用显式路径保持主实现文件低于行数上限。
#[path = "mentions_tests.rs"]
// 声明当前模块的私有回归测试。
mod tests;

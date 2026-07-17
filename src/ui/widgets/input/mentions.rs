//! Mentions 提及输入组件 — 输入 @ 触发下拉建议列表。
//!
//! 基于 Input 交互模式，增加触发字符检测和建议弹出。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::{EventResult, KeyCode, MouseButton, SnapshotFields, SystemEvent, WidgetTree};
use std::cell::Cell;

const SUGGESTION_ROW_HEIGHT: f32 = 32.0;
const MAX_VISIBLE_SUGGESTIONS: usize = 5;

component! {
    /// Mentions — @ 提及输入框。
    ///
    /// 输入 @ 字符后弹出建议列表，选择后插入完整提及文本。
    pub struct Mentions {
        /// 当前输入文本
        value: String,
        /// 占位文本
        placeholder: String,
        /// 选项列表
        options: Vec<String>,
        /// 过滤后的建议列表
        filtered: Vec<String>,
        /// 是否正在触发建议
        suggesting: bool,
        /// 触发字符（默认 @）
        trigger: String,
        /// 光标在 trigger 后的搜索文本
        search_text: String,
        /// 建议列表选中索引
        selected_index: usize,
        /// 焦点状态
        focused: bool,
        /// 平台输入法候选窗锚点
        cursor_rect: Cell<Rect>,
    }

    tab_index => (&self) -> i32 { 1 }

    accepts_text_input => (&self) -> bool { true }

    text_input_cursor_rect => (&self) -> Rect { self.cursor_rect.get() }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        mentions_dirty_rect(frame, self.options.len())
    }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.suggesting && !self.filtered.is_empty() {
            mentions_dirty_rect(frame, self.visible_suggestion_count())
        } else {
            frame
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if self.suggesting && pos.y > 34.0 {
                    let index = ((pos.y - 34.0) / SUGGESTION_ROW_HEIGHT) as usize;
                    if index < self.filtered.len().min(MAX_VISIBLE_SUGGESTIONS) {
                        self.selected_index = index;
                        self.select_current();
                        return EventResult::Handled;
                    }
                }
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::PointerMove { .. } => EventResult::NotHandled,
            SystemEvent::FocusIn => { self.focused = true; EventResult::Handled }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.stop_suggesting();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Backspace => {
                        if self.value.pop().is_none() {
                            return EventResult::NotHandled;
                        }
                        self.refresh_suggestion_from_value();
                        EventResult::Handled
                    }
                    KeyCode::Enter => {
                        if self.suggesting && !self.filtered.is_empty() {
                            self.select_current();
                            EventResult::Handled
                        } else {
                            EventResult::Handled
                        }
                    }
                    KeyCode::Up | KeyCode::Down => {
                        if self.suggesting && !self.filtered.is_empty() {
                            let len = self.filtered.len();
                            if *key == KeyCode::Up {
                                self.selected_index = (self.selected_index + len - 1) % len;
                            } else {
                                self.selected_index = (self.selected_index + 1) % len;
                            }
                            EventResult::Handled
                        } else {
                            EventResult::NotHandled
                        }
                    }
                    _ => EventResult::NotHandled,
                }
            }
            SystemEvent::TextInput { text } => {
                if text.is_empty() || text.chars().any(char::is_control) {
                    return EventResult::NotHandled;
                }
                self.value.push_str(text);
                self.refresh_suggestion_from_value();
                EventResult::Handled
            }
            SystemEvent::Paste { text } => {
                if text.is_empty() || text.chars().any(char::is_control) {
                    return EventResult::NotHandled;
                }
                self.value.push_str(text);
                self.refresh_suggestion_from_value();
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let primary = ctx.tokens().color_primary();
        let border_color = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let bg_elevated = ctx.tokens().color_bg_elevated();
        let border_radius_sm = ctx.tokens().border_radius_sm();
        let radius = Some(crate::draw::Radius::uniform(border_radius_sm));

        // 输入框背景
        ctx.fill_rect(frame, ctx.tokens().color_bg_container(), radius);
        ctx.stroke_rect(frame, if self.focused { primary } else { border_color },
            if self.focused { 2.0 } else { 1.0 }, radius);

        // 显示占位符或文本
        let display = if self.value.is_empty() { &self.placeholder } else { &self.value };
        let color = if self.value.is_empty() { text_tertiary } else { text_color };
        let draw_y = ctx.visual_center_y(frame, 14.0);
        ctx.draw_text(display, Point::new(frame.x + 12.0, draw_y),
            color, 14.0);
        let cursor_x = (frame.x + 12.0 + self.value.chars().count() as f32 * 7.0)
            .min(frame.x + frame.w - 12.0);
        let cursor_rect = Rect::new(cursor_x, frame.y + 7.0, 1.0, 18.0);
        self.cursor_rect.set(cursor_rect);
        if self.focused {
            ctx.fill_rect(cursor_rect, primary, None);
        }

        // 建议弹出层
        if self.suggesting && !self.filtered.is_empty() {
            let popup_h = self.visible_suggestion_count() as f32 * SUGGESTION_ROW_HEIGHT;
            let popup = Rect::new(frame.x, frame.y + frame.h + 2.0, frame.w, popup_h);
            ctx.fill_rect(popup, bg_elevated, radius);
            ctx.stroke_rect(popup, border_color, 1.0, radius);

            for (i, opt) in self.filtered.iter().take(MAX_VISIBLE_SUGGESTIONS).enumerate() {
                let y = popup.y + i as f32 * SUGGESTION_ROW_HEIGHT;
                let item_rect = Rect::new(popup.x, y, popup.w, SUGGESTION_ROW_HEIGHT);
                if i == self.selected_index {
                    ctx.fill_rect(item_rect,
                        ctx.tokens().color_primary_bg(), None);
                }
                let opt_y = ctx.visual_center_y(item_rect, 14.0);
                ctx.draw_text(opt, Point::new(popup.x + 12.0, opt_y),
                    text_color, 14.0);
            }
        }
    }
}

impl Mentions {
    fn intrinsic_size(&self) -> Size {
        Size::new(80.0, 32.0)
    }

    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            value: String::new(),
            placeholder: placeholder.into(),
            options: Vec::new(),
            filtered: Vec::new(),
            suggesting: false,
            trigger: "@".to_string(),
            search_text: String::new(),
            selected_index: 0,
            focused: false,
            cursor_rect: Cell::new(Rect::zero()),
        }
    }

    /// 设置建议选项列表
    pub fn options(mut self, opts: Vec<impl Into<String>>) -> Self {
        self.options = opts.into_iter().map(|s| s.into()).collect();
        self
    }

    pub fn value(&self) -> &str {
        &self.value
    }
    #[cfg(test)]
    pub(crate) fn is_suggesting(&self) -> bool {
        self.suggesting
    }

    #[cfg(test)]
    pub(crate) fn filtered_options(&self) -> &[String] {
        &self.filtered
    }

    fn visible_suggestion_count(&self) -> usize {
        self.filtered.len().min(MAX_VISIBLE_SUGGESTIONS)
    }

    fn refresh_suggestion_from_value(&mut self) {
        let Some(position) = self.value.rfind(&self.trigger) else {
            self.stop_suggesting();
            return;
        };
        let query_start = position + self.trigger.len();
        let query = &self.value[query_start..];
        if query.chars().any(char::is_whitespace) {
            self.stop_suggesting();
            return;
        }

        self.search_text = query.to_owned();
        self.suggesting = true;
        self.update_filtered();
    }

    fn stop_suggesting(&mut self) {
        self.suggesting = false;
        self.search_text.clear();
        self.filtered.clear();
        self.selected_index = 0;
    }

    fn update_filtered(&mut self) {
        if self.search_text.is_empty() {
            self.filtered = self.options.clone();
        } else {
            let lower = self.search_text.to_lowercase();
            self.filtered = self
                .options
                .iter()
                .filter(|o| o.to_lowercase().contains(&lower))
                .cloned()
                .collect();
        }
        self.selected_index = 0;
    }

    fn select_current(&mut self) {
        if let Some(selected) = self.filtered.get(self.selected_index) {
            // 替换最后一段 trigger + query 为 trigger + selected。
            if let Some(pos) = self.value.rfind(&self.trigger) {
                self.value.truncate(pos + self.trigger.len());
                self.value.push_str(selected);
                self.value.push(' ');
            }
            self.stop_suggesting();
        }
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
        self.placeholder = next.placeholder;
        self.options = next.options;
        if self.suggesting {
            self.refresh_suggestion_from_value();
        }
    }
}

fn mentions_dirty_rect(frame: Rect, option_count: usize) -> Rect {
    let popup_height = option_count.min(MAX_VISIBLE_SUGGESTIONS) as f32 * SUGGESTION_ROW_HEIGHT;
    let popup = Rect::new(frame.x, frame.y + frame.h + 2.0, frame.w, popup_height);
    frame.union(&popup)
}

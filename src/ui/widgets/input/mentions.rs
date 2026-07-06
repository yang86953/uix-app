//! Mentions 提及输入组件 — 输入 @ 触发下拉建议列表。
//!
//! 基于 Input 交互模式，增加触发字符检测和建议弹出。

use crate::core::{Point, Rect, Size};
use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::draw::{traits::GraphicsEngine, Color};
use crate::ui::{EventResult, KeyCode, SystemEvent, WidgetTree};

define_widget! {
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
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(80.0, 32.0)
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { .. } => { self.focused = true; EventResult::Handled }
            SystemEvent::PointerMove { .. } => EventResult::NotHandled,
            SystemEvent::FocusOut => { self.focused = false; self.suggesting = false; EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Backspace => {
                        if self.suggesting {
                            // 在建议模式中退格
                            if self.search_text.is_empty() {
                                self.suggesting = false;
                            } else {
                                self.search_text.pop();
                                self.update_filtered();
                            }
                            EventResult::Handled
                        } else {
                            self.value.pop();
                            // 检查是否退格到 @
                            if self.value.ends_with(&self.trigger) {
                                self.suggesting = true;
                                self.search_text.clear();
                                self.update_filtered();
                            }
                            EventResult::Handled
                        }
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
                if text.chars().any(|c| c.is_control()) {
                    return EventResult::NotHandled;
                }
                for ch in text.chars() {
                    if self.suggesting {
                        if ch == ' ' || ch == '\n' {
                            // 空格/换行结束建议
                            self.value.push_str(&self.search_text);
                            self.value.push(ch);
                            self.suggesting = false;
                        } else {
                            self.search_text.push(ch);
                            self.update_filtered();
                        }
                    } else {
                        self.value.push(ch);
                        if ch == '@' {
                            self.suggesting = true;
                            self.search_text.clear();
                            self.selected_index = 0;
                            self.update_filtered();
                        }
                    }
                }
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
        ctx.fill_rect(frame, Color::white(), radius);
        ctx.stroke_rect(frame, if self.focused { primary } else { border_color },
            if self.focused { 2.0 } else { 1.0 }, radius);

        // 显示占位符或文本
        let display = if self.value.is_empty() { &self.placeholder } else { &self.value };
        let color = if self.value.is_empty() { text_tertiary } else { text_color };
        let draw_y = ctx.visual_center_y(frame, 14.0);
        ctx.draw_text(display, Point::new(frame.x + 12.0, draw_y),
            color, 14.0);

        // 建议弹出层
        if self.suggesting && !self.filtered.is_empty() {
            let popup_h = (self.filtered.len() as f32 * 32.0).min(160.0);
            let popup = Rect::new(frame.x, frame.y + frame.h + 2.0, frame.w, popup_h);
            ctx.fill_rect(popup, bg_elevated, radius);
            ctx.stroke_rect(popup, border_color, 1.0, radius);

            for (i, opt) in self.filtered.iter().enumerate() {
                let y = popup.y + i as f32 * 32.0;
                let item_rect = Rect::new(popup.x, y, popup.w, 32.0);
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
            // 替换 @search_text 为 @selected
            // 找到最后一个 @
            if let Some(pos) = self.value.rfind('@') {
                self.value.truncate(pos + 1); // 保留 @
                self.value.push_str(selected);
                self.value.push(' ');
            }
            self.suggesting = false;
        }
    }
}

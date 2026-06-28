//! AutoComplete widget — 自动完成输入框，Ant Design 风格。
//!
//! 输入时弹出匹配选项列表，支持键盘导航选择。

use uix_platform::{Point, Rect, Size};
use crate::define_widget;
use uix_graphics::Radius;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};

// AutoComplete — 自动完成输入框。
define_widget! {
    pub struct AutoComplete {
        placeholder: String,
        value: String,
        options: Vec<String>,
        filtered: Vec<String>,
        open: bool,
        focus: bool,
        hovered: bool,
        selected_idx: usize,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(200.0, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                if pos.y >= 0.0 && pos.y <= 32.0 {
                    self.focus = true;
                    self.open = true;
                    self.filter();
                    return EventResult::Handled;
                }
                // 点击选项
                if self.open && pos.y > 32.0 {
                    let idx = ((pos.y - 32.0) / 28.0) as usize;
                    if idx < self.filtered.len() {
                        self.value = self.filtered[idx].clone();
                        self.open = false;
                        return EventResult::Handled;
                    }
                }
                self.open = false;
                EventResult::NotHandled
            }
            WidgetEvent::HoverEnter => { self.hovered = true; EventResult::Handled }
            WidgetEvent::HoverLeave => { self.hovered = false; EventResult::Handled }
            WidgetEvent::KeyDown { key, .. } => {
                match key {
                    crate::widget::KeyCode::Down if self.open => {
                        self.selected_idx = (self.selected_idx + 1).min(self.filtered.len().saturating_sub(1));
                    }
                    crate::widget::KeyCode::Up if self.open => {
                        self.selected_idx = self.selected_idx.saturating_sub(1);
                    }
                    crate::widget::KeyCode::Enter if self.open => {
                        if self.selected_idx < self.filtered.len() {
                            self.value = self.filtered[self.selected_idx].clone();
                            self.open = false;
                        }
                    }
                    crate::widget::KeyCode::Escape => { self.open = false; }
                    _ => self.open = true,
                }
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_quaternary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius()));
        let input_rect = Rect::new(frame.x, frame.y, frame.w, 32.0);
        let bc = if self.focus { primary } else { border };
        ctx.fill_rect(input_rect, bg, r);
        ctx.stroke_rect(input_rect, bc, if self.focus { 2.0 } else { 1.0 }, r);
        let display = if self.value.is_empty() { &self.placeholder } else { &self.value };
        let disp_c = if self.value.is_empty() { text_sec } else { text };
        let draw_y = ctx.visual_center_y(input_rect, 13.0);
        ctx.draw_text(display, Point::new(frame.x + 10.0, draw_y), disp_c, 13.0);

        // 下拉选项
        if self.open && !self.filtered.is_empty() {
            let menu_h = self.filtered.len() as f32 * 28.0;
            let menu_rect = Rect::new(frame.x, frame.y + 32.0, frame.w, menu_h);
            let fill = ctx.tokens().color_fill_tertiary();
            let bg_elev = ctx.tokens().color_bg_elevated();
            ctx.fill_rect(menu_rect, bg_elev, Some(Radius::uniform(ctx.tokens().border_radius_sm())));
            ctx.stroke_rect(menu_rect, border, 1.0, Some(Radius::uniform(ctx.tokens().border_radius_sm())));
            for (i, opt) in self.filtered.iter().enumerate() {
                let opt_y = frame.y + 32.0 + i as f32 * 28.0;
                let item_rect = Rect::new(frame.x, opt_y, frame.w, 28.0);
                if i == self.selected_idx {
                    ctx.fill_rect(item_rect, fill, None);
                }
                let py = ctx.visual_center_y(item_rect, 13.0);
                ctx.draw_text(opt, Point::new(frame.x + 10.0, py), text, 13.0);
            }
        }
    }
}

impl AutoComplete {
    pub fn new() -> Self {
        Self {
            placeholder: String::new(), value: String::new(), options: Vec::new(),
            filtered: Vec::new(), open: false, focus: false, hovered: false, selected_idx: 0,
        }
    }
    pub fn placeholder(mut self, p: &str) -> Self { self.placeholder = p.to_string(); self }
    pub fn options(mut self, opts: Vec<impl Into<String>>) -> Self {
        self.options = opts.into_iter().map(|s| s.into()).collect(); self
    }
    pub fn value(&self) -> &str { &self.value }
    pub fn set_value(&mut self, v: &str) { self.value = v.to_string(); }
    fn filter(&mut self) {
        if self.value.is_empty() {
            self.filtered = self.options.clone();
        } else {
            self.filtered = self.options.iter().filter(|o| o.contains(&self.value)).cloned().collect();
        }
        self.selected_idx = 0;
    }
}

impl Default for AutoComplete { fn default() -> Self { Self::new() } }

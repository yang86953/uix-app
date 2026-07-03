//! Rate widget — 星级评分，支持半星、hover 预览、disabled、clearable。

use crate::define_widget;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, KeyCode, WidgetEvent, WidgetTree};
use uix_graphics::GraphicsEngine;
use uix_platform::{Point, Rect, Size};

define_widget! {
    /// Rate — 星级评分，点击选择分值。
    pub struct Rate {
        count: usize,
        value: usize,
        half: bool,
        disabled: bool,
        clearable: bool,
        hover_value: usize,
        focused: bool,
        on_change: Option<Box<dyn FnMut(usize) + 'static>>,
        character: String,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(self.count as f32 * 24.0, 24.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                let star_idx = (pos.x / 24.0) as usize;
                if star_idx < self.count {
                    let new_val = if self.half {
                        let in_star_x = pos.x - star_idx as f32 * 24.0;
                        star_idx * 2 + if in_star_x < 12.0 { 1 } else { 2 }
                    } else {
                        star_idx + 1
                    };
                    // clearable: 点击同一个值取消
                    if self.clearable && new_val == self.value {
                        self.value = 0;
                    } else {
                        self.value = new_val;
                    }
                    if let Some(ref mut cb) = self.on_change { cb(self.value); }
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            WidgetEvent::MouseMove { pos, .. } => {
                let star_idx = (pos.x / 24.0) as usize;
                if star_idx < self.count {
                    if self.half {
                        let in_star_x = pos.x - star_idx as f32 * 24.0;
                        self.hover_value = star_idx * 2 + if in_star_x < 12.0 { 1 } else { 2 };
                    } else {
                        self.hover_value = star_idx + 1;
                    }
                } else {
                    self.hover_value = 0;
                }
                EventResult::Handled
            }
            WidgetEvent::HoverLeave => { self.hover_value = 0; EventResult::Handled }
            WidgetEvent::FocusIn => { self.focused = true; EventResult::Handled }
            WidgetEvent::FocusOut => { self.focused = false; EventResult::Handled }
            WidgetEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Right | KeyCode::Up => {
                        let max_val = if self.half { self.count * 2 } else { self.count };
                        if self.value < max_val {
                            self.value += 1;
                            if let Some(ref mut cb) = self.on_change { cb(self.value); }
                        }
                        EventResult::Handled
                    }
                    KeyCode::Left | KeyCode::Down => {
                        if self.value > 0 {
                            self.value -= 1;
                            if let Some(ref mut cb) = self.on_change { cb(self.value); }
                        }
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let warning = ctx.tokens().color_warning();
        let fill_tertiary = ctx.tokens().color_fill_tertiary();
        let text_quaternary = ctx.tokens().color_text_quaternary();
        let bg = ctx.tokens().color_bg_container();
        let ch = if self.character.is_empty() { "★" } else { &self.character };

        // hover 预览值优先于选中值
        let display_val = if self.hover_value > 0 { self.hover_value } else { self.value };

        for i in 0..self.count {
            let sx = frame.x + i as f32 * 24.0;
            let star_rect = Rect::new(sx, frame.y, 24.0, 24.0);
            let sy = ctx.visual_center_y(star_rect, 18.0);
            let filled = if self.half {
                display_val >= i * 2 + 2
            } else {
                display_val > i
            };

            let star_color = if self.disabled {
                if filled { ctx.tokens().color_warning_border() } else { text_quaternary }
            } else if filled {
                warning
            } else {
                fill_tertiary
            };

            ctx.draw_text(ch, Point::new(sx + 2.0, sy), star_color, 18.0);

            // 半星支持：左半填充
            if self.half && display_val == i * 2 + 1 {
                ctx.draw_text(ch, Point::new(sx + 2.0, sy), star_color, 18.0);
                ctx.fill_rect(Rect::new(sx + 14.0, sy, 10.0, 18.0), bg, None);
            }
        }
    }
}

impl Default for Rate {
    fn default() -> Self {
        Self::new()
    }
}

impl Rate {
    pub fn new() -> Self {
        Self {
            count: 5,
            value: 0,
            half: false,
            disabled: false,
            clearable: false,
            hover_value: 0,
            focused: false,
            on_change: None,
            character: String::new(),
        }
    }
    pub fn count(mut self, n: usize) -> Self {
        self.count = n;
        self
    }
    pub fn value(mut self, v: usize) -> Self {
        self.value = v;
        self
    }
    pub fn allow_half(mut self) -> Self {
        self.half = true;
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn clearable(mut self) -> Self {
        self.clearable = true;
        self
    }
    pub fn on_change<F: FnMut(usize) + 'static>(mut self, f: F) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
    pub fn character(mut self, c: impl Into<String>) -> Self {
        self.character = c.into();
        self
    }
}

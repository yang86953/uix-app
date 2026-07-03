//! Checkbox — checkbox with label, checked/unchecked state.

use crate::define_widget;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, KeyCode, WidgetEvent, WidgetTree};
use uix_graphics::Color;
use uix_platform::{Rect, Size};

define_widget! {
    pub struct Checkbox {
        checked: bool,
        disabled: bool,
        label: String,
        hovered: bool,
        focused: bool,
        on_change: Option<Box<dyn FnMut(bool) + 'static>>,
    }


    tab_index => (&self) -> i32 { 1 }
    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        let text_w = self.label.len() as f32 * 8.0;
        Size::new(22.0 + text_w, 22.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        match event {
            WidgetEvent::MouseDown { .. } => {
                self.checked = !self.checked;
                self.focused = true;
                if let Some(ref mut cb) = self.on_change { cb(self.checked); }
                EventResult::Handled
            }
            WidgetEvent::HoverEnter => { self.hovered = true; EventResult::Handled }
            WidgetEvent::HoverLeave => { self.hovered = false; EventResult::Handled }
            WidgetEvent::FocusIn => { self.focused = true; EventResult::Handled }
            WidgetEvent::FocusOut => { self.focused = false; EventResult::Handled }
            WidgetEvent::KeyDown { key, .. } => {
                if *key == KeyCode::Space || *key == KeyCode::Enter {
                    self.checked = !self.checked;
                    if let Some(ref mut cb) = self.on_change { cb(self.checked); }
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            _ => EventResult::NotHandled
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let primary_border = ctx.tokens().color_primary_border();
        let border_c = ctx.tokens().color_border();
        let border_sec = ctx.tokens().color_border_secondary();
        let text_c = if self.disabled { ctx.tokens().color_text_quaternary() } else { ctx.tokens().color_text() };
        let white = Color::white();

        let box_size = 16.0;
        let box_x = frame.x;
        let box_y = frame.y + (frame.h - box_size) * 0.5;
        let box_r = Rect::new(box_x, box_y, box_size, box_size);
        let corner = Some(uix_graphics::Radius::uniform(3.0));

        if self.checked {
            let bg = if self.disabled { primary_border } else if self.hovered { primary_hover } else { primary };
            ctx.fill_rect(box_r, bg, corner);
            let cx = box_x + box_size * 0.5;
            let cy = box_y + box_size * 0.5;
            ctx.canvas_2d().draw_line(cx - 4.0, cy, cx - 1.0, cy + 3.0, white, 2.0);
            ctx.canvas_2d().draw_line(cx - 1.0, cy + 3.0, cx + 4.0, cy - 2.0, white, 2.0);
        } else {
            let border = if self.disabled { border_sec } else if self.hovered { primary_hover } else { border_c };
            ctx.stroke_rect(box_r, border, 1.5, corner);
        }

        if self.focused {
            ctx.stroke_rect(Rect::new(box_x - 1.0, box_y - 1.0, box_size + 2.0, box_size + 2.0), primary, 1.5, Some(uix_graphics::Radius::uniform(4.0)));
        }

        let label_rect = Rect::new(box_x + box_size + 6.0, frame.y, frame.w - box_x - box_size - 6.0, frame.h);
        ctx.text_center(&self.label, label_rect, text_c, 13.0);
    }
}

impl Default for Checkbox {
    fn default() -> Self {
        Self::new("")
    }
}

impl Checkbox {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            checked: false,
            disabled: false,
            label: label.into(),
            hovered: false,
            focused: false,
            on_change: None,
        }
    }
    pub fn checked(mut self, v: bool) -> Self {
        self.checked = v;
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn is_checked(&self) -> bool {
        self.checked
    }
    pub fn on_change<F: FnMut(bool) + 'static>(mut self, f: F) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
}

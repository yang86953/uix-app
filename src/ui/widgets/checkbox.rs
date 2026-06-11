//! Checkbox — checkbox with label, checked/unchecked state.

use crate::define_widget;
use crate::graphics::Color;
use crate::base::{Point, Rect, Size};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

define_widget! {
    pub struct Checkbox {
        checked: bool,
        disabled: bool,
        label: String,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        let text_w = self.label.len() as f32 * 8.0;
        Size::new(22.0 + text_w, 22.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { .. } if !self.disabled => {
                self.checked = !self.checked;
                EventResult::Handled
            }
            _ => EventResult::NotHandled
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // 预先提取所有颜色值，避免 ctx.tokens() 与 ctx.engine() 的借用冲突
        let primary = ctx.tokens().color_primary();
        let primary_border = ctx.tokens().color_primary_border();
        let border_c = ctx.tokens().color_border();
        let border_sec = ctx.tokens().color_border_secondary();
        let text_c = if self.disabled { ctx.tokens().color_text_quaternary() } else { ctx.tokens().color_text() };
        let white = Color::white();
        let transparent = Color::transparent();

        let box_size = 16.0;
        let box_x = frame.x;
        let box_y = frame.y + (frame.h - box_size) * 0.5;
        let box_r = Rect::new(box_x, box_y, box_size, box_size);
        let corner = Some(crate::graphics::Radius::uniform(3.0));

        if self.checked {
            let bg = if self.disabled { primary_border } else { primary };
            ctx.fill_rect(box_r, bg, corner);
            let cx = box_x + box_size * 0.5;
            let cy = box_y + box_size * 0.5;
            ctx.engine().draw_line(cx - 4.0, cy, cx - 1.0, cy + 3.0, white, 2.0);
            ctx.engine().draw_line(cx - 1.0, cy + 3.0, cx + 4.0, cy - 2.0, white, 2.0);
        } else {
            let border = if self.disabled { border_sec } else { border_c };
            ctx.stroke_rect(box_r, border, 1.5, corner);
            ctx.fill_rect(box_r, transparent, None);
        }

        ctx.draw_text(&self.label, Point::new(box_x + box_size + 6.0, frame.y + 3.0), text_c, 13.0);
    }
}

impl Default for Checkbox { fn default() -> Self { Self::new("") } }

impl Checkbox {
    pub fn new(label: impl Into<String>) -> Self {
        Self { checked: false, disabled: false, label: label.into() }
    }
    pub fn checked(mut self, v: bool) -> Self { self.checked = v; self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn is_checked(&self) -> bool { self.checked }
}

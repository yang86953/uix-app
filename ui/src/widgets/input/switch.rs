//! Switch — on/off toggle switch.

use crate::define_widget;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, KeyCode, WidgetEvent, WidgetTree};
use uix_platform::{Rect, Size};

define_widget! {
    pub struct Switch {
        checked: bool,
        disabled: bool,
        size: f32,
        hovered: bool,
        focused: bool,
        on_change: Option<Box<dyn FnMut(bool) + 'static>>,
    }


    tab_index => (&self) -> i32 { 1 }
    preferred_size => (&self, _engine: Option<&dyn uix_graphics::traits::GraphicsEngine>) -> Size {
        Size::new(self.size * 2.0 - 4.0, self.size + 4.0)
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
        let fill_sec = ctx.tokens().color_fill_secondary();
        let fill_ter = ctx.tokens().color_fill_tertiary();
        let bg_container = ctx.tokens().color_bg_container();
        let bg_elevated = ctx.tokens().color_bg_elevated();

        let h = frame.h;
        let w = frame.w;
        let track_r = h * 0.5;
        let thumb_r = track_r - 3.0;
        let thumb_x = if self.checked { frame.x + w - 2.0 - thumb_r * 2.0 } else { frame.x + 2.0 };

        let track_c = if self.checked {
            if self.disabled { primary_border } else if self.hovered { primary_hover } else { primary }
        } else {
            if self.disabled { fill_ter } else { fill_sec }
        };
        let thumb_c = if self.disabled { bg_container } else { bg_elevated };

        let radius = Some(uix_graphics::Radius::uniform(track_r));
        ctx.fill_rect(Rect::new(frame.x, frame.y, w, h), track_c, radius);
        let thumb = Rect::new(thumb_x, frame.y + 2.0, thumb_r * 2.0, thumb_r * 2.0);
        ctx.fill_rect(thumb, thumb_c, Some(uix_graphics::Radius::uniform(thumb_r)));

        if self.focused {
            ctx.stroke_rect(Rect::new(frame.x - 1.0, frame.y - 1.0, w + 2.0, h + 2.0), primary, 1.5, radius);
        }
    }
}

impl Default for Switch {
    fn default() -> Self {
        Self::new()
    }
}

impl Switch {
    pub fn new() -> Self {
        Self {
            checked: false,
            disabled: false,
            size: 22.0,
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

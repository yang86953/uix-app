//! Switch — on/off toggle switch.

use crate::define_widget;
use crate::base::{Rect, Size};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

define_widget! {
    pub struct Switch {
        checked: bool,
        disabled: bool,
        size: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(self.size * 2.0 - 4.0, self.size + 4.0)
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
        let primary = ctx.tokens().color_primary();
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
            if self.disabled { primary_border } else { primary }
        } else {
            if self.disabled { fill_ter } else { fill_sec }
        };
        let thumb_c = if self.disabled { bg_container } else { bg_elevated };

        let radius = Some(crate::graphics::Radius::uniform(track_r));
        ctx.fill_rect(Rect::new(frame.x, frame.y, w, h), track_c, radius);
        let thumb = Rect::new(thumb_x, frame.y + 2.0, thumb_r * 2.0, thumb_r * 2.0);
        ctx.fill_rect(thumb, thumb_c, Some(crate::graphics::Radius::uniform(thumb_r)));
    }
}

impl Default for Switch {
    fn default() -> Self { Self::new() }
}

impl Switch {
    pub fn new() -> Self {
        Self { checked: false, disabled: false, size: 22.0 }
    }
    pub fn checked(mut self, v: bool) -> Self { self.checked = v; self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn is_checked(&self) -> bool { self.checked }
}

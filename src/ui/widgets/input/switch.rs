//! Switch — on/off toggle switch.

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::SnapshotFields;
use crate::ui::{ComponentId, EventResult, KeyCode, SemanticEvent, SystemEvent, WidgetTree};
use std::cell::Cell;

component! {
    pub struct Switch {
        checked: bool,
        disabled: bool,
        size: f32,
        hovered: bool,
        focused: bool,
        pending_change: Cell<Option<bool>>,
    }


    tab_index => (&self) -> i32 { 1 }
    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        match event {
            SystemEvent::PointerDown { .. } => {
                self.checked = !self.checked;
                self.focused = true;
                self.pending_change.set(Some(self.checked));
                EventResult::Handled
            }
            SystemEvent::PointerEnter => { self.hovered = true; EventResult::Handled }
            SystemEvent::PointerLeave => { self.hovered = false; EventResult::Handled }
            SystemEvent::FocusIn => { self.focused = true; EventResult::Handled }
            SystemEvent::FocusOut => { self.focused = false; EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
                if *key == KeyCode::Space || *key == KeyCode::Enter {
                    self.checked = !self.checked;
                    self.pending_change.set(Some(self.checked));
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            _ => EventResult::NotHandled
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|checked| SemanticEvent::change(id, checked.to_string()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
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

        let radius = Some(crate::draw::Radius::uniform(track_r));
        ctx.fill_rect(Rect::new(frame.x, frame.y, w, h), track_c, radius);
        let thumb = Rect::new(thumb_x, frame.y + 2.0, thumb_r * 2.0, thumb_r * 2.0);
        ctx.fill_rect(thumb, thumb_c, Some(crate::draw::Radius::uniform(thumb_r)));

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
            pending_change: Cell::new(None),
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

    fn intrinsic_size(&self) -> Size {
        Size::new(self.size * 2.0 - 4.0, self.size + 4.0)
    }
}

impl Switch {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Switch {
            checked: self.checked,
            disabled: self.disabled,
            size: self.size,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.checked = next.checked;
        self.disabled = next.disabled;
        self.size = next.size;
    }
}

#[cfg(test)]
#[path = "../../../tests/ui/widgets/input/switch.rs"]
mod tests;

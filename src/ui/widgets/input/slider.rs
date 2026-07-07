//! Slider input widget.

use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::SnapshotFields;
use crate::ui::{ComponentId, EventResult, KeyCode, SemanticEvent, SystemEvent, WidgetTree};

component! {
    /// Horizontal slider.
    pub struct Slider {
        min: f32,
        max: f32,
        step: f32,
        value: f32,
        dragging: bool,
        hovered: bool,
        focused: bool,
        last_frame: Cell<Option<Rect>>,
        pending_change: Cell<Option<f32>>,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                self.dragging = true;
                self.focused = true;
                if let Some(frame) = self.last_frame.get() {
                    self.update_from_pos(pos.x, frame);
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.dragging {
                    if let Some(frame) = self.last_frame.get() {
                        self.update_from_pos(pos.x, frame);
                    }
                }
                self.hovered = true;
                EventResult::Handled
            }
            SystemEvent::PointerUp { .. } => {
                self.dragging = false;
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.hovered = false;
                self.dragging = false;
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Right | KeyCode::Up => {
                    self.set_value((self.value + self.step).min(self.max));
                    EventResult::Handled
                }
                KeyCode::Left | KeyCode::Down => {
                    self.set_value((self.value - self.step).max(self.min));
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|value| SemanticEvent::change(id, value.to_string()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame.set(Some(frame));
        if frame.w <= 0.0 || self.max <= self.min {
            return;
        }

        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let fill = ctx.tokens().color_fill_tertiary();
        let track_h = 4.0;
        let thumb_r = 6.0;
        let cy = frame.y + frame.h * 0.5;

        let pct = ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0);
        let thumb_x = frame.x + pct * (frame.w - 2.0);

        ctx.fill_rect(
            Rect::new(frame.x, cy - track_h * 0.5, frame.w, track_h),
            fill,
            Some(Radius::uniform(track_h * 0.5)),
        );
        ctx.fill_rect(
            Rect::new(frame.x, cy - track_h * 0.5, thumb_x - frame.x, track_h),
            primary,
            Some(Radius::uniform(track_h * 0.5)),
        );
        let thumb_color = if self.dragging {
            primary_hover
        } else if self.hovered {
            primary
        } else {
            Color::white()
        };
        ctx.fill_circle(thumb_x, cy, thumb_r, thumb_color);
        ctx.stroke_rect(
            Rect::new(thumb_x - thumb_r, cy - thumb_r, thumb_r * 2.0, thumb_r * 2.0),
            primary,
            2.0,
            Some(Radius::uniform(thumb_r)),
        );

        if self.focused {
            ctx.stroke_rect(frame, primary, 1.5, Some(Radius::uniform(4.0)));
        }
    }
}

impl Slider {
    fn update_from_pos(&mut self, px: f32, frame: Rect) {
        let usable_w = (frame.w - 4.0).max(1.0);
        let pct = ((px - frame.x - 2.0) / usable_w).clamp(0.0, 1.0);
        let raw = self.min + pct * (self.max - self.min);
        if self.step > 0.0 {
            let stepped = (raw / self.step).round() * self.step;
            self.set_value(stepped.clamp(self.min, self.max));
        } else {
            self.set_value(raw.clamp(self.min, self.max));
        }
    }

    fn set_value(&mut self, value: f32) {
        let value = value.clamp(self.min, self.max);
        if (value - self.value).abs() > f32::EPSILON {
            self.value = value;
            self.pending_change.set(Some(self.value));
        }
    }
}

impl Slider {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Slider {
            min: self.min,
            max: self.max,
            step: self.step,
            value: self.value,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.min = next.min;
        self.max = next.max;
        self.step = next.step;
        self.value = next.value.clamp(self.min, self.max);
    }
}

impl Default for Slider {
    fn default() -> Self {
        Self::new()
    }
}

impl Slider {
    pub fn new() -> Self {
        Self {
            min: 0.0,
            max: 100.0,
            step: 1.0,
            value: 30.0,
            dragging: false,
            hovered: false,
            focused: false,
            last_frame: Cell::new(None),
            pending_change: Cell::new(None),
        }
    }

    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = max;
        self.value = self.value.clamp(self.min, self.max);
        self
    }

    pub fn step(mut self, s: f32) -> Self {
        self.step = s;
        self
    }

    pub fn value(mut self, v: f32) -> Self {
        self.value = v.clamp(self.min, self.max);
        self
    }

    pub fn get_value(&self) -> f32 {
        self.value
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(200.0, 24.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_clamps_slider_size() {
        let measured = Slider::new().measure(Constraints::loose(Size::new(120.0, 16.0)));

        assert_eq!(measured, Size::new(120.0, 16.0));
    }
}

//! Slider input widget.

use std::cell::Cell;
use std::ops::RangeInclusive;

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::native::traits::input::ControlSize;
use crate::ui::state::State;
use crate::ui::SnapshotFields;
use crate::ui::{ComponentId, EventResult, KeyCode, SemanticEvent, SystemEvent, WidgetTree};

component! {
    /// Horizontal slider.
    pub struct Slider {
        min: f64,
        max: f64,
        step: f64,
        value: f64,
        value_binding: Option<State<f64>>,
        dragging: bool,
        hovered: bool,
        focused: bool,
        slider_size: ControlSize,
        last_frame: Cell<Option<Rect>>,
        pending_change: Cell<Option<f64>>,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_value();
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
        self.capture_bound_value_dependency();
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        if frame.w <= 0.0 || self.max <= self.min {
            return;
        }

        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let fill = ctx.tokens().color_fill_tertiary();
        let track_h = self.track_height();
        let thumb_r = self.thumb_radius();
        let cy = frame.y + frame.h * 0.5;

        let pct = ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32;
        let (track_x, track_w) = self.track_span(frame);
        let thumb_x = track_x + pct * track_w;

        ctx.fill_rect(
            Rect::new(track_x, cy - track_h * 0.5, track_w, track_h),
            fill,
            Some(Radius::uniform(track_h * 0.5)),
        );
        ctx.fill_rect(
            Rect::new(track_x, cy - track_h * 0.5, thumb_x - track_x, track_h),
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
        let (track_x, track_w) = self.track_span(frame);
        let pct = f64::from(((px - track_x) / track_w).clamp(0.0, 1.0));
        let raw = self.min + pct * (self.max - self.min);
        if self.step > 0.0 {
            let stepped = self.min + ((raw - self.min) / self.step).round() * self.step;
            self.set_value(stepped);
        } else {
            self.set_value(raw);
        }
    }

    fn set_value(&mut self, value: f64) {
        let value = self.clamp_value(value);
        if value != self.value {
            self.value = value;
            self.write_bound_value();
            self.pending_change.set(Some(self.value));
        }
    }

    fn clamp_value(&self, value: f64) -> f64 {
        if value.is_finite() {
            value.clamp(self.min, self.max)
        } else {
            self.min
        }
    }

    fn sync_bound_value(&mut self) {
        if let Some(state) = self.value_binding.as_ref() {
            self.value = self.clamp_value(state.get());
        }
    }

    fn capture_bound_value_dependency(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            let _ = state.get();
        }
    }

    fn write_bound_value(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            if state.get() != self.value {
                state.set(self.value);
            }
        }
    }

    fn control_height(&self) -> f32 {
        crate::ui::config::control_height(self.slider_size)
    }

    fn track_height(&self) -> f32 {
        match self.slider_size {
            ControlSize::Small => 3.0,
            ControlSize::Medium => 4.0,
            ControlSize::Large => 5.0,
        }
    }

    fn thumb_radius(&self) -> f32 {
        match self.slider_size {
            ControlSize::Small => 5.0,
            ControlSize::Medium => 6.0,
            ControlSize::Large => 7.5,
        }
    }

    fn track_span(&self, frame: Rect) -> (f32, f32) {
        let inset = self.thumb_radius();
        (frame.x + inset, (frame.w - inset * 2.0).max(1.0))
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
        let controlled_value = next.value_binding.as_ref().map(|_| next.value);
        self.min = next.min;
        self.max = next.max;
        self.step = next.step;
        self.value_binding = next.value_binding;
        self.value = controlled_value.unwrap_or_else(|| self.clamp_value(self.value));
        self.slider_size = next.slider_size;
    }
}

impl Default for Slider {
    fn default() -> Self {
        Self::new(0.0..=100.0)
    }
}

impl Slider {
    pub fn new(range: RangeInclusive<f64>) -> Self {
        let (min, max) = Self::normalize_range(range);
        let config = crate::ui::config::use_config();
        Self {
            min,
            max,
            step: 1.0,
            value: min,
            value_binding: None,
            dragging: false,
            hovered: false,
            focused: false,
            slider_size: config.size,
            last_frame: Cell::new(None),
            pending_change: Cell::new(None),
        }
    }

    pub fn step(mut self, step: f64) -> Self {
        self.step = if step.is_finite() && step > 0.0 {
            step
        } else {
            0.0
        };
        self
    }

    /// 将滑块值绑定到外部 `State<f64>`。
    pub fn value(mut self, state: &State<f64>) -> Self {
        self.value_binding = Some(state.clone());
        self.value = self.clamp_value(state.get());
        self
    }

    /// 设置非受控滑块的初始值。
    pub fn default_value(mut self, value: f64) -> Self {
        self.value_binding = None;
        self.value = self.clamp_value(value);
        self
    }

    pub fn current_value(&self) -> f64 {
        self.value
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.slider_size = size;
        self
    }

    fn normalize_range(range: RangeInclusive<f64>) -> (f64, f64) {
        let (start, end) = range.into_inner();
        let start = if start.is_finite() { start } else { 0.0 };
        let end = if end.is_finite() { end } else { 100.0 };
        if start <= end {
            (start, end)
        } else {
            (end, start)
        }
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(200.0, self.control_height())
    }
}

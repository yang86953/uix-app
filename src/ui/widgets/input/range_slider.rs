//! Two-thumb range slider.

use std::cell::Cell;
use std::ops::RangeInclusive;

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::api::PaintContext;
use crate::draw::{Color, Radius};
use crate::native::traits::input::ControlSize;
use crate::ui::state::State;
use crate::ui::SnapshotFields;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetTree,
};

use super::slider::decimal_places;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeSliderThumb {
    Start,
    End,
}

component! {
    /// Horizontal two-thumb range slider returned by [`Slider::range`](super::Slider::range).
    pub struct RangeSlider {
        min: f64,
        max: f64,
        step: f64,
        start_value: f64,
        end_value: f64,
        start_binding: Option<State<f64>>,
        end_binding: Option<State<f64>>,
        active_thumb: RangeSliderThumb,
        dragging: bool,
        hovered_thumb: Option<RangeSliderThumb>,
        focused: bool,
        slider_size: ControlSize,
        last_frame: Cell<Option<Rect>>,
        pending_change: Cell<Option<(f64, f64)>>,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(200.0, self.control_height()))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_values();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let Some(frame) = self.last_frame.get() else {
                    return EventResult::NotHandled;
                };
                if !frame.contains(*pos) || self.max <= self.min {
                    return EventResult::NotHandled;
                }
                self.active_thumb = self.nearest_thumb(pos.x, frame);
                self.dragging = true;
                self.focused = true;
                self.hovered_thumb = Some(self.active_thumb);
                self.update_active_from_pos(pos.x, frame);
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if let Some(frame) = self.last_frame.get() {
                    if self.dragging {
                        self.update_active_from_pos(pos.x, frame);
                        self.hovered_thumb = Some(self.active_thumb);
                    } else {
                        self.hovered_thumb = frame
                            .contains(*pos)
                            .then(|| self.nearest_thumb(pos.x, frame));
                    }
                } else {
                    self.hovered_thumb = None;
                }
                EventResult::Handled
            }
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if self.dragging {
                    if let Some(frame) = self.last_frame.get() {
                        self.update_active_from_pos(pos.x, frame);
                        self.hovered_thumb = frame
                            .contains(*pos)
                            .then(|| self.nearest_thumb(pos.x, frame));
                    }
                }
                self.dragging = false;
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.hovered_thumb = None;
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
                    self.step_active(1.0);
                    EventResult::Handled
                }
                KeyCode::Left | KeyCode::Down => {
                    self.step_active(-1.0);
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change.take().map(|(start, end)| {
            SemanticEvent::change(id, format!("{start}..{end}"))
        })
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.capture_bound_value_dependencies();
        let control_rect = Rect::new(
            frame.x,
            frame.y,
            frame.w.max(0.0),
            frame.h.max(0.0).min(self.control_height()),
        );
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, control_rect.w, control_rect.h)));
        if control_rect.w <= 0.0 || control_rect.h <= 0.0 || self.max <= self.min {
            return;
        }

        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let fill = ctx.tokens().color_fill_tertiary();
        let track_h = self.track_height(control_rect);
        let thumb_r = self.thumb_radius(control_rect);
        let cy = control_rect.y + control_rect.h * 0.5;
        let (track_x, track_w) = self.track_span(control_rect);
        let (start_x, end_x) = self.thumb_positions(control_rect);

        ctx.push_clip(control_rect);
        ctx.fill_rect(
            Rect::new(track_x, cy - track_h * 0.5, track_w, track_h),
            fill,
            Some(Radius::uniform(track_h * 0.5)),
        );
        ctx.fill_rect(
            Rect::new(start_x, cy - track_h * 0.5, (end_x - start_x).max(0.0), track_h),
            primary,
            Some(Radius::uniform(track_h * 0.5)),
        );

        let top_thumb = if self.dragging {
            self.active_thumb
        } else {
            self.hovered_thumb.unwrap_or(self.active_thumb)
        };
        let bottom_thumb = match top_thumb {
            RangeSliderThumb::Start => RangeSliderThumb::End,
            RangeSliderThumb::End => RangeSliderThumb::Start,
        };
        for thumb in [bottom_thumb, top_thumb] {
            let x = match thumb {
                RangeSliderThumb::Start => start_x,
                RangeSliderThumb::End => end_x,
            };
            let color = if self.dragging && self.active_thumb == thumb {
                primary_hover
            } else if self.hovered_thumb == Some(thumb) {
                primary
            } else {
                Color::white()
            };
            ctx.fill_circle(x, cy, thumb_r, color);
            ctx.stroke_rect(
                Rect::new(x - thumb_r, cy - thumb_r, thumb_r * 2.0, thumb_r * 2.0),
                primary,
                2.0,
                Some(Radius::uniform(thumb_r)),
            );
        }

        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(control_rect, primary, 1.5, Some(Radius::uniform(4.0)));
        }
        ctx.pop_clip();
    }
}

impl RangeSlider {
    pub fn new(range: RangeInclusive<f64>) -> Self {
        let (min, max) = normalize_range(range);
        let config = crate::ui::config::use_config();
        Self {
            min,
            max,
            step: 1.0,
            start_value: min,
            end_value: max,
            start_binding: None,
            end_binding: None,
            active_thumb: RangeSliderThumb::Start,
            dragging: false,
            hovered_thumb: None,
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

    /// Bind the lower thumb to external state.
    pub fn start(mut self, state: &State<f64>) -> Self {
        self.start_binding = Some(state.clone());
        self.start_value = self.clamp_value(state.get());
        self.normalize_values();
        self
    }

    /// Bind the upper thumb to external state.
    pub fn end(mut self, state: &State<f64>) -> Self {
        self.end_binding = Some(state.clone());
        self.end_value = self.clamp_value(state.get());
        self.normalize_values();
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.slider_size = size;
        self
    }

    pub fn current_range(&self) -> (f64, f64) {
        (self.start_value, self.end_value)
    }

    pub fn active_thumb(&self) -> RangeSliderThumb {
        self.active_thumb
    }

    fn update_active_from_pos(&mut self, px: f32, frame: Rect) {
        let raw = self.value_from_pos(px, frame);
        let value = if self.step > 0.0 {
            let stepped = self.min + ((raw - self.min) / self.step).round() * self.step;
            self.normalize_step_value(stepped)
        } else {
            raw
        };
        self.set_thumb(self.active_thumb, value);
    }

    fn step_active(&mut self, direction: f64) {
        if self.step <= 0.0 {
            return;
        }
        let current = match self.active_thumb {
            RangeSliderThumb::Start => self.start_value,
            RangeSliderThumb::End => self.end_value,
        };
        let position = (current - self.min) / self.step;
        let nearest = position.round();
        let on_grid = (position - nearest).abs() <= 1e-9 * position.abs().max(1.0);
        let index = if on_grid {
            nearest + direction.signum()
        } else if direction > 0.0 {
            position.ceil()
        } else {
            position.floor()
        };
        self.set_thumb(
            self.active_thumb,
            self.normalize_step_value(self.min + index * self.step),
        );
    }

    fn set_thumb(&mut self, thumb: RangeSliderThumb, value: f64) {
        let value = self.clamp_value(value);
        let changed = match thumb {
            RangeSliderThumb::Start => {
                let value = value.min(self.end_value);
                if value == self.start_value {
                    false
                } else {
                    self.start_value = value;
                    if let Some(state) = self.start_binding.as_ref() {
                        if state.get() != value {
                            state.set(value);
                        }
                    }
                    true
                }
            }
            RangeSliderThumb::End => {
                let value = value.max(self.start_value);
                if value == self.end_value {
                    false
                } else {
                    self.end_value = value;
                    if let Some(state) = self.end_binding.as_ref() {
                        if state.get() != value {
                            state.set(value);
                        }
                    }
                    true
                }
            }
        };
        if changed {
            self.pending_change
                .set(Some((self.start_value, self.end_value)));
        }
    }

    fn sync_bound_values(&mut self) {
        if let Some(state) = self.start_binding.as_ref() {
            self.start_value = self.clamp_value(state.get());
        }
        if let Some(state) = self.end_binding.as_ref() {
            self.end_value = self.clamp_value(state.get());
        }
        self.normalize_values();
    }

    fn capture_bound_value_dependencies(&self) {
        if let Some(state) = self.start_binding.as_ref() {
            let _ = state.get();
        }
        if let Some(state) = self.end_binding.as_ref() {
            let _ = state.get();
        }
    }

    fn normalize_values(&mut self) {
        self.start_value = self.clamp_value(self.start_value);
        self.end_value = self.clamp_value(self.end_value);
        if self.start_value > self.end_value {
            self.start_value = self.end_value;
        }
    }

    fn clamp_value(&self, value: f64) -> f64 {
        if value.is_finite() {
            value.clamp(self.min, self.max)
        } else {
            self.min
        }
    }

    fn normalize_step_value(&self, value: f64) -> f64 {
        let precision = decimal_places(self.min)
            .max(decimal_places(self.step))
            .min(15);
        let factor = 10.0f64.powi(precision);
        let scaled = value * factor;
        if factor.is_finite() && scaled.is_finite() {
            scaled.round() / factor
        } else {
            value
        }
    }

    fn value_from_pos(&self, px: f32, frame: Rect) -> f64 {
        let (track_x, track_w) = self.track_span(frame);
        let pct = if track_w > 0.0 {
            f64::from(((px - track_x) / track_w).clamp(0.0, 1.0))
        } else if px <= track_x {
            0.0
        } else {
            1.0
        };
        self.min + pct * (self.max - self.min)
    }

    fn nearest_thumb(&self, px: f32, frame: Rect) -> RangeSliderThumb {
        let (start_x, end_x) = self.thumb_positions(frame);
        let start_distance = (px - start_x).abs();
        let end_distance = (px - end_x).abs();
        if start_distance < end_distance || (start_distance == end_distance && px < start_x) {
            RangeSliderThumb::Start
        } else {
            RangeSliderThumb::End
        }
    }

    fn thumb_positions(&self, frame: Rect) -> (f32, f32) {
        let (track_x, track_w) = self.track_span(frame);
        let span = self.max - self.min;
        let start_pct = ((self.start_value - self.min) / span).clamp(0.0, 1.0) as f32;
        let end_pct = ((self.end_value - self.min) / span).clamp(0.0, 1.0) as f32;
        (track_x + start_pct * track_w, track_x + end_pct * track_w)
    }

    fn control_height(&self) -> f32 {
        crate::ui::config::control_height(self.slider_size)
    }

    fn track_height(&self, frame: Rect) -> f32 {
        let nominal = match self.slider_size {
            ControlSize::Small => 3.0,
            ControlSize::Medium => 4.0,
            ControlSize::Large => 5.0,
        };
        (nominal * self.visual_scale(frame)).min(frame.h)
    }

    fn thumb_radius(&self, frame: Rect) -> f32 {
        let nominal = match self.slider_size {
            ControlSize::Small => 5.0,
            ControlSize::Medium => 6.0,
            ControlSize::Large => 7.5,
        };
        (nominal * self.visual_scale(frame))
            .min(frame.w * 0.5)
            .min(frame.h * 0.5)
            .max(0.0)
    }

    fn visual_scale(&self, frame: Rect) -> f32 {
        if self.control_height() > 0.0 {
            (frame.h / self.control_height()).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn track_span(&self, frame: Rect) -> (f32, f32) {
        let inset = self.thumb_radius(frame);
        (frame.x + inset, (frame.w - inset * 2.0).max(0.0))
    }
}

impl Default for RangeSlider {
    fn default() -> Self {
        Self::new(0.0..=100.0)
    }
}

impl RangeSlider {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::RangeSlider {
            min: self.min,
            max: self.max,
            step: self.step,
            start: self.start_value,
            end: self.end_value,
            active_thumb: self.active_thumb,
            size: self.slider_size,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let start_value = next
            .start_binding
            .as_ref()
            .map_or(self.start_value, |_| next.start_value);
        let end_value = next
            .end_binding
            .as_ref()
            .map_or(self.end_value, |_| next.end_value);
        self.min = next.min;
        self.max = next.max;
        self.step = next.step;
        self.start_binding = next.start_binding;
        self.end_binding = next.end_binding;
        self.start_value = start_value;
        self.end_value = end_value;
        self.slider_size = next.slider_size;
        self.normalize_values();
    }
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

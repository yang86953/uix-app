//! Slider widget — 滑块拖动选择器。

use std::cell::Cell;

use crate::define_widget;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, KeyCode, WidgetEvent, WidgetTree};
use uix_graphics::{Color, Radius};
use uix_platform::{Rect, Size};

define_widget! {
    /// Slider — 水平滑块，支持拖拽选择值。
    pub struct Slider {
        min: f32,
        max: f32,
        step: f32,
        value: f32,
        dragging: bool,
        hovered: bool,
        focused: bool,
        last_frame: Cell<Option<Rect>>,
        on_change: Option<Box<dyn FnMut(f32) + 'static>>,
    }


    tab_index => (&self) -> i32 { 1 }
    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(200.0, 24.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                self.dragging = true;
                self.focused = true;
                let frame_w = self.last_frame.get().map(|f| f.w).unwrap_or(200.0);
                self.update_from_pos(pos.x, frame_w);
                EventResult::Handled
            }
            WidgetEvent::MouseMove { pos, .. } => {
                let frame_w = self.last_frame.get().map(|f| f.w).unwrap_or(200.0);
                if self.dragging {
                    self.update_from_pos(pos.x, frame_w);
                }
                self.hovered = true;
                EventResult::Handled
            }
            WidgetEvent::MouseUp { .. } => {
                self.dragging = false;
                EventResult::Handled
            }
            WidgetEvent::HoverLeave => {
                self.hovered = false;
                self.dragging = false;
                EventResult::Handled
            }
            WidgetEvent::FocusIn => { self.focused = true; EventResult::Handled }
            WidgetEvent::FocusOut => { self.focused = false; EventResult::Handled }
            WidgetEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Right | KeyCode::Up => {
                        let new_val = (self.value + self.step).min(self.max);
                        if (new_val - self.value).abs() > f32::EPSILON {
                            self.value = new_val;
                            if let Some(ref mut cb) = self.on_change { cb(self.value); }
                        }
                        EventResult::Handled
                    }
                    KeyCode::Left | KeyCode::Down => {
                        let new_val = (self.value - self.step).max(self.min);
                        if (self.value - new_val).abs() > f32::EPSILON {
                            self.value = new_val;
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

    needs_continuous_update => (&self) -> bool { self.dragging }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        self.last_frame.set(Some(frame));
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let fill = ctx.tokens().color_fill_tertiary();
        let track_h = 4.0;
        let thumb_r = 6.0;
        let cy = frame.y + frame.h * 0.5;

        if frame.w <= 0.0 { return; }

        let pct = ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0);
        let thumb_x = frame.x + pct * (frame.w - 2.0);

        // 轨道（背景）
        ctx.fill_rect(Rect::new(frame.x, cy - track_h * 0.5, frame.w, track_h), fill, Some(Radius::uniform(track_h * 0.5)));
        // 轨道（已选部分）
        ctx.fill_rect(Rect::new(frame.x, cy - track_h * 0.5, thumb_x - frame.x, track_h), primary, Some(Radius::uniform(track_h * 0.5)));
        // 滑块
        let thumb_color = if self.dragging { primary_hover } else if self.hovered { primary } else { Color::white() };
        ctx.fill_circle(thumb_x, cy, thumb_r, thumb_color);
        ctx.stroke_rect(Rect::new(thumb_x - thumb_r, cy - thumb_r, thumb_r * 2.0, thumb_r * 2.0), primary, 2.0, Some(Radius::uniform(thumb_r)));

        // focus 指示
        if self.focused {
            ctx.stroke_rect(frame, primary, 1.5, Some(Radius::uniform(4.0)));
        }
    }
}

impl Slider {
    fn update_from_pos(&mut self, px: f32, frame_w: f32) {
        let pct = ((px - 2.0) / (frame_w - 4.0)).clamp(0.0, 1.0);
        let raw = self.min + pct * (self.max - self.min);
        let prev = self.value;
        if self.step > 0.0 {
            let stepped = (raw / self.step).round() * self.step;
            self.value = stepped.clamp(self.min, self.max);
        } else {
            self.value = raw.clamp(self.min, self.max);
        }
        if (self.value - prev).abs() > f32::EPSILON {
            if let Some(ref mut cb) = self.on_change {
                cb(self.value);
            }
        }
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
            on_change: None,
        }
    }
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = max;
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
    pub fn on_change<F: FnMut(f32) + 'static>(mut self, f: F) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
}

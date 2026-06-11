//! Slider widget — 滑块拖动选择器。

use crate::define_widget;
use crate::base::{Rect, Size};
use crate::graphics::{Color, Radius};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

define_widget! {
    /// Slider — 水平滑块，支持拖拽选择值。
    pub struct Slider {
        min: f32,
        max: f32,
        step: f32,
        value: f32,
        dragging: bool,
        hovered: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(200.0, 24.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                self.dragging = true;
                self.update_from_pos(pos.x);
                EventResult::Handled
            }
            WidgetEvent::MouseMove { pos } => {
                if self.dragging {
                    self.update_from_pos(pos.x);
                }
                self.hovered = true;
                EventResult::Handled
            }
            WidgetEvent::MouseUp { .. } | WidgetEvent::HoverLeave => {
                self.dragging = false;
                self.hovered = false;
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    needs_continuous_update => (&self) -> bool { self.dragging }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let primary = ctx.tokens().color_primary();
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
        let thumb_color = if self.dragging || self.hovered { primary } else { Color::white() };
        ctx.fill_circle(thumb_x, cy, thumb_r, thumb_color);
        ctx.stroke_rect(Rect::new(thumb_x - thumb_r, cy - thumb_r, thumb_r * 2.0, thumb_r * 2.0), primary, 2.0, Some(Radius::uniform(thumb_r)));
    }
}

impl Slider {
    fn update_from_pos(&mut self, px: f32) {
        // 位置映射到值（步长取整）
        let pct = ((px) / 200.0).clamp(0.0, 1.0);
        let raw = self.min + pct * (self.max - self.min);
        if self.step > 0.0 {
            let stepped = (raw / self.step).round() * self.step;
            self.value = stepped.clamp(self.min, self.max);
        } else {
            self.value = raw.clamp(self.min, self.max);
        }
    }
}

impl Default for Slider { fn default() -> Self { Self::new() } }

impl Slider {
    pub fn new() -> Self {
        Self { min: 0.0, max: 100.0, step: 1.0, value: 30.0, dragging: false, hovered: false }
    }
    pub fn range(mut self, min: f32, max: f32) -> Self { self.min = min; self.max = max; self }
    pub fn step(mut self, s: f32) -> Self { self.step = s; self }
    pub fn value(mut self, v: f32) -> Self { self.value = v.clamp(self.min, self.max); self }
}

//! Static loading indicator.

use crate::core::{Point, Rect, Size};
use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::ui::WidgetTree;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpinSize {
    Small,
    Default,
    Large,
}

define_widget! {
    /// Loading indicator.
    pub struct Spin {
        size: SpinSize,
        color: Option<Color>,
        spinning: bool,
        tip: String,
        wrapper_mode: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        if self.wrapper_mode {
            Size::new(0.0, 0.0)
        } else {
            let d = self.diameter();
            Size::new(d, d)
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if !self.spinning && !self.wrapper_mode {
            return;
        }

        let c = self.color.unwrap_or(ctx.tokens().color_primary());
        let d = self.diameter();
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;

        if self.wrapper_mode && self.spinning {
            ctx.fill_rect(frame, Color::from_rgba(0, 0, 0, 30), None);
        }

        if self.spinning {
            self.render_dots(ctx, cx, cy, d * 0.35, c);
        }

        if self.wrapper_mode && !self.tip.is_empty() {
            let tip_y = cy + d * 0.5 + 8.0;
            let tip_w = ctx.measure_text(&self.tip, 13.0).w;
            ctx.draw_text(
                &self.tip,
                Point::new(cx - tip_w * 0.5, tip_y),
                ctx.tokens().color_text_secondary(),
                13.0,
            );
        }
    }
}

impl Spin {
    fn diameter(&self) -> f32 {
        match self.size {
            SpinSize::Small => 16.0,
            SpinSize::Default => 24.0,
            SpinSize::Large => 36.0,
        }
    }

    fn render_dots(&self, ctx: &mut PaintContext, cx: f32, cy: f32, r: f32, c: Color) {
        let dot_r = r * 0.18;
        for i in 0..8 {
            let angle = i as f32 * std::f32::consts::TAU / 8.0;
            let dx = angle.cos() * r;
            let dy = angle.sin() * r;
            let opacity = 0.25 + (i as f32 / 8.0) * 0.75;
            let dot_color = Color::from_rgba(
                (c.r as f32 * opacity) as u8,
                (c.g as f32 * opacity) as u8,
                (c.b as f32 * opacity) as u8,
                (c.a as f32 * opacity) as u8,
            );
            ctx.fill_circle(cx + dx, cy + dy, dot_r, dot_color);
        }
    }
}

impl Default for Spin {
    fn default() -> Self {
        Self::new()
    }
}

impl Spin {
    pub fn new() -> Self {
        Self {
            size: SpinSize::Default,
            color: None,
            spinning: true,
            tip: String::new(),
            wrapper_mode: false,
        }
    }

    pub fn small(mut self) -> Self {
        self.size = SpinSize::Small;
        self
    }

    pub fn large(mut self) -> Self {
        self.size = SpinSize::Large;
        self
    }

    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }

    pub fn spinning(mut self, v: bool) -> Self {
        self.spinning = v;
        self
    }

    pub fn tip(mut self, t: impl Into<String>) -> Self {
        self.tip = t.into();
        self
    }

    pub fn wrapper_mode(mut self) -> Self {
        self.wrapper_mode = true;
        self
    }
}

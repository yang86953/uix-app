//! ProgressBar widget — deterministic and indeterminate progress indicators.

use crate::define_widget;
use crate::graphics::{Color, Radius};
use crate::base::{Rect, Size};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::WidgetTree;

/// Progress mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProgressMode {
    /// Fixed percentage (0.0 - 1.0).
    Determinate(f32),
    /// Animated indeterminate bar.
    Indeterminate,
}

define_widget! {
    /// ProgressBar widget.
    pub struct ProgressBar {
        progress: f32,
        mode: ProgressMode,
        stroke_color: Option<Color>,
        track_color: Option<Color>,
        height: f32,
        width: f32,
        round: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(self.width, self.height)
    }

    on_update => (&mut self, dt: f32) {
        if matches!(self.mode, ProgressMode::Indeterminate) {
            self.progress = (self.progress + dt * 0.5) % 1.0;
        }
    }

    needs_continuous_update => (&self) -> bool {
        matches!(self.mode, ProgressMode::Indeterminate)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let tokens = ctx.tokens();

        let track_c = self.track_color.unwrap_or(tokens.color_fill_tertiary());
        let stroke_c = self.stroke_color.unwrap_or(tokens.color_primary());
        let radius = if self.round {
            Some(Radius::uniform(frame.h * 0.5))
        } else {
            Some(Radius::uniform(tokens.border_radius_sm()))
        };

        // Track (background)
        ctx.fill_rect(frame, track_c, radius);

        match self.mode {
            ProgressMode::Determinate(p) => {
                let fill_w = frame.w * p;
                if fill_w > 0.0 {
                    let fill_rect = Rect::new(frame.x, frame.y, fill_w, frame.h);
                    ctx.fill_rect(fill_rect, stroke_c, radius);
                }
            }
            ProgressMode::Indeterminate => {
                let bar_w = frame.w * 0.3;
                let bar_x = frame.x + (self.progress * (frame.w - bar_w));
                let bar_rect = Rect::new(bar_x, frame.y, bar_w, frame.h);
                ctx.fill_rect(bar_rect, stroke_c, radius);
            }
        }
    }
}

impl Default for ProgressBar {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressBar {
    pub fn new() -> Self {
        Self {
            progress: 0.0,
            mode: ProgressMode::Determinate(0.0),
            stroke_color: None,
            track_color: None,
            height: 8.0,
            width: 200.0,
            round: true,
        }
    }

    pub fn progress(mut self, p: f32) -> Self {
        self.progress = p.clamp(0.0, 1.0);
        self.mode = ProgressMode::Determinate(self.progress);
        self
    }

    pub fn indeterminate(mut self) -> Self {
        self.mode = ProgressMode::Indeterminate;
        self
    }

    pub fn stroke_color(mut self, c: Color) -> Self {
        self.stroke_color = Some(c);
        self
    }

    pub fn track_color(mut self, c: Color) -> Self {
        self.track_color = Some(c);
        self
    }

    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = w;
        self.height = h;
        self
    }
}

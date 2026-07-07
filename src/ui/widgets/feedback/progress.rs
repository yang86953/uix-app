//! ProgressBar widget - deterministic and indeterminate progress indicators.

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;

/// Progress display type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProgressType {
    Line,
    Circle,
}

/// Progress mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProgressMode {
    /// Fixed percentage (0.0 - 1.0).
    Determinate(f32),
    /// Indeterminate bar. Animation is driven outside the component.
    Indeterminate,
}

component! {
    /// ProgressBar widget.
    pub struct ProgressBar {
        progress: f32,
        mode: ProgressMode,
        stroke_color: Option<Color>,
        track_color: Option<Color>,
        height: f32,
        width: f32,
        round: bool,
        progress_type: ProgressType,
        indeterminate_phase: f32,
        previous_indeterminate_phase: f32,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }



    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let tokens = ctx.tokens();

        let track_c = self.track_color.unwrap_or(tokens.color_fill_tertiary());
        let stroke_c = self.stroke_color.unwrap_or(tokens.color_primary());

        if self.progress_type == ProgressType::Circle {
            let cx = frame.x + frame.w * 0.5;
            let cy = frame.y + frame.h * 0.5;
            let r = frame.w.min(frame.h) * 0.4;
            let track_width = 8.0;

            ctx.fill_circle(cx, cy, r, track_c);
            ctx.fill_circle(cx, cy, r - track_width, ctx.tokens().color_bg_container());

            let (start_angle, end_angle) = match self.mode {
                ProgressMode::Determinate(p) => {
                    let start_angle = -std::f32::consts::FRAC_PI_2;
                    (start_angle, start_angle + std::f32::consts::TAU * p)
                }
                ProgressMode::Indeterminate => {
                    let start_angle = self.indeterminate_phase * std::f32::consts::TAU;
                    (start_angle, start_angle + std::f32::consts::TAU * 0.25)
                }
            };
            if end_angle > start_angle {
                let segments = 64;
                for i in 0..segments {
                    let a1 = start_angle + (end_angle - start_angle) * i as f32 / segments as f32;
                    let a2 = start_angle + (end_angle - start_angle) * (i + 1) as f32 / segments as f32;
                    let inner_r = r - track_width * 0.5;
                    let x1 = cx + a1.cos() * inner_r;
                    let y1 = cy + a1.sin() * inner_r;
                    let x2 = cx + a2.cos() * inner_r;
                    let y2 = cy + a2.sin() * inner_r;
                    ctx.canvas_2d().draw_line(x1, y1, x2, y2, stroke_c, track_width);
                }
            }
            return;
        }

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
                let bar_x = frame.x + (frame.w - bar_w) * self.indeterminate_phase;
                let bar_rect = Rect::new(bar_x, frame.y, bar_w, frame.h);
                ctx.fill_rect(bar_rect, stroke_c, radius);
            }
        }
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !matches!(self.mode, ProgressMode::Indeterminate) {
            return false;
        }

        self.previous_indeterminate_phase = self.indeterminate_phase;
        self.indeterminate_phase = (self.indeterminate_phase
            + dt as f32 * Self::INDETERMINATE_PHASE_SPEED)
            .rem_euclid(1.0);
        true
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        match self.mode {
            ProgressMode::Indeterminate if self.progress_type == ProgressType::Circle => {
                self.circle_indeterminate_bounds(frame)
            }
            ProgressMode::Indeterminate => self.line_indeterminate_bounds(frame),
            ProgressMode::Determinate(_) => Rect::zero(),
        }
    }
}

impl Default for ProgressBar {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressBar {
    const INDETERMINATE_PHASE_SPEED: f32 = 0.75;

    pub fn new() -> Self {
        Self {
            progress: 0.0,
            mode: ProgressMode::Determinate(0.0),
            stroke_color: None,
            track_color: None,
            height: 8.0,
            width: 200.0,
            round: true,
            progress_type: ProgressType::Line,
            indeterminate_phase: 0.0,
            previous_indeterminate_phase: 0.0,
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

    pub fn circle(mut self) -> Self {
        self.progress_type = ProgressType::Circle;
        self
    }

    pub fn animation_phase(&self) -> f32 {
        self.indeterminate_phase
    }

    fn line_indeterminate_bounds(&self, frame: Rect) -> Rect {
        let previous = self.indeterminate_bar_rect(frame, self.previous_indeterminate_phase);
        let current = self.indeterminate_bar_rect(frame, self.indeterminate_phase);
        previous.union(&current)
    }

    fn indeterminate_bar_rect(&self, frame: Rect, phase: f32) -> Rect {
        let bar_w = frame.w * 0.3;
        let bar_x = frame.x + (frame.w - bar_w) * phase;
        Rect::new(bar_x, frame.y, bar_w, frame.h)
    }

    fn circle_indeterminate_bounds(&self, frame: Rect) -> Rect {
        let r = frame.w.min(frame.h) * 0.4 + 1.0;
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;
        Rect::new(cx - r, cy - r, r * 2.0, r * 2.0)
    }

    fn intrinsic_size(&self) -> Size {
        if self.progress_type == ProgressType::Circle {
            let d = self.width.max(self.height);
            return Size::new(d, d);
        }
        Size::new(self.width, self.height)
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::ProgressBar {
            progress: self.progress,
            mode: self.mode,
            stroke_color: self.stroke_color,
            track_color: self.track_color,
            height: self.height,
            width: self.width,
            round: self.round,
            progress_type: self.progress_type,
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/ui/widgets/feedback/progress.rs"]
mod tests;

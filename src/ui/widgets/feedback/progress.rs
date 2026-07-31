//! ProgressBar widget - deterministic and indeterminate progress indicators.

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::command::PaintPass;
use crate::draw::{Color, GradientDirection, Radius};
use crate::ui::core::paint_context::PaintContext;
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;
use std::rc::Rc;

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
        gradient_start: Option<Color>,
        gradient_end: Option<Color>,
        steps: usize,
        dashboard: bool,
        format_text: Option<Rc<dyn Fn(f32) -> String>>,
        indeterminate_phase: f32,
        previous_indeterminate_phase: f32,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }



    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if ctx.paint_pass() != PaintPass::Content {
            return;
        }
        let frame = Self::normalize_frame(frame);
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let track_c = self
            .track_color
            .unwrap_or_else(|| ctx.tokens().color_fill_tertiary());
        let stroke_c = self
            .stroke_color
            .unwrap_or_else(|| ctx.tokens().color_primary());
        let label_color = ctx.tokens().color_text();

        ctx.push_clip(frame);
        if self.dashboard {
            self.render_dashboard(frame, track_c, stroke_c, label_color, ctx);
            ctx.pop_clip();
            return;
        }
        if self.progress_type == ProgressType::Circle {
            let cx = frame.x + frame.w * 0.5;
            let cy = frame.y + frame.h * 0.5;
            let r = frame.w.min(frame.h) * 0.4;
            if r <= 0.0 {
                ctx.pop_clip();
                return;
            }
            let track_width = (r * 0.25).clamp(1.0, 8.0).min(r);

            ctx.fill_circle(cx, cy, r, track_c);
            let inner_radius = r - track_width;
            if inner_radius > 0.0 {
                ctx.fill_circle(cx, cy, inner_radius, ctx.tokens().color_bg_container());
            }

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
                    ctx.draw_line(x1, y1, x2, y2, stroke_c, track_width);
                }
            }
            self.paint_progress_label(frame, label_color, ctx);
            ctx.pop_clip();
            return;
        }

        let radius = self.line_radius(frame);

        // Track (background)
        ctx.fill_rect(frame, track_c, radius);

        match self.mode {
            ProgressMode::Determinate(p) => {
                let fill_w = frame.w * p;
                if fill_w > 0.0 {
                    let fill_rect = Rect::new(frame.x, frame.y, fill_w, frame.h);
                    if self.steps > 1 {
                        let gap = (frame.w / self.steps as f32 * 0.08).min(2.0);
                        for index in 0..self.steps {
                            let start = frame.x + index as f32 * frame.w / self.steps as f32;
                            let end = frame.x + (index + 1) as f32 * frame.w / self.steps as f32;
                            let visible_end = (end - gap).min(frame.x + fill_w);
                            if visible_end > start {
                                self.paint_line_fill(
                                    ctx,
                                    Rect::new(start, frame.y, visible_end - start, frame.h),
                                    frame,
                                    stroke_c,
                                );
                            }
                        }
                    } else {
                        self.paint_line_fill(ctx, fill_rect, frame, stroke_c);
                    }
                }
                self.paint_progress_label(frame, label_color, ctx);
            }
            ProgressMode::Indeterminate => {
                let bar_w = frame.w * 0.3;
                let bar_x = frame.x + (frame.w - bar_w) * self.indeterminate_phase;
                let bar_rect = Rect::new(bar_x, frame.y, bar_w, frame.h);
                ctx.fill_rect(bar_rect, stroke_c, radius);
            }
        }
        ctx.pop_clip();
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
        let frame = Self::normalize_frame(frame);
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
            gradient_start: None,
            gradient_end: None,
            steps: 0,
            dashboard: false,
            format_text: None,
            indeterminate_phase: 0.0,
            previous_indeterminate_phase: 0.0,
        }
    }

    pub fn progress(mut self, p: f32) -> Self {
        self.progress = Self::normalize_progress(p);
        self.mode = ProgressMode::Determinate(self.progress);
        self
    }

    /// `.percent()` 是 `.progress()` 的语义别名，参数仍为 0.0～1.0。
    pub fn percent(self, p: f32) -> Self {
        self.progress(p)
    }

    pub fn gradient(mut self, start: Color, end: Color) -> Self {
        self.gradient_start = Some(start);
        self.gradient_end = Some(end);
        self
    }

    pub fn steps(mut self, count: usize) -> Self {
        self.steps = count.max(1);
        self
    }

    pub fn dashboard(mut self) -> Self {
        self.dashboard = true;
        self.progress_type = ProgressType::Circle;
        self
    }

    pub fn format<F>(mut self, formatter: F) -> Self
    where
        F: Fn(f32) -> String + 'static,
    {
        self.format_text = Some(Rc::new(formatter));
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
        self.height = Self::normalize_dimension(h);
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Self::normalize_dimension(w);
        self
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = Self::normalize_dimension(w);
        self.height = Self::normalize_dimension(h);
        self
    }

    pub fn round(mut self, round: bool) -> Self {
        self.round = round;
        self
    }

    pub fn circle(mut self) -> Self {
        self.progress_type = ProgressType::Circle;
        self
    }

    pub fn animation_phase(&self) -> f32 {
        self.indeterminate_phase
    }

    fn render_dashboard(
        &self,
        frame: Rect,
        track_color: Color,
        stroke_color: Color,
        label_color: Color,
        ctx: &mut PaintContext,
    ) {
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.72;
        let radius = (frame.w * 0.42).min(frame.h * 0.65);
        if radius <= 0.0 {
            return;
        }
        let width = (radius * 0.2).clamp(1.0, 8.0).min(radius);
        let start = std::f32::consts::PI;
        let end = std::f32::consts::TAU;
        ctx.stroke_arc(cx, cy, radius, start, end, track_color, width);

        let (progress_start, progress_end) = match self.mode {
            ProgressMode::Determinate(progress) => (start, start + std::f32::consts::PI * progress),
            ProgressMode::Indeterminate => {
                let sweep = std::f32::consts::PI * 0.25;
                let travel = std::f32::consts::PI - sweep;
                let progress_start = start + travel * self.indeterminate_phase;
                (progress_start, progress_start + sweep)
            }
        };
        if progress_end > progress_start {
            ctx.stroke_arc(
                cx,
                cy,
                radius,
                progress_start,
                progress_end,
                stroke_color,
                width,
            );
        }
        self.paint_progress_label(frame, label_color, ctx);
    }

    fn paint_progress_label(&self, frame: Rect, color: Color, ctx: &mut PaintContext) {
        if let (ProgressMode::Determinate(progress), Some(format)) =
            (self.mode, self.format_text.as_ref())
        {
            ctx.text_center(&format(progress), frame, color, 11.0);
        }
    }

    fn paint_line_fill(
        &self,
        ctx: &mut PaintContext,
        rect: Rect,
        gradient_domain: Rect,
        fallback: Color,
    ) {
        let Some((start, end)) = self.gradient_start.zip(self.gradient_end) else {
            ctx.fill_rect(rect, fallback, self.line_radius(rect));
            return;
        };
        if !self.round {
            let color_a = gradient_color_at(start, end, gradient_domain, rect.x);
            let color_b = gradient_color_at(start, end, gradient_domain, rect.x + rect.w);
            ctx.fill_linear_gradient(rect, color_a, color_b, GradientDirection::Horizontal);
            return;
        }

        let cap_radius = (rect.h * 0.5).min(rect.w * 0.5);
        if cap_radius <= 0.0 {
            return;
        }
        let left_center = rect.x + cap_radius;
        let right_center = rect.x + rect.w - cap_radius;
        if right_center > left_center {
            let center = Rect::new(left_center, rect.y, right_center - left_center, rect.h);
            ctx.fill_linear_gradient(
                center,
                gradient_color_at(start, end, gradient_domain, left_center),
                gradient_color_at(start, end, gradient_domain, right_center),
                GradientDirection::Horizontal,
            );
        }
        let cy = rect.y + rect.h * 0.5;
        ctx.fill_circle(
            left_center,
            cy,
            cap_radius,
            gradient_color_at(start, end, gradient_domain, left_center),
        );
        if right_center > left_center {
            ctx.fill_circle(
                right_center,
                cy,
                cap_radius,
                gradient_color_at(start, end, gradient_domain, right_center),
            );
        }
    }

    fn line_indeterminate_bounds(&self, frame: Rect) -> Rect {
        let previous = self.indeterminate_bar_rect(frame, self.previous_indeterminate_phase);
        let current = self.indeterminate_bar_rect(frame, self.indeterminate_phase);
        previous
            .union(&current)
            .intersect(&frame)
            .unwrap_or_default()
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
            .intersect(&frame)
            .unwrap_or_default()
    }

    fn line_radius(&self, rect: Rect) -> Option<Radius> {
        self.round
            .then(|| Radius::uniform(rect.w.min(rect.h) * 0.5))
    }

    fn normalize_frame(frame: Rect) -> Rect {
        Rect::new(
            if frame.x.is_finite() { frame.x } else { 0.0 },
            if frame.y.is_finite() { frame.y } else { 0.0 },
            Self::normalize_dimension(frame.w),
            Self::normalize_dimension(frame.h),
        )
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

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.progress = Self::normalize_progress(next.progress);
        self.mode = match next.mode {
            ProgressMode::Determinate(progress) => {
                ProgressMode::Determinate(Self::normalize_progress(progress))
            }
            ProgressMode::Indeterminate => ProgressMode::Indeterminate,
        };
        self.stroke_color = next.stroke_color;
        self.track_color = next.track_color;
        self.height = Self::normalize_dimension(next.height);
        self.width = Self::normalize_dimension(next.width);
        self.round = next.round;
        self.progress_type = next.progress_type;
        self.gradient_start = next.gradient_start;
        self.gradient_end = next.gradient_end;
        self.steps = next.steps;
        self.dashboard = next.dashboard;
        self.format_text = next.format_text;
    }

    fn normalize_progress(value: f32) -> f32 {
        if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn normalize_dimension(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }
}

fn interpolate_color(start: Color, end: Color, amount: f32) -> Color {
    let t = amount.clamp(0.0, 1.0);
    Color::from_rgba(
        (start.r as f32 + (end.r as f32 - start.r as f32) * t).round() as u8,
        (start.g as f32 + (end.g as f32 - start.g as f32) * t).round() as u8,
        (start.b as f32 + (end.b as f32 - start.b as f32) * t).round() as u8,
        (start.a as f32 + (end.a as f32 - start.a as f32) * t).round() as u8,
    )
}

fn gradient_color_at(start: Color, end: Color, domain: Rect, x: f32) -> Color {
    let amount = if domain.w > 0.0 {
        (x - domain.x) / domain.w
    } else {
        0.0
    };
    interpolate_color(start, end, amount)
}

//! ProgressBar widget — deterministic and indeterminate progress indicators.

use crate::define_widget;
use crate::animation::core::Animation;
use crate::api::Easing;
use uix_graphics::{Color, Radius};
use uix_platform::{Rect, Size};
use crate::render_context::RenderContext;
use crate::widget::WidgetTree;

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
        progress_type: ProgressType,
        /// 不确定进度动画（0→1 循环，周期 2s）
        indet_anim: Option<Animation<f32>>,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        if self.progress_type == ProgressType::Circle {
            let d = self.width.max(self.height);
            return Size::new(d, d);
        }
        Size::new(self.width, self.height)
    }

    on_update => (&mut self, dt: f64) {
        if let Some(ref mut anim) = self.indet_anim {
            anim.update(dt);
            if anim.is_finished() {
                // 循环：重置到起点
                *anim = Animation::new(0.0, 1.0, 2.0).with_easing(Easing::Linear);
            }
        }
    }

    needs_continuous_update => (&self) -> bool {
        self.indet_anim.is_some()
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
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

            if let ProgressMode::Determinate(p) = self.mode {
                let start_angle = -std::f32::consts::FRAC_PI_2;
                let end_angle = start_angle + std::f32::consts::TAU * p;
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
                let p = self.indet_anim.as_ref().map(|a| a.current_value()).unwrap_or(0.0);
                let bar_w = frame.w * 0.3;
                let bar_x = frame.x + (p * (frame.w - bar_w));
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
            progress_type: ProgressType::Line,
            indet_anim: None,
        }
    }

    pub fn progress(mut self, p: f32) -> Self {
        self.progress = p.clamp(0.0, 1.0);
        self.mode = ProgressMode::Determinate(self.progress);
        self.indet_anim = None;
        self
    }

    pub fn indeterminate(mut self) -> Self {
        self.mode = ProgressMode::Indeterminate;
        self.indet_anim = Some(
            Animation::new(0.0, 1.0, 2.0).with_easing(Easing::Linear),
        );
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
}

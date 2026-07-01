//! Spin widget — 加载中旋转动画指示器。

use crate::define_widget;
use crate::animation::core::Animation;
use crate::api::Easing;
use uix_platform::{Rect, Size};
use uix_graphics::Color;
use crate::render_context::RenderContext;
use crate::widget::WidgetTree;

/// Spin 尺寸。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpinSize { Small, Default, Large }

define_widget! {
    /// Spin — 旋转加载动画。
    pub struct Spin {
        size: SpinSize,
        /// 旋转动画（0→1 循环，周期 ≈1.25s）
        anim: Option<Animation<f32>>,
        color: Option<Color>,
        spinning: bool,
        tip: String,
        wrapper_mode: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        if self.wrapper_mode {
            // wrapper mode defers to child; return minimal since children are separate
            Size::new(0.0, 0.0)
        } else {
            let d = self.diameter();
            Size::new(d, d)
        }
    }

    on_update => (&mut self, dt: f64) {
        if let Some(ref mut anim) = self.anim {
            anim.update(dt);
            if anim.is_finished() {
                anim.reset();
            }
        }
    }

    needs_continuous_update => (&self) -> bool {
        self.spinning
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if !self.spinning && !self.wrapper_mode { return; }
        let phase = self.anim.as_ref().map(|a| a.current_value()).unwrap_or(0.0);
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;
        let r = frame.w.min(frame.h) * 0.35;
        let c = self.color.unwrap_or(ctx.tokens().color_primary());

        if self.wrapper_mode && self.spinning {
            // overlay background
            ctx.fill_rect(frame, Color::from_rgba(0, 0, 0, 30), None);
            // spinner centered
            let d = self.diameter();
            let sp_cx = cx;
            let sp_cy = cy;
            let sp_r = d * 0.35;
            let dot_r = sp_r * 0.18;
            for i in 0..8 {
                let angle = i as f32 * std::f32::consts::TAU / 8.0 + phase * std::f32::consts::TAU;
                let dx = angle.cos() * sp_r;
                let dy = angle.sin() * sp_r;
                let opacity = 0.15 + ((i as f32 / 8.0 + phase).fract() * 0.85);
                let dot_color = Color::from_rgba(
                    (c.r as f32 * opacity) as u8,
                    (c.g as f32 * opacity) as u8,
                    (c.b as f32 * opacity) as u8,
                    (c.a as f32 * opacity) as u8,
                );
                ctx.fill_circle(sp_cx + dx, sp_cy + dy, dot_r, dot_color);
            }
            // tip text
            if !self.tip.is_empty() {
                let tip_y = sp_cy + d * 0.5 + 8.0;
                let tip_w = ctx.measure_text(&self.tip, 13.0).w;
                ctx.draw_text(&self.tip, uix_platform::Point::new(cx - tip_w * 0.5, tip_y), ctx.tokens().color_text_secondary(), 13.0);
            }
            return;
        }

        // 画 8 个点，绕圆排列，根据相位控制透明度
        let dot_r = r * 0.18;
        for i in 0..8 {
            let angle = i as f32 * std::f32::consts::TAU / 8.0 + phase * std::f32::consts::TAU;
            let dx = angle.cos() * r;
            let dy = angle.sin() * r;
            let opacity = 0.15 + ((i as f32 / 8.0 + phase).fract() * 0.85);
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

impl Spin {
    fn diameter(&self) -> f32 {
        match self.size {
            SpinSize::Small => 16.0,
            SpinSize::Default => 24.0,
            SpinSize::Large => 36.0,
        }
    }
}

impl Default for Spin { fn default() -> Self { Self::new() } }

impl Spin {
    pub fn new() -> Self {
        Self {
            size: SpinSize::Default,
            anim: Some(Animation::new(0.0, 1.0, 1.25).with_easing(Easing::Linear)),
            color: None, spinning: true, tip: String::new(), wrapper_mode: false,
        }
    }
    pub fn small(mut self) -> Self { self.size = SpinSize::Small; self }
    pub fn large(mut self) -> Self { self.size = SpinSize::Large; self }
    pub fn color(mut self, c: Color) -> Self { self.color = Some(c); self }
    pub fn spinning(mut self, v: bool) -> Self { self.spinning = v; self }
    pub fn tip(mut self, t: impl Into<String>) -> Self { self.tip = t.into(); self }
    pub fn wrapper_mode(mut self) -> Self { self.wrapper_mode = true; self }
}

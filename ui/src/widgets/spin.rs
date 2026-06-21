//! Spin widget — 加载中旋转动画指示器。

use crate::define_widget;
use uix_core::{Rect, Size};
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
        /// 动画相位 [0, 1)
        phase: f32,
        color: Option<Color>,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        let d = self.diameter();
        Size::new(d, d)
    }

    on_update => (&mut self, dt: f32) {
        self.phase = (self.phase + dt * 0.8) % 1.0;
    }

    needs_continuous_update => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;
        let r = frame.w.min(frame.h) * 0.35;
        let c = self.color.unwrap_or(ctx.tokens().color_primary());

        // 画 8 个点，绕圆排列，根据相位控制透明度
        let dot_r = r * 0.18;
        for i in 0..8 {
            let angle = i as f32 * std::f32::consts::TAU / 8.0 + self.phase * std::f32::consts::TAU;
            let dx = angle.cos() * r;
            let dy = angle.sin() * r;
            let opacity = 0.15 + ((i as f32 / 8.0 + self.phase).fract() * 0.85);
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
        Self { size: SpinSize::Default, phase: 0.0, color: None }
    }
    pub fn small(mut self) -> Self { self.size = SpinSize::Small; self }
    pub fn large(mut self) -> Self { self.size = SpinSize::Large; self }
    pub fn color(mut self, c: Color) -> Self { self.color = Some(c); self }
}

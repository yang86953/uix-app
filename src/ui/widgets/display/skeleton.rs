//! Skeleton widget — 骨架屏加载占位（带动画闪烁）。

use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::native::{Rect, Size};
use crate::ui::animation::core::Animation;
use crate::ui::animation::Easing;
use crate::ui::WidgetTree;

/// 骨架形状。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SkeletonShape {
    Rect,
    Circle,
    Text,
}

define_widget! {
    /// Skeleton — 加载中骨架占位，带动画闪烁效果。
    pub struct Skeleton {
        shape: SkeletonShape,
        w: f32,
        h: f32,
        /// 闪烁动画（0→1 循环，周期 ≈0.67s）
        anim: Option<Animation<f32>>,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        Size::new(self.w, self.h)
    }

    on_update => (&mut self, dt: f64) {
        if let Some(ref mut anim) = self.anim {
            anim.update(dt);
            if anim.is_finished() {
                anim.reset();
            }
        }
    }

    needs_continuous_update => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_fill_tertiary();
        let phase = self.anim.as_ref().map(|a| a.current_value()).unwrap_or(0.0);
        // 闪烁：相位控制亮度偏移
        let bright = (phase * std::f32::consts::TAU).sin() * 0.15 + 0.85;
        let r = (bg.r as f32 * bright) as u8;
        let g = (bg.g as f32 * bright) as u8;
        let b = (bg.b as f32 * bright) as u8;
        let a = bg.a;
        let color = Color::from_rgba(r, g, b, a);

        match self.shape {
            SkeletonShape::Rect => {
                let radius = Some(crate::draw::Radius::uniform(4.0));
                ctx.fill_rect(frame, color, radius);
            }
            SkeletonShape::Circle => {
                ctx.fill_circle(frame.x + frame.w * 0.5, frame.y + frame.h * 0.5,
                    frame.w.min(frame.h) * 0.5, color);
            }
            SkeletonShape::Text => {
                // 两行文字模拟
                let line_h = self.h * 0.35;
                let gap = self.h * 0.15;
                ctx.fill_rect(Rect::new(frame.x, frame.y, frame.w * 0.8, line_h), color,
                    Some(crate::draw::Radius::uniform(2.0)));
                ctx.fill_rect(Rect::new(frame.x, frame.y + line_h + gap, frame.w * 0.5, line_h), color,
                    Some(crate::draw::Radius::uniform(2.0)));
            }
        }
    }
}

impl Default for Skeleton {
    fn default() -> Self {
        Self::new()
    }
}

impl Skeleton {
    pub fn new() -> Self {
        Self {
            shape: SkeletonShape::Rect,
            w: 200.0,
            h: 16.0,
            anim: Some(Animation::new(0.0, 1.0, 0.667).with_easing(Easing::Linear)),
        }
    }
    pub fn shape(mut self, s: SkeletonShape) -> Self {
        self.shape = s;
        self
    }
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.w = w;
        self.h = h;
        self
    }
}

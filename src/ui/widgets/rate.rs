//! Rate widget — 星级评分。

use crate::define_widget;
use crate::base::{Rect, Size};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

define_widget! {
    /// Rate — 星级评分，点击选择分值。
    pub struct Rate {
        count: usize,
        value: usize,
        half: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(self.count as f32 * 24.0, 24.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            let star_idx = (pos.x / 24.0) as usize;
            if star_idx < self.count {
                if self.half {
                    // 点击左半 → 半星 (value * 2 - 1), 右半 → 整星
                    let in_star_x = pos.x - star_idx as f32 * 24.0;
                    self.value = star_idx * 2 + if in_star_x < 12.0 { 1 } else { 2 };
                } else {
                    self.value = star_idx + 1;
                }
                return EventResult::Handled;
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let warning = ctx.tokens().color_warning();
        let fill_tertiary = ctx.tokens().color_fill_tertiary();
        for i in 0..self.count {
            let sx = frame.x + i as f32 * 24.0;
            let sy = frame.y + 2.0;
            // 空心星
            let filled = if self.half {
                self.value >= i * 2 + 2
            } else {
                self.value > i
            };
            let color = if filled { warning } else { fill_tertiary };
            // 用 ★ 字符渲染（纯文本方式）
            ctx.draw_text("★", crate::base::Point::new(sx + 2.0, sy), color, 18.0);

            // 半星支持：左半填充
            if self.half && self.value == i * 2 + 1 {
                // 用裁剪绘制左半星——简化：用半透明叠加
                ctx.draw_text("★", crate::base::Point::new(sx + 2.0, sy), warning, 18.0);
                // 遮盖右半（用背景色填充矩形）
                let bg = ctx.tokens().color_bg_container();
                ctx.fill_rect(Rect::new(sx + 14.0, sy, 10.0, 18.0), bg, None);
            }
        }
    }
}

impl Default for Rate { fn default() -> Self { Self::new() } }

impl Rate {
    pub fn new() -> Self {
        Self { count: 5, value: 0, half: false }
    }
    pub fn count(mut self, n: usize) -> Self { self.count = n; self }
    pub fn value(mut self, v: usize) -> Self { self.value = v; self }
    pub fn allow_half(mut self) -> Self { self.half = true; self }
}

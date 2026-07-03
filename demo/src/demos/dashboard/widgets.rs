use uix::define_widget;
use uix::graphics::{Color, traits::GraphicsEngine};
use uix::platform::{Point, Rect, Size};
use uix::ui::render_context::RenderContext;
use uix::ui::widget::WidgetTree;
use uix::ui::widget::{EventResult, WidgetEvent};

// ── Counter — 自定义 widget 示例 ──

define_widget! {
    pub struct Counter { pub count: u32 }
    @new -> Self { Self { count: 0 } }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size { Size::new(120.0, 36.0) }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event { WidgetEvent::MouseDown { .. } => { self.count += 1; EventResult::Handled } _ => EventResult::NotHandled }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_primary_bg();
        let color = ctx.tokens().color_primary();
        ctx.fill_rect(frame, bg, None);
        ctx.draw_text(&format!("Count: {}", self.count), Point::new(frame.x + 8.0, frame.y + 8.0), color, 14.0);
    }
}

// ── PulseRing — 脉冲动画 ──

define_widget! {
    pub struct PulseRing {
        pub time: f32,
    }
    @new -> Self { Self { time: 0.0 } }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(48.0, 48.0)
    }

    on_update => (&mut self, dt: f64) {
        self.time += dt as f32;
        if self.time > std::f32::consts::TAU { self.time -= std::f32::consts::TAU; }
    }

    needs_continuous_update => (&self) -> bool { true }

    // 只返回脉冲圆实际覆盖的区域，避免清除整个 frame 导致四角白线
    dirty_rect => (&self, frame: Rect) -> Rect {
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;
        let max_r = 24.0; // 最大脉冲半径 ≈ 6 + 16 + 3(stroke)
        Rect::new(cx - max_r, cy - max_r, max_r * 2.0, max_r * 2.0)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;
        let phase = (self.time * 1.5).sin() * 0.5 + 0.5; // 0..1
        let r = 6.0 + phase * 16.0;
        let alpha = (1.0 - phase * 0.6) * 255.0;
        let base = ctx.tokens().color_primary();
        let c = Color::from_rgba(base.r, base.g, base.b, alpha as u8);
        let eng = ctx.canvas_2d();
        eng.stroke_circle(cx, cy, r, c, 3.0);
        if r > 10.0 {
            eng.fill_circle(cx, cy, r * 0.3, c);
        }
    }
}

// ── BounceBall — 弹跳动画 ──

define_widget! {
    pub struct BounceBall {
        pub time: f32,
    }
    @new -> Self { Self { time: 0.0 } }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(200.0, 60.0)
    }

    on_update => (&mut self, dt: f64) {
        self.time += dt as f32;
        if self.time > 2.0 { self.time -= 2.0; }
    }

    needs_continuous_update => (&self) -> bool { true }

    // 只返回球 + 阴影的边界框，避免清除整个 frame
    dirty_rect => (&self, frame: Rect) -> Rect {
        let cx = frame.x + frame.w * 0.5;
        let ball_top = frame.y + frame.h - 50.0; // 球最高位置（约 bottom-50）
        let ball_bot = frame.y + frame.h - 10.0 + 10.0; // 球最低位置 + 半径
        let max_r = 14.0; // 球半径 10 + 阴影半径 ~14
        Rect::new(cx - max_r, ball_top - max_r, max_r * 2.0, ball_bot - ball_top + max_r * 2.0)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let t = self.time / 2.0; // 0..1
        // 弹跳：快速上升，慢速下降
        let bounce = if t < 0.5 {
            1.0 - (t * 2.0).powf(2.0)  // 快速上升
        } else {
            -((t - 0.5) * 2.0 - 1.0).powf(2.0) + 1.0  // 慢速下降
        };
        let cy = frame.y + frame.h - 10.0 - bounce * 40.0;
        let cx = frame.x + frame.w * 0.5;
        let primary = ctx.tokens().color_primary();
        // 影子（根据高度变化大小和透明度）
        let shadow_alpha = (0.3 + bounce * 0.5 * 0.7) * 255.0;
        let shadow_r = 6.0 + bounce * 8.0;
        let shadow_c = Color::from_rgba(0, 0, 0, shadow_alpha as u8);
        ctx.fill_circle(cx, frame.y + frame.h - 6.0, shadow_r, shadow_c);
        ctx.canvas_2d().fill_circle(cx, cy, 10.0, primary);
    }
}

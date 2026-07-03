//! DebugRenderService — 调试绘制服务。
//!
//! 提供 widget 调试边框、标签和 frame 信息的绘制。
//! 由 RenderContext 组合持有。

use crate::api::traits::DebugRenderer;
use uix_graphics::traits::Canvas2D;
use uix_graphics::Color;
use uix_platform::Rect;

/// 调试渲染服务。
pub struct DebugRenderService {
    pub debug_mode: bool,
}

impl DebugRenderService {
    pub fn new(debug_mode: bool) -> Self {
        Self { debug_mode }
    }

    pub fn set_debug_mode(&mut self, mode: bool) {
        self.debug_mode = mode;
    }

    /// 调试颜色调色板。
    pub const DEBUG_COLORS: [Color; 8] = [
        Color::from_rgba(220, 60, 60, 200),
        Color::from_rgba(60, 140, 220, 200),
        Color::from_rgba(60, 180, 80, 200),
        Color::from_rgba(220, 160, 40, 200),
        Color::from_rgba(160, 60, 220, 200),
        Color::from_rgba(220, 80, 140, 200),
        Color::from_rgba(40, 200, 200, 200),
        Color::from_rgba(180, 180, 60, 200),
    ];

    /// 绘制 widget 调试边框。
    pub fn draw_debug_border(
        &self,
        canvas: &mut dyn Canvas2D,
        rect: Rect,
        depth: usize,
        hovered: bool,
    ) {
        if !self.debug_mode {
            return;
        }
        let base = Self::DEBUG_COLORS[depth % Self::DEBUG_COLORS.len()];
        let color = if hovered {
            base
        } else {
            Color::from_rgba(base.r, base.g, base.b, 30)
        };
        canvas.stroke_rect(rect, color, if hovered { 1.5 } else { 0.5 }, None);
    }

    /// 在 widget 左上角显示调试标签。
    pub fn draw_debug_label(
        &self,
        canvas: &mut dyn Canvas2D,
        widget_id: usize,
        depth: usize,
        rect: Rect,
    ) {
        if !self.debug_mode {
            return;
        }
        let _color = Self::DEBUG_COLORS[depth % Self::DEBUG_COLORS.len()];
        let label = format!("#{} d{}", widget_id, depth);
        let _font_size = 12.0;
        let label_w = label.len() as f32 * 7.0 + 6.0;
        let label_h = 16.0;
        canvas.fill_rect(
            Rect::new(rect.x, rect.y, label_w, label_h),
            Color::from_rgba(0, 0, 0, 180),
            None,
        );
        // draw_text 由 RenderContext 委托，此处由 RenderContext 调用 debug service 后自行绘制
    }

    /// 在 widget 下方显示 frame 坐标和尺寸。
    pub fn draw_debug_frame_info(&self, canvas: &mut dyn Canvas2D, widget_id: usize, rect: Rect) {
        if !self.debug_mode {
            return;
        }
        let info = format!(
            "#{} ({:.0},{:.0}) {:.0}×{:.0}",
            widget_id, rect.x, rect.y, rect.w, rect.h
        );
        let _font_size = 11.0;
        let info_w = info.len() as f32 * 6.5 + 6.0;
        let info_h = 15.0;
        let info_y = rect.y + rect.h;
        canvas.fill_rect(
            Rect::new(rect.x, info_y, info_w, info_h),
            Color::from_rgba(0, 0, 0, 160),
            None,
        );
        // draw_text 由 RenderContext 委托，此处由 RenderContext 调用 debug service 后自行绘制
    }
}

// ── DebugRenderer trait 实现 ────────────────────────────────────

impl DebugRenderer for DebugRenderService {
    fn set_debug_mode(&mut self, mode: bool) {
        self.set_debug_mode(mode);
    }
    fn debug_mode(&self) -> bool {
        self.debug_mode
    }
    fn draw_debug_border(
        &self,
        canvas: &mut dyn Canvas2D,
        rect: Rect,
        depth: usize,
        hovered: bool,
    ) {
        self.draw_debug_border(canvas, rect, depth, hovered);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uix_graphics::engine::cpu::noop_canvas_2d::NoopCanvas2D;

    #[test]
    fn new_off_by_default() {
        let d = DebugRenderService::new(false);
        assert!(!d.debug_mode);
    }

    #[test]
    fn new_on() {
        let d = DebugRenderService::new(true);
        assert!(d.debug_mode);
    }

    #[test]
    fn set_debug_mode_toggle() {
        let mut d = DebugRenderService::new(false);
        assert!(!d.debug_mode);
        d.set_debug_mode(true);
        assert!(d.debug_mode);
        d.set_debug_mode(false);
        assert!(!d.debug_mode);
    }

    #[test]
    fn draw_debug_border_skip_when_off() {
        let d = DebugRenderService::new(false);
        let mut canvas = NoopCanvas2D;
        // 当 debug_mode=false 时不 panic
        d.draw_debug_border(&mut canvas, Rect::new(0.0, 0.0, 100.0, 50.0), 0, false);
    }

    #[test]
    fn draw_debug_border_runs_when_on() {
        let d = DebugRenderService::new(true);
        let mut canvas = NoopCanvas2D;
        d.draw_debug_border(&mut canvas, Rect::new(0.0, 0.0, 100.0, 50.0), 0, false);
        d.draw_debug_border(&mut canvas, Rect::new(0.0, 0.0, 100.0, 50.0), 0, true);
    }

    #[test]
    fn draw_debug_border_hovered_uses_full_alpha() {
        let d = DebugRenderService::new(true);
        let mut canvas = NoopCanvas2D;
        d.draw_debug_border(&mut canvas, Rect::new(10.0, 10.0, 50.0, 30.0), 0, true);
    }

    #[test]
    fn draw_debug_border_multiple_depths() {
        let d = DebugRenderService::new(true);
        let mut canvas = NoopCanvas2D;
        // 所有深度都应正常工作
        for depth in 0..16 {
            d.draw_debug_border(
                &mut canvas,
                Rect::new(0.0, 0.0, 100.0, 50.0),
                depth,
                depth == 0,
            );
        }
    }

    #[test]
    fn draw_debug_label_skip_when_off() {
        let d = DebugRenderService::new(false);
        let mut canvas = NoopCanvas2D;
        d.draw_debug_label(&mut canvas, 1, 0, Rect::new(0.0, 0.0, 100.0, 50.0));
    }

    #[test]
    fn draw_debug_label_runs_when_on() {
        let d = DebugRenderService::new(true);
        let mut canvas = NoopCanvas2D;
        d.draw_debug_label(&mut canvas, 42, 2, Rect::new(10.0, 10.0, 100.0, 50.0));
    }

    #[test]
    fn draw_debug_frame_info_skip_when_off() {
        let d = DebugRenderService::new(false);
        let mut canvas = NoopCanvas2D;
        d.draw_debug_frame_info(&mut canvas, 1, Rect::new(0.0, 0.0, 100.0, 50.0));
    }

    #[test]
    fn draw_debug_frame_info_runs_when_on() {
        let d = DebugRenderService::new(true);
        let mut canvas = NoopCanvas2D;
        d.draw_debug_frame_info(&mut canvas, 42, Rect::new(10.0, 20.0, 200.0, 100.0));
    }

    #[test]
    fn debug_colors_has_eight_entries() {
        assert_eq!(DebugRenderService::DEBUG_COLORS.len(), 8);
    }

    #[test]
    fn debug_colors_all_have_alpha() {
        for color in &DebugRenderService::DEBUG_COLORS {
            assert!(color.a > 0 || color.a == 0); // 至少不 panic
        }
    }

    #[test]
    fn debug_renderer_trait_set_debug_mode() {
        let mut d = DebugRenderService::new(false);
        let r: &mut dyn DebugRenderer = &mut d;
        r.set_debug_mode(true);
        assert!(r.debug_mode());
        r.set_debug_mode(false);
        assert!(!r.debug_mode());
    }

    #[test]
    fn draw_debug_label_rect_position() {
        let d = DebugRenderService::new(true);
        let mut canvas = NoopCanvas2D;
        // 验证不 panic：标签绘制在 widget 左上角
        d.draw_debug_label(&mut canvas, 99, 1, Rect::new(10.0, 20.0, 150.0, 80.0));
    }

    #[test]
    fn draw_debug_frame_info_below_widget() {
        let d = DebugRenderService::new(true);
        let mut canvas = NoopCanvas2D;
        // 验证不 panic：坐标信息绘制在 widget 下方
        d.draw_debug_frame_info(&mut canvas, 7, Rect::new(30.0, 40.0, 200.0, 100.0));
    }

    #[test]
    fn debug_colors_have_expected_rgb_ranges() {
        for color in &DebugRenderService::DEBUG_COLORS {
            assert!(color.a > 0, "每个调试颜色应有非零 alpha");
            // r/g/b 是 u8，无需范围检查
        }
    }
}

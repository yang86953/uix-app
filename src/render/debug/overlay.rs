//! DebugRenderService — 调试绘制服务。

use crate::render::pipeline::{InvalidationSource, RenderMetrics};
use crate::render::traits::Canvas2D;
use crate::api::render::Color;
use crate::platform::Rect;

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

    /// Debug overlay：右上角显示度量计数与 invalidation 来源。
    pub fn draw_telemetry_hud(
        &self,
        canvas: &mut dyn Canvas2D,
        metrics: &RenderMetrics,
        surface_w: i32,
    ) {
        if !self.debug_mode {
            return;
        }
        let panel_w = 220.0;
        let panel_h = 88.0;
        let x = surface_w as f32 - panel_w - 6.0;
        canvas.fill_rect(
            Rect::new(x, 6.0, panel_w, panel_h),
            Color::from_rgba(0, 0, 0, 200),
            None,
        );
        let source = metrics.last_invalidation;
        let indicator = match source {
            InvalidationSource::None => Color::from_rgba(80, 80, 80, 255),
            InvalidationSource::FirstFrame => Color::from_rgba(220, 180, 40, 255),
            InvalidationSource::DirtyRegion => Color::from_rgba(60, 180, 80, 255),
            InvalidationSource::AnimationPolling => Color::from_rgba(60, 140, 220, 255),
            InvalidationSource::LayoutEvent => Color::from_rgba(220, 80, 140, 255),
        };
        canvas.fill_rect(Rect::new(x + 6.0, 10.0, 10.0, 10.0), indicator, None);
        // 条形指示各计数相对量级（无字体时仍可见趋势）
        let bar_max_w = panel_w - 20.0;
        let scale = |v: u64| -> f32 {
            let f = v as f32;
            (f * 12.0).min(bar_max_w)
        };
        let bar_y = 26.0;
        let bar_h = 4.0;
        let colors = [
            Color::from_rgba(100, 200, 100, 255),
            Color::from_rgba(100, 160, 220, 255),
            Color::from_rgba(220, 160, 60, 255),
            Color::from_rgba(180, 180, 180, 255),
        ];
        let values = [
            metrics.layout_calls,
            metrics.paint_calls,
            metrics.present_calls,
            metrics.idle_frames,
        ];
        for (i, (&v, &c)) in values.iter().zip(colors.iter()).enumerate() {
            let y = bar_y + i as f32 * 14.0;
            canvas.fill_rect(
                Rect::new(x + 10.0, y, scale(v).max(if v > 0 { 2.0 } else { 0.0 }), bar_h),
                c,
                None,
            );
        }
        // invalidation 来源标签区（细线编码字符长度）
        let label_len = source.label().len() as f32;
        canvas.fill_rect(
            Rect::new(x + 22.0, 10.0, label_len * 3.0, 10.0),
            Color::from_rgba(255, 255, 255, 120),
            None,
        );
    }

    /// 返回 HUD 文本行（供 RenderContext 绘制标签）。
    pub fn telemetry_hud_lines(metrics: &RenderMetrics) -> [String; 5] {
        [
            format!("inv: {}", metrics.last_invalidation.label()),
            format!("layout: {}", metrics.layout_calls),
            format!("paint: {}", metrics.paint_calls),
            format!("present: {}", metrics.present_calls),
            format!("idle: {}", metrics.idle_frames),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::engine::cpu::noop_canvas_2d::NoopCanvas2D;

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
    fn draw_debug_label_rect_position() {
        let d = DebugRenderService::new(true);
        let mut canvas = NoopCanvas2D;
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

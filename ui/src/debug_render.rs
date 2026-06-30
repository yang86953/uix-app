//! DebugRenderService — 调试绘制服务。
//!
//! 提供 widget 调试边框、标签和 frame 信息的绘制。
//! 由 RenderContext 组合持有。

use uix_platform::Rect;
use uix_graphics::traits::Canvas2D;
use uix_graphics::Color;
use crate::api::traits::DebugRenderer;

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
        if !self.debug_mode { return; }
        let _color = Self::DEBUG_COLORS[depth % Self::DEBUG_COLORS.len()];
        let label = format!("#{} d{}", widget_id, depth);
        let _font_size = 12.0;
        let label_w = label.len() as f32 * 7.0 + 6.0;
        let label_h = 16.0;
        canvas.fill_rect(Rect::new(rect.x, rect.y, label_w, label_h), Color::from_rgba(0, 0, 0, 180), None);
        // draw_text 由 RenderContext 委托，此处由 RenderContext 调用 debug service 后自行绘制
    }

    /// 在 widget 下方显示 frame 坐标和尺寸。
    pub fn draw_debug_frame_info(
        &self,
        canvas: &mut dyn Canvas2D,
        widget_id: usize,
        rect: Rect,
    ) {
        if !self.debug_mode { return; }
        let info = format!("#{} ({:.0},{:.0}) {:.0}×{:.0}", widget_id, rect.x, rect.y, rect.w, rect.h);
        let _font_size = 11.0;
        let info_w = info.len() as f32 * 6.5 + 6.0;
        let info_h = 15.0;
        let info_y = rect.y + rect.h;
        canvas.fill_rect(Rect::new(rect.x, info_y, info_w, info_h), Color::from_rgba(0, 0, 0, 160), None);
        // draw_text 由 RenderContext 委托，此处由 RenderContext 调用 debug service 后自行绘制
    }
}

// ── DebugRenderer trait 实现 ────────────────────────────────────

impl DebugRenderer for DebugRenderService {
    fn set_debug_mode(&mut self, mode: bool) { self.set_debug_mode(mode); }
    fn debug_mode(&self) -> bool { self.debug_mode }
    fn draw_debug_border(&self, canvas: &mut dyn Canvas2D, rect: Rect, depth: usize, hovered: bool) {
        self.draw_debug_border(canvas, rect, depth, hovered);
    }
}

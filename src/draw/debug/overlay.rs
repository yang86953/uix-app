//! DebugRenderService — 调试绘制服务。

use crate::core::Rect;
use crate::draw::pipeline::{InvalidationSource, RenderMetrics};
use crate::draw::traits::Canvas2D;
use crate::draw::Color;

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

    /// 绘制节点调试边框（仅 hover 链调用；非 hover 直接跳过，避免满屏淡彩框）。
    pub fn draw_debug_border(
        &self,
        canvas: &mut dyn Canvas2D,
        rect: Rect,
        depth: usize,
        hovered: bool,
    ) {
        if !self.debug_mode || !hovered {
            return;
        }
        let base = Self::DEBUG_COLORS[depth % Self::DEBUG_COLORS.len()];
        canvas.stroke_rect(rect, base, 1.5, None);
    }

    /// 在节点左上角显示调试标签。
    pub fn draw_debug_label(
        &self,
        canvas: &mut dyn Canvas2D,
        node_slot: usize,
        depth: usize,
        rect: Rect,
    ) {
        if !self.debug_mode {
            return;
        }
        let _color = Self::DEBUG_COLORS[depth % Self::DEBUG_COLORS.len()];
        let label = format!("#{} d{}", node_slot, depth);
        let _font_size = 12.0;
        let label_w = label.len() as f32 * 7.0 + 6.0;
        let label_h = 16.0;
        canvas.fill_rect(
            Rect::new(rect.x, rect.y, label_w, label_h),
            Color::from_rgba(0, 0, 0, 180),
            None,
        );
        // draw_text 由 PaintContext 委托，此处由 PaintContext 调用 debug service 后自行绘制
    }

    /// 在 widget 下方显示 frame 坐标和尺寸。
    pub fn draw_debug_frame_info(&self, canvas: &mut dyn Canvas2D, node_slot: usize, rect: Rect) {
        if !self.debug_mode {
            return;
        }
        let info = format!(
            "#{} ({:.0},{:.0}) {:.0}×{:.0}",
            node_slot, rect.x, rect.y, rect.w, rect.h
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
        // draw_text 由 PaintContext 委托，此处由 PaintContext 调用 debug service 后自行绘制
    }

    /// HUD 面板几何（与 `telemetry_hud_lines` / 文字绘制共用，避免错位）。
    pub const HUD_PANEL_W: f32 = 228.0;
    pub const HUD_PANEL_H: f32 = 118.0;
    pub const HUD_MARGIN: f32 = 6.0;
    pub const HUD_PAD_X: f32 = 10.0;
    pub const HUD_LINE_H: f32 = 14.0;
    pub const HUD_TEXT_TOP: f32 = 12.0;

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
        let x = surface_w as f32 - Self::HUD_PANEL_W - Self::HUD_MARGIN;
        let y = Self::HUD_MARGIN;
        canvas.fill_rect(
            Rect::new(x, y, Self::HUD_PANEL_W, Self::HUD_PANEL_H),
            Color::from_rgba(0, 0, 0, 210),
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
        // 色条在文字左侧作次要指示；主信息靠 telemetry_hud_lines 文字。
        let bar_x = x + Self::HUD_PAD_X;
        let bar_w_max = 36.0;
        let scale = |v: u64| -> f32 {
            if v == 0 {
                0.0
            } else {
                ((v as f32).ln_1p() * 8.0).min(bar_w_max).max(2.0)
            }
        };
        let colors = [
            indicator,
            Color::from_rgba(100, 200, 100, 255),
            Color::from_rgba(100, 160, 220, 255),
            Color::from_rgba(220, 160, 60, 255),
            Color::from_rgba(180, 180, 180, 255),
            Color::from_rgba(140, 140, 160, 255),
        ];
        let values = [
            1u64, // inv 指示点
            metrics.layout_calls,
            metrics.paint_calls,
            metrics.present_calls,
            metrics.idle_frames,
            0, // 快捷键行无条
        ];
        for (i, (&v, &c)) in values.iter().zip(colors.iter()).enumerate() {
            let line_y = y + Self::HUD_TEXT_TOP + i as f32 * Self::HUD_LINE_H;
            if i == 0 {
                canvas.fill_rect(Rect::new(bar_x, line_y, 8.0, 8.0), c, None);
            } else if i < 5 {
                canvas.fill_rect(
                    Rect::new(bar_x, line_y + 4.0, scale(v), 3.0),
                    c,
                    None,
                );
            }
        }
    }

    /// HUD 面板左上角 x（与 `draw_telemetry_hud` 一致）。
    pub fn hud_panel_x(surface_w: i32) -> f32 {
        surface_w as f32 - Self::HUD_PANEL_W - Self::HUD_MARGIN
    }

    /// 返回 HUD 文本行（供 FrameRenderer 绘制标签）。
    pub fn telemetry_hud_lines(metrics: &RenderMetrics) -> [String; 6] {
        [
            format!("inv: {}", metrics.last_invalidation.label()),
            format!("layout: {}", metrics.layout_calls),
            format!("paint: {}", metrics.paint_calls),
            format!("present: {}", metrics.present_calls),
            format!("idle: {}", metrics.idle_frames),
            "toggle: Ctrl+Shift+D".to_string(),
        ]
    }
}

#[cfg(test)]
#[path = "../../tests/draw/debug/overlay.rs"]
mod tests;

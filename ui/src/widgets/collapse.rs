//! Collapse widget — 折叠面板。

use crate::define_widget;
use uix_platform::{Rect, Size};
use uix_graphics::Radius;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};

/// 单个折叠面板。
#[derive(Debug, Clone)]
pub struct CollapsePanel {
    pub header: String,
    pub content: String,
    pub expanded: bool,
}

impl CollapsePanel {
    pub fn new(header: impl Into<String>, content: impl Into<String>) -> Self {
        Self { header: header.into(), content: content.into(), expanded: false }
    }
    pub fn expanded(mut self) -> Self { self.expanded = true; self }
}

define_widget! {
    /// Collapse — 可折叠面板组。
    pub struct Collapse {
        panels: Vec<CollapsePanel>,
        accordion: bool,
        on_change: Option<Box<dyn FnMut(usize) + 'static>>,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        let mut h = 0.0f32;
        for p in &self.panels {
            h += 36.0; // header 高度（14px 字体 + padding）
            if p.expanded {
                // 内容高度 = 行数 × 行高（12px × 1.5）+ 上下 padding（8+8）
                // 与 render 中 draw_text(12.0) 保持一致，不再使用虚构字符宽度估算。
                let line_count = p.content.lines().count().max(1) as f32;
                h += line_count * 12.0 * 1.5 + 16.0;
            }
        }
        Size::new(0.0, h)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            let mut cy = 0.0f32;
            let panel_count = self.panels.len();
            for i in 0..panel_count {
                let header_h = 36.0f32;
                if pos.y >= cy && pos.y <= cy + header_h {
                    let new_state = !self.panels[i].expanded;
                    let name = self.panels[i].header.clone();
                    if self.accordion {
                        for p in &mut self.panels {
                            p.expanded = false;
                        }
                    }
                    self.panels[i].expanded = new_state;
                    if let Some(ref mut cb) = self.on_change { cb(i); }
                    log::debug!(
                        "[Collapse] 面板 \"{}\" 切换 expanded: {} → {}",
                        name, !new_state, new_state
                    );
                    return EventResult::Handled;
                }
                cy += header_h;
                if self.panels[i].expanded {
                    let lc = self.panels[i].content.lines().count().max(1) as f32;
                    cy += lc * 12.0 * 1.5 + 16.0;
                }
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        let mut y = frame.y;

        for p in &self.panels {
            let header_rect = Rect::new(frame.x, y, frame.w, 36.0);
            // header 背景
            ctx.fill_rect(header_rect, bg, r);
            ctx.stroke_rect(header_rect, border, 1.0, r);
            // 展开指示符
            let arrow = if p.expanded { "▼" } else { "▶" };
            let arrow_y = ctx.visual_center_y(header_rect, 12.0);
            ctx.draw_text(arrow, uix_platform::Point::new(frame.x + 10.0, arrow_y), text_secondary, 12.0);
            let header_y = ctx.visual_center_y(header_rect, 14.0);
            ctx.draw_text(&p.header, uix_platform::Point::new(frame.x + 28.0, header_y), text_color, 14.0);
            y += 36.0;

            if p.expanded {
                let content_y = y + 8.0;
                ctx.draw_text(&p.content, uix_platform::Point::new(frame.x + 16.0, content_y), text_secondary, 12.0);
                let lc = p.content.lines().count().max(1) as f32;
                y += lc * 12.0 * 1.5 + 16.0;
            }
        }
    }

    // NOTE(布局): dirty_rect 目前返回所有面板最大展开时的全量区域（frame），
    // 而不是仅返回变化区域（delta）。因为 collapse 无法可靠追踪哪个面板的
    // expanded 状态在上帧到本帧之间发生了变化（on_event 中修改 expanded 时
    // 未保存旧状态），返回全量可确保展开/折叠时残留像素被清除。
    // 优化方向：在 on_event 中记录 changed_panel index，dirty_rect 仅返回
    // 该 header + 内容区域的变化部分。
    // 始终包含最大展开高度，确保 expanded 切换时残留像素被清除
    dirty_rect => (&self, frame: Rect) -> Rect {
        let mut h = self.panels.len() as f32 * 36.0;
        for p in &self.panels {
            let lc = p.content.lines().count().max(1) as f32;
            h += lc * 12.0 * 1.5 + 16.0;
        }
        Rect::new(frame.x, frame.y, frame.w, h)
    }
}

impl Default for Collapse { fn default() -> Self { Self::new() } }

impl Collapse {
    pub fn new() -> Self {
        Self { panels: Vec::new(), accordion: false, on_change: None }
    }
    pub fn panels(mut self, ps: Vec<CollapsePanel>) -> Self { self.panels = ps; self }
    pub fn accordion(mut self) -> Self { self.accordion = true; self }
    pub fn on_change<F: FnMut(usize) + 'static>(mut self, f: F) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
}

//! Collapse widget — 折叠面板。

use crate::define_widget;
use crate::base::{Rect, Size};
use crate::graphics::Radius;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

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
    /// 内容在 post_render 中渲染为浮层，不触发布局偏移。
    pub struct Collapse {
        panels: Vec<CollapsePanel>,
        accordion: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        // 固定为 header 总高度，不随 expanded 变化，避免布局偏移
        let h = self.panels.len() as f32 * 36.0;
        Size::new(300.0, h)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            let panel_count = self.panels.len();
            for i in 0..panel_count {
                let y0 = i as f32 * 36.0;
                if pos.y >= y0 && pos.y <= y0 + 36.0 {
                    if self.accordion {
                        for p in &mut self.panels {
                            p.expanded = false;
                        }
                    }
                    self.panels[i].expanded = !self.panels[i].expanded;
                    return EventResult::Handled;
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
            ctx.fill_rect(header_rect, bg, r);
            ctx.stroke_rect(header_rect, border, 1.0, r);
            let arrow = if p.expanded { "▼" } else { "▶" };
            ctx.draw_text(arrow, crate::base::Point::new(frame.x + 10.0, y + 10.0), text_secondary, 12.0);
            ctx.draw_text(&p.header, crate::base::Point::new(frame.x + 28.0, y + 9.0), text_color, 14.0);
            y += 36.0;
        }
    }

    // 内容作为浮层渲染（不影响布局定位）
    post_render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let text_secondary = ctx.tokens().color_text_secondary();
        let mut y = frame.y;

        for p in &self.panels {
            y += 36.0; // header
            if p.expanded {
                let content_y = y + 8.0;
                ctx.draw_text(&p.content, crate::base::Point::new(frame.x + 16.0, content_y), text_secondary, 12.0);
                y += p.content.len() as f32 * 0.4 * 14.0 + 16.0;
            }
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // 始终包含最大展开内容区域，确保 expanded 切换时残留像素被清除
        let mut h = self.panels.len() as f32 * 36.0;
        for p in &self.panels {
            h += p.content.len() as f32 * 0.4 * 14.0 + 16.0;
        }
        Rect::new(frame.x, frame.y, frame.w, h)
    }
}

impl Default for Collapse { fn default() -> Self { Self::new() } }

impl Collapse {
    pub fn new() -> Self {
        Self { panels: Vec::new(), accordion: false }
    }
    pub fn panels(mut self, ps: Vec<CollapsePanel>) -> Self { self.panels = ps; self }
    pub fn accordion(mut self) -> Self { self.accordion = true; self }
}

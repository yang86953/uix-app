//! Modal widget — overlay dialog with backdrop and optional close button.

use crate::graphics::{Color, Point, Radius, Rect, Size};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, Widget, WidgetEvent, WidgetTree};

/// Modal dialog with title, body, and footer areas.
pub struct Modal {
    title: String,
    visible: bool,
    width: f32,
    height: f32,
    closable: bool,
    mask_closable: bool,
    footer_visible: bool,
}

impl Modal {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            visible: false,
            width: 520.0,
            height: 300.0,
            closable: true,
            mask_closable: true,
            footer_visible: true,
        }
    }

    pub fn visible(mut self, v: bool) -> Self {
        self.visible = v;
        self
    }

    pub fn show(mut self) -> Self {
        self.visible = true;
        self
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = w;
        self.height = h;
        self
    }

    pub fn closable(mut self, v: bool) -> Self {
        self.closable = v;
        self
    }

    pub fn mask_closable(mut self, v: bool) -> Self {
        self.mask_closable = v;
        self
    }

    pub fn footer_visible(mut self, v: bool) -> Self {
        self.footer_visible = v;
        self
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, v: bool) {
        self.visible = v;
    }

    pub fn open(&mut self) {
        self.visible = true;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }
}

impl Widget for Modal {
    fn preferred_size(&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        if self.visible {
            Size::new(self.width, self.height)
        } else {
            Size::zero()
        }
    }

    fn on_event(&mut self, event: &WidgetEvent) -> EventResult {
        if !self.visible {
            return EventResult::NotHandled;
        }
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                // Check if clicked on close button area
                let close_x = self.width - 48.0;
                let close_rect = Rect::new(close_x, 0.0, 48.0, 48.0);
                if self.closable && close_rect.contains(*pos) {
                    self.close();
                    return EventResult::Handled;
                }
                // Check if clicked on mask (outside the dialog)
                if self.mask_closable && (pos.x < 0.0 || pos.y < 0.0) {
                    self.close();
                    return EventResult::Handled;
                }
                EventResult::Handled // Consume events when modal is open
            }
            WidgetEvent::KeyDown { key } => {
                if *key == crate::ui::widget::KeyCode::Escape && self.closable {
                    self.close();
                    return EventResult::Handled;
                }
                EventResult::Handled
            }
            _ => {
                if self.visible {
                    EventResult::Handled // Block events from passing through modal
                } else {
                    EventResult::NotHandled
                }
            }
        }
    }

    fn render(&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if !self.visible {
            return;
        }

        // Extract all token values upfront to avoid borrow conflict with ctx
        let border_radius_lg = ctx.tokens().border_radius_lg();
        let bg_container = ctx.tokens().color_bg_container();
        let border_secondary = ctx.tokens().color_border_secondary();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();

        // Backdrop mask
        ctx.fill_rect(
            Rect::new(
                frame.x - 1000.0,
                frame.y - 1000.0,
                frame.w + 2000.0,
                frame.h + 2000.0,
            ),
            Color::from_rgba(0, 0, 0, 128),
            None,
        );

        // Dialog card
        let radius = Some(Radius::uniform(border_radius_lg));

        // Shadow/background
        ctx.fill_rect(frame, bg_container, radius);

        // Border
        ctx.stroke_rect(frame, border_secondary, 1.0, radius);

        let title_h = 56.0;
        let footer_h = if self.footer_visible { 56.0 } else { 0.0 };

        // ── Header ──
        ctx.draw_text(
            &self.title,
            Point::new(frame.x + 24.0, frame.y + 16.0),
            text_color,
            16.0,
        );

        // Separator
        ctx.fill_rect(
            Rect::new(frame.x, frame.y + title_h, frame.w, 1.0),
            border_secondary,
            None,
        );

        // Close button
        if self.closable {
            let cx = frame.x + frame.w - 36.0;
            let cy = frame.y + 16.0;
            ctx.draw_text("✕", Point::new(cx, cy), text_secondary, 16.0);
        }

        // ── Footer (placeholder) ──
        if self.footer_visible {
            ctx.fill_rect(
                Rect::new(frame.x, frame.y + frame.h - footer_h, frame.w, 1.0),
                border_secondary,
                None,
            );
            ctx.draw_text(
                "",
                Point::new(frame.x + 24.0, frame.y + frame.h - footer_h + 16.0),
                text_secondary,
                14.0,
            );
        }
    }

    fn layout_children(
        &self,
        frame: Rect,
        children: &[crate::ui::widget::WidgetId],
        _tree: &WidgetTree,
    ) -> Vec<(crate::ui::widget::WidgetId, Rect)> {
        if !self.visible || children.is_empty() {
            return Vec::new();
        }

        let title_h = 56.0;
        let footer_h = if self.footer_visible { 56.0 } else { 0.0 };
        let body_y = frame.y + title_h;
        let body_h = frame.h - title_h - footer_h;

        let mut result = Vec::new();
        // Place children in the body area
        let padding = 24.0;
        for &cid in children {
            result.push((
                cid,
                Rect::new(
                    frame.x + padding,
                    body_y + padding,
                    frame.w - padding * 2.0,
                    body_h - padding * 2.0,
                ),
            ));
        }
        result
    }
}

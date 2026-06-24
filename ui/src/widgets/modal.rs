//! Modal widget — overlay dialog with backdrop and optional close button.

use crate::define_widget;
use uix_graphics::{Color, Radius};
use uix_core::{Point, Rect, Size};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};

define_widget! {
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

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        if self.visible { Size::new(self.width, self.height) } else { Size::zero() }
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if !self.visible { return EventResult::NotHandled; }
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                let close_rect = Rect::new(self.width - 48.0, 0.0, 48.0, 48.0);
                if self.closable && close_rect.contains(*pos) { self.close(); return EventResult::Handled; }
                if self.mask_closable && (pos.x < 0.0 || pos.y < 0.0) { self.close(); return EventResult::Handled; }
                EventResult::Handled
            }
            WidgetEvent::KeyDown { key, .. } => {
                if *key == crate::widget::KeyCode::Escape && self.closable { self.close(); return EventResult::Handled; }
                EventResult::Handled
            }
            _ => { if self.visible { EventResult::Handled } else { EventResult::NotHandled } }
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if !self.visible { return; }

        let border_radius_lg = ctx.tokens().border_radius_lg();
        let bg_container = ctx.tokens().color_bg_container();
        let border_secondary = ctx.tokens().color_border_secondary();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();

        ctx.fill_rect(Rect::new(frame.x - 1000.0, frame.y - 1000.0, frame.w + 2000.0, frame.h + 2000.0),
            Color::from_rgba(0, 0, 0, 128), None);

        let radius = Some(Radius::uniform(border_radius_lg));
        ctx.fill_rect(frame, bg_container, radius);
        ctx.stroke_rect(frame, border_secondary, 1.0, radius);

        let title_h = 56.0;
        let footer_h = if self.footer_visible { 56.0 } else { 0.0 };

        let title_rect = Rect::new(frame.x, frame.y, frame.w, title_h);
        let ty = ctx.visual_center_y(title_rect, 16.0);
        ctx.draw_text(&self.title, Point::new(frame.x + 24.0, ty), text_color, 16.0);
        ctx.fill_rect(Rect::new(frame.x, frame.y + title_h, frame.w, 1.0), border_secondary, None);

        if self.closable {
            ctx.draw_text("✕", Point::new(frame.x + frame.w - 36.0, ty), text_secondary, 16.0);
        }
        if self.footer_visible {
            ctx.fill_rect(Rect::new(frame.x, frame.y + frame.h - footer_h, frame.w, 1.0), border_secondary, None);
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::widget::WidgetId], _tree: &WidgetTree)
        -> Vec<(crate::widget::WidgetId, Rect)>
    {
        if !self.visible || children.is_empty() { return Vec::new(); }
        let title_h = 56.0;
        let footer_h = if self.footer_visible { 56.0 } else { 0.0 };
        let body_y = frame.y + title_h;
        let body_h = frame.h - title_h - footer_h;
        let padding = 24.0;
        children.iter().map(|&cid| (cid, Rect::new(frame.x + padding, body_y + padding, frame.w - padding * 2.0, body_h - padding * 2.0))).collect()
    }
}

impl Modal {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            visible: false, width: 520.0, height: 300.0,
            closable: true, mask_closable: true, footer_visible: true,
        }
    }
    pub fn visible(mut self, v: bool) -> Self { self.visible = v; self }
    pub fn show(mut self) -> Self { self.visible = true; self }
    pub fn size(mut self, w: f32, h: f32) -> Self { self.width = w; self.height = h; self }
    pub fn closable(mut self, v: bool) -> Self { self.closable = v; self }
    pub fn mask_closable(mut self, v: bool) -> Self { self.mask_closable = v; self }
    pub fn footer_visible(mut self, v: bool) -> Self { self.footer_visible = v; self }
    pub fn is_visible(&self) -> bool { self.visible }
    pub fn set_visible(&mut self, v: bool) { self.visible = v; }
    pub fn open(&mut self) { self.visible = true; }
    pub fn close(&mut self) { self.visible = false; }
}

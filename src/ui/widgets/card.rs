//! Card widget — Ant Design style container with elevation, shadow, optional
//! title, body, hover feedback, and configurable border radius.

use crate::define_widget;
use crate::graphics::{Color, Point, Radius, Rect, Size};
use crate::ui::children::WidgetChildren;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, Widget, WidgetEvent, WidgetId, WidgetTree};

define_widget! {
    /// Card widget with shadow elevation, hover highlight, and content padding.
    pub struct Card {
        title: Option<String>,
        children: WidgetChildren,
        bordered: bool,
        hoverable: bool,
        hovered: bool,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        padding: f32,
        elevation: u8,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(self.fixed_width.unwrap_or(200.0), self.fixed_height.unwrap_or(0.0))
    }

    build => (&self) -> Vec<Box<dyn Widget>> {
        self.children.take()
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if self.hoverable {
            match event {
                WidgetEvent::HoverEnter => { self.hovered = true; EventResult::Handled }
                WidgetEvent::HoverLeave => { self.hovered = false; EventResult::Handled }
                _ => EventResult::NotHandled,
            }
        } else {
            EventResult::NotHandled
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let border_radius_lg = ctx.tokens().border_radius_lg();
        let bg_container = ctx.tokens().color_bg_container();
        let bg_elevated = ctx.tokens().color_bg_elevated();
        let primary = ctx.tokens().color_primary();
        let border_secondary = ctx.tokens().color_border_secondary();
        let text = ctx.tokens().color_text();

        let card_radius = Some(Radius::uniform(border_radius_lg));

        // Shadow
        draw_elevation_shadow(ctx, frame, self.elevation);

        // Background
        let bg = if self.hovered {
            Color::from_rgb(
                (bg_container.r as f32 * 0.95 + 255.0 * 0.05) as u8,
                (bg_container.g as f32 * 0.95 + 255.0 * 0.05) as u8,
                (bg_container.b as f32 * 0.95 + 255.0 * 0.05) as u8,
            )
        } else {
            bg_elevated
        };
        ctx.fill_rect(frame, bg, card_radius);

        // Top accent line
        if self.elevation > 1 {
            let accent_rect = Rect::new(frame.x + 24.0, frame.y, frame.w - 48.0, 3.0);
            ctx.fill_rect(accent_rect, primary, Some(Radius::uniform(1.5)));
        }

        // Border
        if self.bordered {
            ctx.stroke_rect(frame, border_secondary, 1.0, card_radius);
        }

        // Title
        if let Some(ref title) = self.title {
            ctx.draw_text(title, Point::new(frame.x + self.padding, frame.y + 12.0), text, 15.0);
            let sep_y = frame.y + 44.0 + 4.0;
            ctx.fill_rect(Rect::new(frame.x + self.padding, sep_y, frame.w - self.padding * 2.0, 1.0), border_secondary, None);
        }
    }

    layout_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        let mut result = Vec::new();
        if children.is_empty() { return result; }

        let title_offset = if self.title.is_some() { 56.0 } else { self.padding };
        let inner = Rect::new(
            frame.x + self.padding, frame.y + title_offset,
            (frame.w - self.padding * 2.0).max(0.0),
            (frame.h - title_offset - self.padding).max(0.0),
        );
        if inner.w <= 0.0 || inner.h <= 0.0 { return result; }

        let mut cursor_y = inner.y;
        for &cid in children {
            let pref = tree.get(cid).map(|c| c.preferred_size(None)).unwrap_or_else(Size::zero);
            let ch = if pref.h > 0.0 { pref.h.min(inner.h) } else { inner.h / children.len() as f32 };
            result.push((cid, Rect::new(inner.x, cursor_y, inner.w, ch)));
            cursor_y += ch;
        }
        if !result.is_empty() {
            let last = result.len() - 1;
            let last_rect = &mut result[last].1;
            if last_rect.y + last_rect.h < inner.y + inner.h {
                last_rect.h = inner.y + inner.h - last_rect.y;
            }
        }
        result
    }
}

/// Render a real box shadow from the theme's `ShadowToken` layers.
fn draw_elevation_shadow(ctx: &mut RenderContext, frame: Rect, elevation: u8) {
    if elevation == 0 { return; }
    let shadow = ctx.tokens().box_shadow();
    let corner_radius = Some(Radius::uniform(ctx.tokens().border_radius_lg()));
    let scale = match elevation { 1 => 0.6, 2 => 0.8, 3 => 1.0, _ => return };

    let (ox1, oy1, bl1, col1) = shadow.layer_1;
    if bl1 > 0.0 && col1.a > 0 { ctx.draw_box_shadow(frame, bl1 * scale, ox1 * scale, oy1 * scale, col1, corner_radius); }
    let (ox2, oy2, bl2, col2) = shadow.layer_2;
    if bl2 > 0.0 && col2.a > 0 { ctx.draw_box_shadow(frame, bl2 * scale, ox2 * scale, oy2 * scale, col2, corner_radius); }
    let (ox3, oy3, bl3, col3) = shadow.layer_3;
    if bl3 > 0.0 && col3.a > 0 { ctx.draw_box_shadow(frame, bl3 * scale, ox3 * scale, oy3 * scale, col3, corner_radius); }
}

impl Default for Card {
    fn default() -> Self { Self::new() }
}

impl Card {
    pub fn new() -> Self {
        Self {
            title: None,
            children: WidgetChildren::new(),
            bordered: true,
            hoverable: false,
            hovered: false,
            fixed_width: None,
            fixed_height: None,
            padding: 16.0,
            elevation: 1,
        }
    }

    pub fn title(mut self, t: &str) -> Self { self.title = Some(t.to_string()); self }
    pub fn bordered(mut self, v: bool) -> Self { self.bordered = v; self }
    pub fn hoverable(mut self) -> Self { self.hoverable = true; self }
    pub fn size(mut self, w: f32, h: f32) -> Self { self.fixed_width = Some(w); self.fixed_height = Some(h); self }
    pub fn padding(mut self, p: f32) -> Self { self.padding = p; self }
    pub fn elevation(mut self, e: u8) -> Self { self.elevation = e.min(3); self }
    pub fn child(self, w: impl Widget + 'static) -> Self {
        self.children.add(w);
        self
    }
    pub fn children(self, widgets: Vec<Box<dyn Widget>>) -> Self {
        self.children.set_all(widgets);
        self
    }
}

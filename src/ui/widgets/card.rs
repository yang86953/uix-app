//! Card widget — Ant Design style container with elevation, shadow, optional
//! title, body, hover feedback, and configurable border radius.

use std::cell::RefCell;

use crate::graphics::{Color, Point, Radius, Rect, Size};
use crate::ui::render_context::RenderContext;
use crate::ui::theme::DesignTokens;
use crate::ui::widget::{EventResult, Widget, WidgetEvent, WidgetId, WidgetTree};

/// Card widget with shadow elevation, hover highlight, and content padding.
pub struct Card {
    title: Option<String>,
    children: RefCell<Option<Vec<Box<dyn Widget>>>>,
    bordered: bool,
    hoverable: bool,
    hovered: bool,
    fixed_width: Option<f32>,
    fixed_height: Option<f32>,
    padding: f32,
    /// Elevation level 0-3: 0=flat, 1=subtle shadow, 2=medium, 3=prominent
    elevation: u8,
}

impl Card {
    pub fn new() -> Self {
        Self {
            title: None,
            children: RefCell::new(None),
            bordered: true,
            hoverable: false,
            hovered: false,
            fixed_width: None,
            fixed_height: None,
            padding: 16.0,
            elevation: 1,
        }
    }

    pub fn title(mut self, t: &str) -> Self {
        self.title = Some(t.to_string());
        self
    }
    pub fn bordered(mut self, v: bool) -> Self {
        self.bordered = v;
        self
    }
    pub fn hoverable(mut self) -> Self {
        self.hoverable = true;
        self
    }
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }
    pub fn padding(mut self, p: f32) -> Self {
        self.padding = p;
        self
    }
    pub fn elevation(mut self, e: u8) -> Self {
        self.elevation = e.min(3);
        self
    }
    pub fn child(self, w: impl Widget + 'static) -> Self {
        if self.children.borrow().is_none() {
            *self.children.borrow_mut() = Some(Vec::new());
        }
        self.children
            .borrow_mut()
            .as_mut()
            .map(|v| v.push(Box::new(w)));
        self
    }
    pub fn children(self, widgets: Vec<Box<dyn Widget>>) -> Self {
        *self.children.borrow_mut() = Some(widgets);
        self
    }
}

/// Simulate a box shadow by drawing offset layered rectangles.
fn draw_shadow(
    ctx: &mut RenderContext,
    frame: Rect,
    elevation: u8,
    tokens: &DesignTokens,
) {
    if elevation == 0 {
        return;
    }
    let offsets: &[(f32, f32, f32)] = match elevation {
        1 => &[(0.0, 1.0, 4.0), (0.0, 2.0, 8.0)],
        2 => &[(0.0, 2.0, 8.0), (0.0, 4.0, 16.0)],
        3 => &[(0.0, 4.0, 12.0), (0.0, 8.0, 24.0)],
        _ => return,
    };
    for (i, &(dx, dy, blur)) in offsets.iter().enumerate() {
        let alpha_mult = if i == 0 { 1.0 } else { 0.6 };
        let base = tokens.color_shadow;
        let shadow_color = Color::from_rgba(
            base.r,
            base.g,
            base.b,
            (base.a as f32 * alpha_mult).min(255.0) as u8,
        );
        // Draw a gradient-blur approximation: a filled rect slightly offset
        let shadow_rect = Rect::new(
            frame.x + dx,
            frame.y + dy,
            frame.w,
            frame.h,
        );
        // Use a filled rect with the shadow color; for a more realistic shadow
        // we'd use a gradient, but this is a lightweight approximation.
        ctx.fill_rect(shadow_rect, shadow_color, Some(Radius::uniform(blur)));
    }
}

impl Widget for Card {
    fn preferred_size(&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(
            self.fixed_width.unwrap_or(200.0),
            self.fixed_height.unwrap_or(0.0),
        )
    }

    fn build(&self) -> Vec<Box<dyn Widget>> {
        self.children.borrow_mut().take().unwrap_or_default()
    }

    fn on_event(&mut self, event: &WidgetEvent) -> EventResult {
        if self.hoverable {
            match event {
                WidgetEvent::HoverEnter => {
                    self.hovered = true;
                    EventResult::Handled
                }
                WidgetEvent::HoverLeave => {
                    self.hovered = false;
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            }
        } else {
            EventResult::NotHandled
        }
    }

    fn render(&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let tokens = DesignTokens::antd_light();
        let card_radius = Some(Radius::uniform(tokens.border_radius_lg));

        // ── Shadow (behind card body) ──
        draw_shadow(ctx, frame, self.elevation, &tokens);

        // ── Background ──
        let bg = if self.hovered {
            // Slightly lighter when hovered
            let base = tokens.color_bg_container;
            Color::from_rgb(
                (base.r as f32 * 0.95 + 255.0 * 0.05) as u8,
                (base.g as f32 * 0.95 + 255.0 * 0.05) as u8,
                (base.b as f32 * 0.95 + 255.0 * 0.05) as u8,
            )
        } else {
            tokens.color_bg_elevated
        };
        ctx.fill_rect(frame, bg, card_radius);

        // ── Top accent line (elevation indicator) ──
        if self.elevation > 1 {
            let accent_rect = Rect::new(
                frame.x + 24.0,
                frame.y,
                frame.w - 48.0,
                3.0,
            );
            ctx.fill_rect(accent_rect, tokens.color_primary, Some(Radius::uniform(1.5)));
        }

        // ── Border ──
        if self.bordered {
            ctx.stroke_rect(frame, tokens.color_border_secondary, 1.0, card_radius);
        }

        // ── Title region ──
        if let Some(ref title) = self.title {
            let title_h = 44.0;
            ctx.draw_text(
                title,
                Point::new(frame.x + self.padding, frame.y + 12.0),
                tokens.color_text,
                15.0,
            );

            // Subtle separator line under title
            let sep_y = frame.y + title_h + 4.0;
            let sep_rect = Rect::new(
                frame.x + self.padding,
                sep_y,
                frame.w - self.padding * 2.0,
                1.0,
            );
            ctx.fill_rect(sep_rect, tokens.color_border_secondary, None);
        }
    }

    fn layout_children(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
    ) -> Vec<(WidgetId, Rect)> {
        let mut result = Vec::new();
        if children.is_empty() {
            return result;
        }

        let title_offset = if self.title.is_some() { 56.0 } else { self.padding };
        let inner = Rect::new(
            frame.x + self.padding,
            frame.y + title_offset,
            (frame.w - self.padding * 2.0).max(0.0),
            (frame.h - title_offset - self.padding).max(0.0),
        );

        if inner.w <= 0.0 || inner.h <= 0.0 {
            return result;
        }

        // Stack children vertically
        let mut cursor_y = inner.y;
        for &cid in children {
            let pref = tree
                .get(cid)
                .map(|c| c.preferred_size(None))
                .unwrap_or_else(|| Size::zero());
            let ch = if pref.h > 0.0 {
                pref.h.min(inner.h)
            } else {
                inner.h / children.len() as f32
            };
            result.push((cid, Rect::new(inner.x, cursor_y, inner.w, ch)));
            cursor_y += ch;
        }
        // If children don't fill the space, let the last one expand
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

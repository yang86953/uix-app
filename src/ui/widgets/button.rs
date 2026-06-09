//! Button widget — Ant Design style button with variants, sizes, and states.

use crate::define_widget;
use crate::graphics::{Color, GraphicsEngine, Point, Rect, Size, Radius};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

/// Button style variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary, Default, Dashed, Text, Link,
}

/// Button size matching Ant Design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSize {
    Small, Middle, Large,
}

impl ButtonSize {
    pub fn height(&self) -> f32 {
        match self { Self::Small => 24.0, Self::Middle => 32.0, Self::Large => 40.0 }
    }
    pub fn font_size(&self) -> f32 {
        match self { Self::Small => 14.0, Self::Middle => 14.0, Self::Large => 16.0 }
    }
    pub fn padding_h(&self) -> f32 {
        match self { Self::Small => 7.0, Self::Middle => 15.0, Self::Large => 15.0 }
    }
}

define_widget! {
    /// Button widget.
    pub struct Button {
        text: String,
        variant: ButtonVariant,
        btn_size: ButtonSize,
        #[allow(dead_code)] block: bool,
        disabled: bool,
        #[allow(dead_code)] loading: bool,
        hovered: bool,
        pressed: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        let h = self.btn_size.height();
        let w = self.text.len() as f32 * 7.0 + self.btn_size.padding_h() * 2.0;
        Size::new(w, h)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        match event {
            WidgetEvent::MouseDown { .. } => { self.pressed = true; EventResult::Handled }
            WidgetEvent::MouseUp { .. } => { self.pressed = false; EventResult::Handled }
            WidgetEvent::HoverEnter => { self.hovered = true; EventResult::Handled }
            WidgetEvent::HoverLeave => { self.hovered = false; self.pressed = false; EventResult::Handled }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let h = self.btn_size.height();
        let font_size = self.btn_size.font_size();
        let btn_frame = Rect::new(frame.x, frame.y, frame.w, h.min(frame.h));

        let primary_border = ctx.tokens().color_primary_border();
        let primary_active = ctx.tokens().color_primary_active();
        let primary_hover = ctx.tokens().color_primary_hover();
        let primary = ctx.tokens().color_primary();
        let border = ctx.tokens().color_border();
        let text_quaternary = ctx.tokens().color_text_quaternary();
        let text = ctx.tokens().color_text();
        let border_radius = ctx.tokens().border_radius();

        let (bg, border, text_color, border_width) = if self.disabled {
            match self.variant {
                ButtonVariant::Primary => (Some(primary_border), border, text_quaternary, 1.0),
                ButtonVariant::Text | ButtonVariant::Link => (None, Color::transparent(), text_quaternary, 0.0),
                _ => (None, border, text_quaternary, 1.0),
            }
        } else if self.pressed {
            match self.variant {
                ButtonVariant::Primary => (Some(primary_active), primary_active, Color::white(), 1.0),
                ButtonVariant::Text | ButtonVariant::Link => (None, Color::transparent(), primary_active, 0.0),
                _ => (None, primary_active, primary_active, 1.0),
            }
        } else if self.hovered {
            match self.variant {
                ButtonVariant::Primary => (Some(primary_hover), primary_hover, Color::white(), 1.0),
                ButtonVariant::Text | ButtonVariant::Link => (None, Color::transparent(), primary_hover, 0.0),
                _ => (None, primary, primary, 1.0),
            }
        } else {
            match self.variant {
                ButtonVariant::Primary => (Some(primary), primary, Color::white(), 1.0),
                ButtonVariant::Dashed => (None, border, text, 1.0),
                ButtonVariant::Text | ButtonVariant::Link => (None, Color::transparent(), primary, 0.0),
                _ => (None, border, text, 1.0),
            }
        };

        if let Some(bg_color) = bg {
            let r = if border_radius > 0.0 { Some(Radius::uniform(border_radius)) } else { None };
            ctx.fill_rect(btn_frame, bg_color, r);
        }
        if border_width > 0.0 && border.a > 0 {
            ctx.stroke_rect(btn_frame, border, border_width, Some(Radius::uniform(border_radius)));
        }
        let text_x = btn_frame.x + self.btn_size.padding_h();
        let text_y = btn_frame.y + (btn_frame.h - font_size) * 0.5 - 4.0;
        ctx.draw_text(&self.text, Point::new(text_x, text_y), text_color, font_size);
    }
}

impl Button {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            variant: ButtonVariant::Default,
            btn_size: ButtonSize::Middle,
            block: false, disabled: false, loading: false,
            hovered: false, pressed: false,
        }
    }
    pub fn variant(mut self, v: ButtonVariant) -> Self { self.variant = v; self }
    pub fn size(mut self, s: ButtonSize) -> Self { self.btn_size = s; self }
    pub fn primary(mut self) -> Self { self.variant = ButtonVariant::Primary; self }
    pub fn dashed(mut self) -> Self { self.variant = ButtonVariant::Dashed; self }
    pub fn text(mut self) -> Self { self.variant = ButtonVariant::Text; self }
    pub fn link(mut self) -> Self { self.variant = ButtonVariant::Link; self }
    pub fn block(mut self) -> Self { self.block = true; self }
    pub fn loading(mut self) -> Self { self.loading = true; self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
}

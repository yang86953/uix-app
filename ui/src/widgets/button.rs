//! Button widget — Ant Design style button with variants, sizes, and states.

use uix_core::{Point, Rect, Size};
use crate::define_widget;
use uix_graphics::{Color, GraphicsEngine};
use crate::render_context::RenderContext;
use crate::style::Style;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};

/// Button style variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Default,
    Dashed,
    Text,
    Link,
}

/// Button size matching Ant Design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSize {
    Small,
    Middle,
    Large,
}

impl ButtonSize {
    pub fn height(&self) -> f32 {
        match self {
            Self::Small => 24.0,
            Self::Middle => 32.0,
            Self::Large => 40.0,
        }
    }
    pub fn font_size(&self) -> f32 {
        match self {
            Self::Small => 14.0,
            Self::Middle => 14.0,
            Self::Large => 16.0,
        }
    }
    pub fn padding_h(&self) -> f32 {
        match self {
            Self::Small => 7.0,
            Self::Middle => 15.0,
            Self::Large => 15.0,
        }
    }
}

define_widget! {
    /// Button widget.
    pub struct Button {
        text: String,
        variant: ButtonVariant,
        btn_size: ButtonSize,
        disabled: bool,
        hovered: bool,
        pressed: bool,
        anim_progress: f32,
        click_pos: Option<Point>,
        on_click: Option<Box<dyn FnMut() + 'static>>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        let h = self.btn_size.height();
        let w = self.text.len() as f32 * 7.0 + self.btn_size.padding_h() * 2.0;
        Size::new(w, h)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                self.pressed = true;
                self.click_pos = Some(*pos);
                self.anim_progress = 0.001; // start ripple
                EventResult::Handled
            }
            WidgetEvent::MouseUp { .. } => {
                self.pressed = false;
                if let Some(ref mut cb) = self.on_click {
                    cb();
                }
                EventResult::Handled
            }
            WidgetEvent::HoverEnter => { self.hovered = true; EventResult::Handled }
            WidgetEvent::HoverLeave => { self.hovered = false; self.pressed = false; EventResult::Handled }
            _ => EventResult::NotHandled,
        }
    }

    on_update => (&mut self, dt: f32) {
        if self.anim_progress > 0.0 {
            self.anim_progress += dt / 0.4; // 400ms wave duration
            if self.anim_progress >= 1.0 {
                self.anim_progress = 0.0;
            }
        }
    }

    needs_continuous_update => (&self) -> bool {
        self.anim_progress > 0.0
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let btn_frame = Rect::new(frame.x, frame.y, frame.w,
            self.btn_size.height().min(frame.h));
        let style = self.compute_style(ctx);

        ctx.apply_style(btn_frame, &style);
        let content_rect = btn_frame.inset(style.padding);
        ctx.text_center(&self.text, content_rect, style.color, style.font_size);
    }

    // ── Post-render: ripple overlay (separate from UI render) ──────────
    // Rendered as an overlay so it doesn't couple with the button's base
    // appearance. The dirty_rect() method ensures the ripple's bounding
    // box is accurately tracked for incremental re-rendering.
    post_render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if self.anim_progress <= 0.0 || self.anim_progress >= 1.0 {
            return;
        }
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let (r, g, b) = match self.variant {
            ButtonVariant::Primary => (primary_hover.r, primary_hover.g, primary_hover.b),
            _ => (primary.r, primary.g, primary.b),
        };
        let (cx, cy) = self.click_pos
            .map(|p| (frame.x + p.x, frame.y + p.y))
            .unwrap_or((frame.x + frame.w / 2.0, frame.y + frame.h / 2.0));
        let max_r = (frame.w.max(frame.h)) * 0.7;
        let r_radius = max_r * self.anim_progress;
        let alpha = (60.0 * (1.0 - self.anim_progress)).max(0.0) as u8;
        if alpha > 0 {
            ctx.fill_circle(cx, cy, r_radius, Color::from_rgba(r, g, b, alpha));
        }
    }

    // ── Dirty rect: exact ripple bounding box (may overflow button) ────
    // The ripple circle may extend beyond the button frame. Returning the
    // exact bounding box ensures the incremental renderer covers the full
    // ripple area without overpainting the entire button.
    dirty_rect => (&self, frame: Rect) -> Rect {
        if self.anim_progress <= 0.0 || self.anim_progress >= 1.0 {
            return frame;
        }
        let (cx, cy) = self.click_pos
            .map(|p| (frame.x + p.x, frame.y + p.y))
            .unwrap_or((frame.x + frame.w / 2.0, frame.y + frame.h / 2.0));
        let max_r = (frame.w.max(frame.h)) * 1.2;
        let r = max_r * self.anim_progress;
        Rect::new(cx - r, cy - r, r * 2.0, r * 2.0)
    }
}

impl Button {
    /// 根据当前状态（disabled/pressed/hovered/normal × variant）计算 Style。
    fn compute_style(&self, ctx: &RenderContext) -> Style {
        let t = ctx.tokens();
        let font_size = self.btn_size.font_size();

        let (bg, border, text_color, bw) = if self.disabled {
            match self.variant {
                ButtonVariant::Primary => (
                    Some(t.color_primary_border()),
                    t.color_border(),
                    t.color_text_quaternary(),
                    1.0,
                ),
                ButtonVariant::Text | ButtonVariant::Link => {
                    (None, Color::transparent(), t.color_text_quaternary(), 0.0)
                }
                _ => (None, t.color_border(), t.color_text_quaternary(), 1.0),
            }
        } else if self.pressed {
            match self.variant {
                ButtonVariant::Primary => (
                    Some(t.color_primary_active()),
                    t.color_primary_active(),
                    Color::white(),
                    1.0,
                ),
                ButtonVariant::Text | ButtonVariant::Link => {
                    (None, Color::transparent(), t.color_primary_active(), 0.0)
                }
                _ => (
                    None,
                    t.color_primary_active(),
                    t.color_primary_active(),
                    1.0,
                ),
            }
        } else if self.hovered {
            match self.variant {
                ButtonVariant::Primary => (
                    Some(t.color_primary_hover()),
                    t.color_primary_hover(),
                    Color::white(),
                    1.0,
                ),
                ButtonVariant::Text | ButtonVariant::Link => {
                    (None, Color::transparent(), t.color_primary_hover(), 0.0)
                }
                _ => (None, t.color_primary(), t.color_primary(), 1.0),
            }
        } else {
            match self.variant {
                ButtonVariant::Primary => (
                    Some(t.color_primary()),
                    t.color_primary(),
                    Color::white(),
                    1.0,
                ),
                ButtonVariant::Dashed => (None, t.color_border(), t.color_text(), 1.0),
                ButtonVariant::Text | ButtonVariant::Link => {
                    (None, Color::transparent(), t.color_primary(), 0.0)
                }
                _ => (None, t.color_border(), t.color_text(), 1.0),
            }
        };

        Style {
            background: bg,
            border_color: if bw > 0.0 { Some(border) } else { None },
            border_width: bw,
            border_radius: t.border_radius(),
            padding: uix_core::EdgeInsets::new(
                self.btn_size.padding_h(),
                0.0,
                self.btn_size.padding_h(),
                0.0,
            ),
            color: text_color,
            font_size,
        }
    }

    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            variant: ButtonVariant::Default,
            btn_size: ButtonSize::Middle,
            disabled: false,
            hovered: false,
            pressed: false,
            anim_progress: 0.0,
            click_pos: None,
            on_click: None,
        }
    }
    pub fn variant(mut self, v: ButtonVariant) -> Self {
        self.variant = v;
        self
    }
    pub fn size(mut self, s: ButtonSize) -> Self {
        self.btn_size = s;
        self
    }
    pub fn primary(mut self) -> Self {
        self.variant = ButtonVariant::Primary;
        self
    }
    pub fn dashed(mut self) -> Self {
        self.variant = ButtonVariant::Dashed;
        self
    }
    pub fn text(mut self) -> Self {
        self.variant = ButtonVariant::Text;
        self
    }
    pub fn link(mut self) -> Self {
        self.variant = ButtonVariant::Link;
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn on_click<F: FnMut() + 'static>(mut self, f: F) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

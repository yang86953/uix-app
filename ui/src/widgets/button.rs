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
/// （已统一为 uix_core::ControlSize，保留别名以兼容旧代码。）
pub use uix_core::ControlSize as ButtonSize;

/// Button 尺寸对应的高度。
pub fn button_height(size: ButtonSize) -> f32 {
    match size {
        ButtonSize::Small => 24.0,
        ButtonSize::Medium => 32.0,
        ButtonSize::Large => 40.0,
    }
}

/// Button 尺寸对应的字号。
pub fn button_font_size(size: ButtonSize) -> f32 {
    match size {
        ButtonSize::Small => 14.0,
        ButtonSize::Medium => 14.0,
        ButtonSize::Large => 16.0,
    }
}

/// Button 尺寸对应的水平内边距。
pub fn button_padding_h(size: ButtonSize) -> f32 {
    match size {
        ButtonSize::Small => 7.0,
        ButtonSize::Medium => 15.0,
        ButtonSize::Large => 15.0,
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
        let h = button_height(self.btn_size);
        let w = self.text.len() as f32 * 7.0 + button_padding_h(self.btn_size) * 2.0;
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
            button_height(self.btn_size).min(frame.h));
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
        let style = self.compute_style(ctx);
        // 波纹颜色按变体区分，同时保证在按钮底色和页面底色上都可见。
        let page_bg = ctx.tokens().color_bg_container();
        let base = match self.variant {
            // Primary：波纹用品牌色混黑，在蓝底上呈"按凹"效果，在白底上呈深蓝点
            ButtonVariant::Primary => ctx.tokens().color_primary().mix(&Color::black(), 0.3),
            // Text/Link：品牌色，比 Default 更 subtle
            ButtonVariant::Text | ButtonVariant::Link => ctx.tokens().color_primary(),
            // Default/Dashed：品牌色
            _ => ctx.tokens().color_primary(),
        };
        // 对比度保底：如果波纹色与页面底色太接近（亮度差 < 60），
        // 根据页面底色反转为黑/白，确保溢出按钮的部分始终可见。
        let (r, g, b) = if (base.luminance() as i16 - page_bg.luminance() as i16).abs() < 60 {
            if page_bg.is_light() { (0u8, 0u8, 0u8) } else { (255u8, 255u8, 255u8) }
        } else {
            (base.r, base.g, base.b)
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

    // ── Dirty rect: 波纹可能超出按钮边框，返回完整波纹区域 ────
    dirty_rect => (&self, frame: Rect) -> Rect {
        let btn_h = button_height(self.btn_size).min(frame.h);
        if self.anim_progress <= 0.0 || self.anim_progress >= 1.0 {
            return Rect::new(frame.x, frame.y, frame.w, btn_h);
        }
        let (cx, cy) = self.click_pos
            .map(|p| (frame.x + p.x, frame.y + p.y))
            .unwrap_or((frame.x + frame.w / 2.0, frame.y + frame.h / 2.0));
        let max_r = (frame.w.max(btn_h)) * 0.7;
        let r = max_r * self.anim_progress;
        let ripple_rect = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
        // 合并按钮区域 + 波纹区域，覆盖完整变化范围
        let btn_frame = Rect::new(frame.x, frame.y, frame.w, btn_h);
        let left = btn_frame.x.min(ripple_rect.x);
        let top = btn_frame.y.min(ripple_rect.y);
        let right = (btn_frame.x + btn_frame.w).max(ripple_rect.x + ripple_rect.w);
        let bottom = (btn_frame.y + btn_frame.h).max(ripple_rect.y + ripple_rect.h);
        Rect::new(left, top, right - left, bottom - top)
    }
}

impl Button {
    /// 根据当前状态（disabled/pressed/hovered/normal × variant）计算 Style。
    fn compute_style(&self, ctx: &RenderContext) -> Style {
        let t = ctx.tokens();
        let font_size = button_font_size(self.btn_size);

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
                button_padding_h(self.btn_size),
                0.0,
                button_padding_h(self.btn_size),
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
            btn_size: ButtonSize::Medium,
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

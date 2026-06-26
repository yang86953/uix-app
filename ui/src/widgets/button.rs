use uix_core::{Point, Rect, Size};
use crate::define_widget;
use crate::style::Style;
use uix_graphics::{Color, GraphicsEngine};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};

/// Button style variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary, Default, Dashed, Text, Link,
}

pub use uix_core::ControlSize as ButtonSize;

pub fn button_height(size: ButtonSize) -> f32 {
    match size { ButtonSize::Small => 24.0, ButtonSize::Medium => 32.0, ButtonSize::Large => 40.0 }
}

pub fn button_font_size(size: ButtonSize) -> f32 {
    match size { ButtonSize::Small => 14.0, ButtonSize::Medium => 14.0, ButtonSize::Large => 16.0 }
}

pub fn button_padding_h(size: ButtonSize) -> f32 {
    match size { ButtonSize::Small => 7.0, ButtonSize::Medium => 15.0, ButtonSize::Large => 15.0 }
}

define_widget! {
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
        loading: bool,
        icon: String,
        danger: bool,
        block: bool,
        ghost: bool,
        /// 用户自定义样式（完全覆盖 compute_style 的计算结果）。
        custom_style: Option<Style>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        let h = button_height(self.btn_size);
        let icon_w = if self.icon.is_empty() { 0.0 } else { 20.0 };
        let loading_w = if self.loading { 16.0 } else { 0.0 };
        let text_w = if self.text.is_empty() { 0.0 } else { self.text.len() as f32 * 7.0 };
        let extra = icon_w + loading_w + if !self.icon.is_empty() && !self.text.is_empty() { 4.0 } else { 0.0 };
        let w = text_w + button_padding_h(self.btn_size) * 2.0 + extra;
        if self.block { Size::new(f32::MAX, h) } else { Size::new(w.max(32.0), h) }
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if self.disabled || self.loading { return EventResult::NotHandled; }
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                self.pressed = true; self.click_pos = Some(*pos); self.anim_progress = 0.001;
                EventResult::Handled
            }
            WidgetEvent::MouseUp { .. } => {
                self.pressed = false;
                if let Some(ref mut cb) = self.on_click { cb(); }
                EventResult::Handled
            }
            WidgetEvent::HoverEnter => { self.hovered = true; EventResult::Handled }
            WidgetEvent::HoverLeave => { self.hovered = false; self.pressed = false; EventResult::Handled }
            _ => EventResult::NotHandled,
        }
    }

    on_update => (&mut self, dt: f32) {
        if self.anim_progress > 0.0 {
            self.anim_progress += dt / 0.4;
            if self.anim_progress >= 1.0 { self.anim_progress = 0.0; }
        }
    }

    needs_continuous_update => (&self) -> bool { self.anim_progress > 0.0 }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let btn_h = button_height(self.btn_size).min(frame.h);
        let btn_frame = Rect::new(frame.x, frame.y, if self.block { frame.w } else { frame.w }, btn_h);
        let style = self.compute_style(ctx);

        ctx.apply_style(btn_frame, &style);
        let content_rect = btn_frame.inset(style.padding);

        let mut cursor = content_rect.x;

        // loading 旋转图标
        if self.loading {
            let spin_chars = ["◐", "◓", "◑", "◒"];
            let phase = ((std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as f32 / 200.0) as usize) % 4;
            let ly = ctx.visual_center_y(btn_frame, 12.0);
            ctx.draw_text(spin_chars[phase], Point::new(cursor, ly), style.color, 12.0);
            cursor += 18.0;
        }

        // icon
        if !self.icon.is_empty() {
            let icon_str = crate::widgets::icon::icon_char(&self.icon);
            let saved = *ctx.font();
            if let Some(fh) = crate::widgets::icon::lucide_handle() { ctx.set_font(fh); }
            let iy = ctx.visual_center_y(btn_frame, 14.0);
            ctx.draw_text(icon_str, Point::new(cursor, iy), style.color, 14.0);
            ctx.set_font(saved);
            cursor += 20.0;
        }

        // text
        if !self.text.is_empty() {
            let tw = if self.icon.is_empty() { content_rect.w } else { content_rect.w - (cursor - content_rect.x) };
            let tx = if self.icon.is_empty() { content_rect.x } else { cursor };
            let text_center = content_rect.w / 2.0 - self.text.len() as f32 * 3.5;
            if self.icon.is_empty() && !self.loading {
                ctx.text_center(&self.text, content_rect, style.color, style.font_size);
            } else {
                let ty = ctx.visual_center_y(btn_frame, style.font_size);
                ctx.draw_text(&self.text, Point::new(tx, ty), style.color, style.font_size);
            }
        }
    }

    post_render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if self.anim_progress <= 0.0 || self.anim_progress >= 1.0 { return; }
        let style = self.compute_style(ctx);
        let page_bg = ctx.tokens().color_bg_container();
        let base = match self.variant {
            ButtonVariant::Primary => ctx.tokens().color_primary().mix(&Color::black(), 0.3),
            ButtonVariant::Text | ButtonVariant::Link => ctx.tokens().color_primary(),
            _ => ctx.tokens().color_primary(),
        };
        let (r, g, b) = if (base.luminance() as i16 - page_bg.luminance() as i16).abs() < 60 {
            if page_bg.is_light() { (0u8, 0u8, 0u8) } else { (255u8, 255u8, 255u8) }
        } else { (base.r, base.g, base.b) };
        let (cx, cy) = self.click_pos
            .map(|p| (frame.x + p.x, frame.y + p.y))
            .unwrap_or((frame.x + frame.w / 2.0, frame.y + frame.h / 2.0));
        let max_r = (frame.w.max(frame.h)) * 0.7;
        let r_radius = max_r * self.anim_progress;
        let alpha = (60.0 * (1.0 - self.anim_progress)).max(0.0) as u8;
        if alpha > 0 { ctx.fill_circle(cx, cy, r_radius, Color::from_rgba(r, g, b, alpha)); }
    }

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
        let btn_frame = Rect::new(frame.x, frame.y, frame.w, btn_h);
        let left = btn_frame.x.min(ripple_rect.x);
        let top = btn_frame.y.min(ripple_rect.y);
        let right = (btn_frame.x + btn_frame.w).max(ripple_rect.x + ripple_rect.w);
        let bottom = (btn_frame.y + btn_frame.h).max(ripple_rect.y + ripple_rect.h);
        Rect::new(left, top, right - left, bottom - top)
    }
}

impl Button {
    fn compute_style(&self, ctx: &RenderContext) -> Style {
        // 用户自定义样式优先——完全覆盖计算结果，由用户全权控制
        if let Some(ref cs) = self.custom_style {
            return cs.clone();
        }

        let t = ctx.tokens();
        let font_size = button_font_size(self.btn_size);



        let (bg, border, text_color, bw) = if self.disabled {
            match self.variant {
                ButtonVariant::Primary => (Some(t.color_primary_border()), t.color_border(), t.color_text_quaternary(), 1.0),
                ButtonVariant::Text | ButtonVariant::Link => (None, Color::transparent(), t.color_text_quaternary(), 0.0),
                _ => (None, t.color_border(), t.color_text_quaternary(), 1.0),
            }
        } else if self.pressed {
            let c = if self.danger { t.color_error() } else { t.color_primary() };
            match self.variant {
                ButtonVariant::Primary => (Some(c), c, Color::white(), 1.0),
                ButtonVariant::Text | ButtonVariant::Link => (None, Color::transparent(), c, 0.0),
                _ => (None, c, c, 1.0),
            }
        } else if self.hovered {
            let c = if self.danger { t.color_error() } else { t.color_primary_hover() };
            match self.variant {
                ButtonVariant::Primary => (Some(c), c, Color::white(), 1.0),
                ButtonVariant::Text | ButtonVariant::Link => (None, Color::transparent(), c, 0.0),
                _ => (None, if self.danger { t.color_error() } else { t.color_primary() }, c, 1.0),
            }
        } else {
                let normal_border = if self.danger { t.color_error() } else { t.color_primary() };
                let normal_text = if self.danger { t.color_error() } else { t.color_primary() };
            match self.variant {
                ButtonVariant::Primary => {
                    let bg_color = if self.danger { t.color_error() } else { t.color_primary() };
                    (Some(bg_color), bg_color, Color::white(), 1.0)
                }
                ButtonVariant::Dashed => (None, t.color_border(), t.color_text(), 1.0),
                ButtonVariant::Text | ButtonVariant::Link => (None, Color::transparent(), normal_text, 0.0),
                _ => {
                    if self.ghost { (None, if self.danger { t.color_error() } else { t.color_primary() }, if self.danger { t.color_error() } else { t.color_primary() }, 1.0) }
                    else { (None, t.color_border(), t.color_text(), 1.0) }
                }
            }
        };

        Style {
            background: bg,
            border_color: if bw > 0.0 { Some(border) } else { None },
            border_width: bw,
            border_radius: t.border_radius(),
            padding: uix_core::EdgeInsets::new(button_padding_h(self.btn_size), 0.0, button_padding_h(self.btn_size), 0.0),
            color: text_color,
            font_size,
            ..Style::default()
        }
    }

    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(), variant: ButtonVariant::Default, btn_size: ButtonSize::Medium,
            disabled: false, hovered: false, pressed: false, anim_progress: 0.0, click_pos: None,
            on_click: None, loading: false, icon: String::new(), danger: false, block: false, ghost: false,
            custom_style: None,
        }
    }

    /// 设置用户自定义样式（完全覆盖内置的 variant/state 计算）。
    /// 设置后，primary()/danger()/size() 等视觉方法不再生效。
    pub fn style(mut self, s: Style) -> Self {
        self.custom_style = Some(s);
        self
    }
    pub fn variant(mut self, v: ButtonVariant) -> Self { self.variant = v; self }
    pub fn size(mut self, s: ButtonSize) -> Self { self.btn_size = s; self }
    pub fn primary(mut self) -> Self { self.variant = ButtonVariant::Primary; self }
    pub fn dashed(mut self) -> Self { self.variant = ButtonVariant::Dashed; self }
    pub fn text(mut self) -> Self { self.variant = ButtonVariant::Text; self }
    pub fn link(mut self) -> Self { self.variant = ButtonVariant::Link; self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn on_click<F: FnMut() + 'static>(mut self, f: F) -> Self { self.on_click = Some(Box::new(f)); self }
    pub fn loading(mut self, v: bool) -> Self { self.loading = v; self }
    pub fn icon(mut self, i: &str) -> Self { self.icon = i.to_string(); self }
    pub fn danger(mut self, v: bool) -> Self { self.danger = v; self }
    pub fn block(mut self, v: bool) -> Self { self.block = v; self }
    pub fn ghost(mut self, v: bool) -> Self { self.ghost = v; self }
}

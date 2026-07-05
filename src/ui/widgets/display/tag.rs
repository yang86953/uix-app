//! Tag widget — 彩色标签/徽标，支持关闭按钮。

use crate::define_widget;
use crate::draw::painting::RenderContext;
use crate::ui::WidgetTree;
use crate::draw::Radius;
use crate::native::{Rect, Size};

/// 预设标签类型。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TagColor {
    Default,
    Success,
    Info,
    Warning,
    Error,
}

define_widget! {
    pub struct Tag {
        text: String,
        color: TagColor,
        closable: bool,
        font_size: f32,
        custom_color: Option<Color>,
        checkable: bool,
        on_close: Option<Box<dyn FnMut() + 'static>>,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        let w = self.text.len() as f32 * (self.font_size * 0.6) + 16.0
            + if self.closable { 20.0 } else { 0.0 };
        Size::new(w, self.font_size + 8.0)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let (bg, fg) = if let Some(cc) = self.custom_color {
            (cc, Color::white())
        } else {
            match self.color {
                TagColor::Default => (ctx.tokens().color_fill_tertiary(), ctx.tokens().color_text()),
                TagColor::Success => (ctx.tokens().color_success_bg(), ctx.tokens().color_success()),
                TagColor::Info    => (ctx.tokens().color_info_bg(), ctx.tokens().color_info()),
                TagColor::Warning => (ctx.tokens().color_warning_bg(), ctx.tokens().color_warning()),
                TagColor::Error   => (ctx.tokens().color_error_bg(), ctx.tokens().color_error()),
            }
        };
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        ctx.fill_rect(frame, bg, r);
        let text_x = frame.x + 8.0;
        let text_w = frame.w - 16.0 - if self.closable { 20.0 } else { 0.0 };
        let text_frame = Rect::new(text_x, frame.y, text_w, frame.h);
        ctx.text_center(&self.text, text_frame, fg, self.font_size);
        if self.closable {
            let cx = frame.x + frame.w - 14.0;
            let cy = ctx.visual_center_y(frame, 10.0);
            ctx.draw_text("✕", crate::native::Point::new(cx, cy), fg, 10.0);
        }
    }
}

impl Default for Tag {
    fn default() -> Self {
        Self::new("")
    }
}

impl Tag {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: TagColor::Default,
            closable: false,
            font_size: 12.0,
            custom_color: None,
            checkable: false,
            on_close: None,
        }
    }
    pub fn color(mut self, c: TagColor) -> Self {
        self.color = c;
        self
    }
    pub fn custom_color(mut self, c: Color) -> Self {
        self.custom_color = Some(c);
        self
    }
    pub fn closable(mut self) -> Self {
        self.closable = true;
        self
    }
    pub fn checkable(mut self, v: bool) -> Self {
        self.checkable = v;
        self
    }
    pub fn on_close<F: FnMut() + 'static>(mut self, f: F) -> Self {
        self.on_close = Some(Box::new(f));
        self
    }
    pub fn font_size(mut self, s: f32) -> Self {
        self.font_size = s;
        self
    }
}

use crate::draw::Color;

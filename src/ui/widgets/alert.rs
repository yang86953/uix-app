//! Alert widget — 警示条，支持类型、图标、关闭。

use crate::define_widget;
use crate::base::{Rect, Size};
use crate::graphics::Radius;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::WidgetTree;

/// 警示类型。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlertType { Success, Info, Warning, Error }

define_widget! {
    /// Alert — 带类型颜色的警示条。
    pub struct Alert {
        message: String,
        description: String,
        type_: AlertType,
        closable: bool,
        _show_icon: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        let h = 36.0 + if self.description.is_empty() { 0.0 } else { 18.0 };
        Size::new(300.0, h)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let (bg, border, fg) = match self.type_ {
            AlertType::Success => (ctx.tokens().color_success_bg(), ctx.tokens().color_success(), ctx.tokens().color_success()),
            AlertType::Info    => (ctx.tokens().color_info_bg(), ctx.tokens().color_info(), ctx.tokens().color_info()),
            AlertType::Warning => (ctx.tokens().color_warning_bg(), ctx.tokens().color_warning(), ctx.tokens().color_warning()),
            AlertType::Error   => (ctx.tokens().color_error_bg(), ctx.tokens().color_error(), ctx.tokens().color_error()),
        };
        let r = Some(Radius::uniform(ctx.tokens().border_radius()));
        ctx.fill_rect(frame, bg, r);
        // 左边框强调线
        ctx.fill_rect(Rect::new(frame.x + 2.0, frame.y + 4.0, 3.0, frame.h - 8.0), border, Some(Radius::uniform(1.5)));

        let text_x = frame.x + 14.0;
        ctx.draw_text(&self.message, crate::base::Point::new(text_x, frame.y + 8.0), fg, 14.0);
        if !self.description.is_empty() {
            ctx.draw_text(&self.description, crate::base::Point::new(text_x, frame.y + 26.0), ctx.tokens().color_text_secondary(), 12.0);
        }
        if self.closable {
            let cx = frame.x + frame.w - 18.0;
            let cy = frame.y + frame.h * 0.5 - 5.0;
            ctx.draw_text("✕", crate::base::Point::new(cx, cy), ctx.tokens().color_text_quaternary(), 14.0);
        }
    }
}

impl Default for Alert { fn default() -> Self { Self::new("") } }

impl Alert {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            description: String::new(),
            type_: AlertType::Info,
            closable: false,
            _show_icon: true,
        }
    }
    pub fn description(mut self, d: impl Into<String>) -> Self { self.description = d.into(); self }
    pub fn type_(mut self, t: AlertType) -> Self { self.type_ = t; self }
    pub fn closable(mut self) -> Self { self.closable = true; self }
}

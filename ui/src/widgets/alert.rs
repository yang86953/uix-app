//! Alert widget — 警示条，支持类型、图标、关闭。

use crate::define_widget;
use uix_core::{Rect, Size};
use uix_graphics::Radius;
use crate::render_context::RenderContext;
use crate::widget::WidgetTree;

/// 警示类型。
/// （已统一为 uix_core::StatusLevel，保留别名以兼容旧代码。）
pub use uix_core::StatusLevel as AlertType;

define_widget! {
    /// Alert — 带类型颜色的警示条。
    pub struct Alert {
        message: String,
        description: String,
        type_: AlertType,
        closable: bool,
        _show_icon: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
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
        let msg_y = ctx.visual_center_y(frame, 14.0);
        ctx.draw_text(&self.message, uix_core::Point::new(text_x, msg_y), fg, 14.0);
        if !self.description.is_empty() {
            let desc_rect = Rect::new(frame.x, frame.y + frame.h * 0.5, frame.w, frame.h * 0.5);
            let desc_y = ctx.visual_center_y(desc_rect, 12.0);
            ctx.draw_text(&self.description, uix_core::Point::new(text_x, desc_y), ctx.tokens().color_text_secondary(), 12.0);
        }
        if self.closable {
            let cx = frame.x + frame.w - 18.0;
            let cy = ctx.visual_center_y(frame, 14.0);
            ctx.draw_text("✕", uix_core::Point::new(cx, cy), ctx.tokens().color_text_quaternary(), 14.0);
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

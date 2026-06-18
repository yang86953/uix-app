//! Result widget — 结果页，Ant Design 风格。
//!
//! 用于展示操作结果（成功/错误/警告/信息/404/403/500），
//! 包含图标、标题、副标题、额外操作区域。

use crate::base::{Point, Rect, Size};
use crate::define_widget;
use crate::graphics::Color;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

/// 结果类型。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResultType {
    Success,
    Error,
    Info,
    Warning,
    NotFound,   // 404
    Forbidden,  // 403
    ServerError,// 500
}

/// Result — 结果页组件。
define_widget! {
    pub struct Result {
        type_: ResultType,
        title: String,
        subtitle: String,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(400.0, 300.0)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let (icon, icon_color, main_title) = match self.type_ {
            ResultType::Success => ("✓", ctx.tokens().color_success(), if self.title.is_empty() { "操作成功" } else { &self.title }),
            ResultType::Error   => ("✗", ctx.tokens().color_error(), if self.title.is_empty() { "操作失败" } else { &self.title }),
            ResultType::Info    => ("ℹ", ctx.tokens().color_info(), if self.title.is_empty() { "提示信息" } else { &self.title }),
            ResultType::Warning => ("⚠", ctx.tokens().color_warning(), if self.title.is_empty() { "警告" } else { &self.title }),
            ResultType::NotFound => ("404", ctx.tokens().color_text_quaternary(), if self.title.is_empty() { "页面不存在" } else { &self.title }),
            ResultType::Forbidden => ("403", ctx.tokens().color_warning(), if self.title.is_empty() { "无权限访问" } else { &self.title }),
            ResultType::ServerError => ("500", ctx.tokens().color_error(), if self.title.is_empty() { "服务器错误" } else { &self.title }),
        };
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.4;

        // 大图标
        match self.type_ {
            ResultType::NotFound | ResultType::Forbidden | ResultType::ServerError => {
                ctx.draw_text(icon, Point::new(cx - 28.0, cy - 50.0), icon_color, 48.0);
            }
            _ => {
                ctx.fill_circle(cx, cy - 30.0, 32.0, icon_color);
                ctx.draw_text(icon, Point::new(cx - 10.0, cy - 42.0), Color::white(), 24.0);
            }
        }

        // 标题
        ctx.draw_text(main_title, Point::new(cx - main_title.len() as f32 * 5.0, cy + 10.0), text, 20.0);

        // 副标题
        let sub = if self.subtitle.is_empty() {
            match self.type_ {
                ResultType::NotFound => "请检查您访问的地址是否正确",
                ResultType::Forbidden => "请联系管理员获取权限",
                ResultType::ServerError => "请稍后重试",
                _ => "",
            }
        } else { &self.subtitle };
        if !sub.is_empty() {
            ctx.draw_text(sub, Point::new(cx - sub.len() as f32 * 3.5, cy + 40.0), text_sec, 13.0);
        }
    }
}

impl Result {
    pub fn new(type_: ResultType) -> Self {
        Self { type_, title: String::new(), subtitle: String::new() }
    }
    pub fn title(mut self, t: &str) -> Self { self.title = t.to_string(); self }
    pub fn subtitle(mut self, s: &str) -> Self { self.subtitle = s.to_string(); self }
}

impl Default for Result {
    fn default() -> Self { Self::new(ResultType::Info) }
}

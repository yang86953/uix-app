//! Empty widget — 空状态占位（图标 + 描述居中）。

use crate::define_widget;
use crate::render_context::RenderContext;
use crate::widget::WidgetTree;
use uix_platform::{Rect, Size};

define_widget! {
    /// Empty — 空状态展示。
    pub struct Empty {
        description: String,
        icon_name: String,
        image: String,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::traits::GraphicsEngine>) -> Size {
        Size::new(160.0, 100.0)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let loc = crate::locale::use_locale();
        let desc = if self.description.is_empty() { loc.empty_description } else { &self.description };
        let text_secondary = ctx.tokens().color_text_quaternary();
        let text_tertiary = ctx.tokens().color_text_tertiary();

        // image preset
        if !self.image.is_empty() {
            let icon_str = match self.image.as_str() {
                "default" => "📦",
                "search" => "🔍",
                "file" => "📄",
                "folder" => "📁",
                "network" => "🌐",
                _ => "📦",
            };
            let icon_frame = Rect::new(frame.x, frame.y + 4.0, frame.w, frame.h * 0.4);
            ctx.text_center(icon_str, icon_frame, text_tertiary, 36.0);
        }

        // 图标（25% 高度位置）
        if !self.icon_name.is_empty() && self.image.is_empty() {
            let icon_str = crate::widgets::icon::icon_char(&self.icon_name);
            let saved = *ctx.font();
            if let Some(fh) = crate::widgets::icon::lucide_handle() {
                ctx.set_font(fh);
            }
            let icon_frame = Rect::new(frame.x, frame.y + 8.0, frame.w, frame.h * 0.35);
            ctx.text_center(icon_str, icon_frame, text_secondary, 28.0);
            ctx.set_font(saved);
        }
        // 描述文字（图标下方居中）
        let desc_frame = Rect::new(frame.x, frame.y + frame.h * 0.45, frame.w, frame.h * 0.5);
        ctx.text_center(desc, desc_frame, text_secondary, 13.0);
    }
}

impl Default for Empty {
    fn default() -> Self {
        Self::new()
    }
}

impl Empty {
    pub fn new() -> Self {
        Self {
            description: String::new(),
            icon_name: String::new(),
            image: String::new(),
        }
    }
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = d.into();
        self
    }
    pub fn icon(mut self, name: impl Into<String>) -> Self {
        self.icon_name = name.into();
        self
    }
    pub fn image(mut self, name: impl Into<String>) -> Self {
        self.image = name.into();
        self
    }
}

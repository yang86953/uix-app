//! ThemeToggle — 主题切换按钮（暗色 ↔ 亮色）。
//!
//! 点击切换暗色/亮色主题，通过 Cell<bool> 通知外部代码。

use crate::define_widget;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};
use std::cell::Cell;
use uix_platform::{Rect, Size};

define_widget! {
    /// ThemeToggle — 主题切换按钮。
    pub struct ThemeToggle {
        pub dark: Cell<bool>,
    }

    preferred_size => (&self, _eng: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(32.0, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { .. } = event {
            let new = !self.dark.get();
            self.dark.set(new);
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let icon = if self.dark.get() { "sun" } else { "moon" };
        let icon_str = crate::widgets::icon::icon_char(icon);
        let text_color = ctx.tokens().color_text();
        let saved = *ctx.font();
        if let Some(fh) = crate::widgets::icon::lucide_handle() {
            ctx.set_font(fh);
        }
        ctx.text_center(icon_str, frame, text_color, 18.0);
        ctx.set_font(saved);
    }
}

impl ThemeToggle {
    pub fn new() -> Self {
        Self {
            dark: Cell::new(false),
        }
    }

    pub fn dark(self, dark: bool) -> Self {
        self.dark.set(dark);
        self
    }

    pub fn is_dark(&self) -> bool {
        self.dark.get()
    }
}

impl Default for ThemeToggle {
    fn default() -> Self {
        Self::new()
    }
}

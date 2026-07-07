//! ThemeToggle — 主题切换按钮（暗色 ↔ 亮色）。
//!
//! 点击切换暗色/亮色主题，通过 Cell<bool> 通知外部代码。

use crate::core::{Constraints, Rect, Size};
use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::ui::{EventResult, SystemEvent, WidgetTree};
use std::cell::Cell;

define_widget! {
    /// ThemeToggle — 主题切换按钮。
    pub struct ThemeToggle {
        pub dark: Cell<bool>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    preferred_size => (&self, _eng: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        self.intrinsic_size()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if let SystemEvent::PointerDown { .. } = event {
            let new = !self.dark.get();
            self.dark.set(new);
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let icon = if self.dark.get() { "sun" } else { "moon" };
        let icon_str = crate::ui::widgets::icon::icon_char(icon);
        let text_color = ctx.tokens().color_text();
        let saved = *ctx.font();
        if let Some(fh) = crate::ui::widgets::icon::lucide_handle() {
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

    fn intrinsic_size(&self) -> Size {
        Size::new(32.0, 32.0)
    }
}

impl Default for ThemeToggle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_clamps_theme_toggle_size() {
        let measured = ThemeToggle::new().measure(Constraints::loose(Size::new(20.0, 20.0)));

        assert_eq!(measured, Size::new(20.0, 20.0));
    }
}

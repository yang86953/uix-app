//! ThemeToggle — 主题切换按钮（暗色 ↔ 亮色）。
//!
//! 点击切换暗色/亮色主题，通过 Cell<bool> 通知外部代码。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::{EventResult, SnapshotFields, SystemEvent, WidgetTree};
use std::cell::Cell;

component! {
    /// ThemeToggle — 主题切换按钮。
    pub struct ThemeToggle {
        #[snapshot(skip)]
        pub dark: Cell<bool>,
        initial_dark: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
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
            initial_dark: false,
        }
    }

    pub fn dark(mut self, dark: bool) -> Self {
        self.dark.set(dark);
        self.initial_dark = dark;
        self
    }

    pub fn is_dark(&self) -> bool {
        self.dark.get()
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::ThemeToggle {
            dark: self.initial_dark,
        }
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

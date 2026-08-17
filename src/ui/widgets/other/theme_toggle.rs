//! ThemeToggle — 主题切换按钮（暗色 ↔ 亮色）。
//!
//! 点击切换暗色/亮色主题，通过 `Cell<bool>` 通知外部代码。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};
use std::cell::Cell;

component! {
    /// ThemeToggle — 主题切换按钮。
    pub struct ThemeToggle {
        #[snapshot(skip)]
        /// 当前是否选择暗色主题的内部可变状态。
        pub dark: Cell<bool>,
        initial_dark: bool,
        focused: bool,
        pending_change: Cell<Option<bool>>,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                button: MouseButton::Left,
                ..
            }
            | SystemEvent::KeyDown {
                key: KeyCode::Enter | KeyCode::Space,
                ..
            } => {
                self.toggle();
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change.take().map(|dark| {
            SemanticEvent::change(id, if dark { "dark" } else { "light" })
        })
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let icon = if self.dark.get() { "sun" } else { "moon" };
        let text_color = ctx.tokens().color_text();
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            icon,
            frame,
            text_color,
            18.0,
        );
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                frame,
                ctx.tokens().color_primary(),
                1.5,
                Some(crate::draw::Radius::uniform(frame.w.min(frame.h) * 0.5)),
            );
        }
    }
}

impl ThemeToggle {
    /// 创建初始处于亮色模式的主题切换按钮。
    pub fn new() -> Self {
        Self {
            dark: Cell::new(false),
            initial_dark: false,
            focused: false,
            pending_change: Cell::new(None),
        }
    }

    /// 设置组件的初始暗色模式状态。
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark.set(dark);
        self.initial_dark = dark;
        self
    }

    /// 返回组件当前是否处于暗色模式。
    pub fn is_dark(&self) -> bool {
        self.dark.get()
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.initial_dark = next.initial_dark;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::ThemeToggle {
            dark: self.dark.get(),
        }
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(32.0, 32.0)
    }

    fn toggle(&self) {
        let dark = !self.dark.get();
        self.dark.set(dark);
        self.pending_change.set(Some(dark));
    }
}

impl Default for ThemeToggle {
    fn default() -> Self {
        Self::new()
    }
}

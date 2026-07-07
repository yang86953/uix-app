//! Button — 纯文本按钮，外观由 StyleSet 预设驱动。

use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::impl_widget_component;
use crate::native::traits::input::KeyCode;
use crate::ui::style::{apply_style, Style, StyleSet, StyleState};
use crate::ui::traits::{EventHandler, WidgetLayout, WidgetRender};
use crate::ui::{EventResult, SystemEvent, WidgetTree};
use crate::ui::{SnapshotFields, SnapshotSource};

/// 按钮组件。业务绑定不存放在组件内，由 HandlerTable 按 ComponentId 管理。
pub struct Button {
    text: String,
    disabled: bool,
    block: bool,
    hovered: bool,
    pressed: bool,
    focused: bool,
    pub(crate) style_set: StyleSet,
    pub(crate) style: Style,
}

impl_widget_component!(Button; Layout, Render, Event; tab_index => 1);

impl SnapshotSource for Button {
    fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Button {
            text: self.text.clone(),
            disabled: self.disabled,
            block: self.block,
            style_set: self.style_set.clone(),
            style: self.style.clone(),
        }
    }
}

impl WidgetLayout for Button {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    fn layout_margin(&self) -> crate::core::EdgeInsets {
        self.style.margin
    }

    fn align_self(&self) -> Option<crate::ui::layout::AlignItems> {
        self.style.align_self
    }

    fn grid_cell(&self) -> Option<usize> {
        self.style.grid_cell
    }

    fn grid_column_span(&self) -> u32 {
        self.style.grid_column_span
    }

    fn grid_row_span(&self) -> u32 {
        self.style.grid_row_span
    }
}

impl EventHandler for Button {
    fn on_event(&mut self, event: &SystemEvent) -> EventResult {
        if self.disabled {
            return EventResult::NotHandled;
        }

        match event {
            SystemEvent::PointerDown { .. } => {
                self.pressed = true;
                EventResult::Handled
            }
            SystemEvent::PointerUp { .. } => {
                self.pressed = false;
                EventResult::Handled
            }
            SystemEvent::PointerEnter => {
                self.hovered = true;
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.hovered = false;
                self.pressed = false;
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.pressed = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } if matches!(*key, KeyCode::Enter | KeyCode::Space) => {
                self.pressed = true;
                EventResult::Handled
            }
            SystemEvent::KeyUp { key, .. } if matches!(*key, KeyCode::Enter | KeyCode::Space) => {
                self.pressed = false;
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }
}

impl WidgetRender for Button {
    fn render(&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let style = self.resolve_style();
        apply_style(ctx, frame, &style);
        if !self.text.is_empty() {
            let content = frame.inset(style.padding);
            ctx.text_center(
                &self.text,
                content,
                style.resolve_color(ctx.tokens()),
                style.resolve_font_size(ctx.tokens()),
            );
        }
    }

    fn dirty_rect(&self, frame: Rect) -> Rect {
        frame
    }
}

impl Button {
    pub fn new(text: impl Into<String>) -> Self {
        Self::assemble(text.into(), StyleSet::button_default(), false, false)
    }

    pub(crate) fn assemble(text: String, style_set: StyleSet, disabled: bool, block: bool) -> Self {
        Self {
            text,
            disabled,
            block,
            hovered: false,
            pressed: false,
            focused: false,
            style_set,
            style: Style::default(),
        }
    }

    pub fn style_set(mut self, style_set: StyleSet) -> Self {
        self.style_set = style_set;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.text = next.text;
        self.disabled = next.disabled;
        self.block = next.block;
        self.style_set = next.style_set;
        self.style = next.style;
    }

    pub fn primary(self) -> Self {
        self.style_set(StyleSet::button_primary())
    }

    pub fn ghost(self) -> Self {
        self.style_set(StyleSet::button_ghost())
    }

    pub fn danger(self) -> Self {
        self.style_set(StyleSet::button_danger())
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn block(mut self, block: bool) -> Self {
        self.block = block;
        self
    }

    fn resolve_style(&self) -> Style {
        self.style_set
            .resolve(StyleState {
                hovered: self.hovered,
                pressed: self.pressed,
                focused: self.focused,
                disabled: self.disabled,
            })
            .apply(self.style.clone())
    }

    fn intrinsic_size(&self) -> Size {
        let base = self.style_set.normal.clone().apply(self.style.clone());
        let font_size = base.font_size.default_size().max(1.0);
        let height = base.height.unwrap_or(32.0);
        let text_w = self.text.chars().count() as f32 * font_size * 0.55;
        let width = base
            .width
            .unwrap_or(text_w + base.padding.horizontal())
            .max(32.0);
        if self.block {
            Size::new(f32::MAX, height)
        } else {
            Size::new(width, height)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_clamps_button_size() {
        let measured = Button::new("abcdef").measure(Constraints::loose(Size::new(40.0, 24.0)));

        assert_eq!(measured, Size::new(40.0, 24.0));
    }
}

//! Checkbox — checkbox with label, checked/unchecked state.

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::native::traits::input::ControlSize;
use crate::ui::state::State;
use crate::ui::SnapshotFields;
use crate::ui::{ComponentId, EventResult, KeyCode, SemanticEvent, SystemEvent, WidgetTree};
use std::cell::Cell;

component! {
    pub struct Checkbox {
        checked: bool,
        checked_binding: Option<State<bool>>,
        disabled: bool,
        label: String,
        hovered: bool,
        focused: bool,
        checkbox_size: ControlSize,
        pending_change: Cell<Option<bool>>,
    }


    tab_index => (&self) -> i32 { 1 }
    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        self.sync_bound_checked();
        match event {
            SystemEvent::PointerDown { .. } => {
                self.toggle_checked();
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::PointerEnter => { self.hovered = true; EventResult::Handled }
            SystemEvent::PointerLeave => { self.hovered = false; EventResult::Handled }
            SystemEvent::FocusIn => { self.focused = true; EventResult::Handled }
            SystemEvent::FocusOut => { self.focused = false; EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
                if *key == KeyCode::Space || *key == KeyCode::Enter {
                    self.toggle_checked();
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            _ => EventResult::NotHandled
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|checked| SemanticEvent::change(id, checked.to_string()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.capture_bound_checked_dependency();
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let primary_border = ctx.tokens().color_primary_border();
        let border_c = ctx.tokens().color_border();
        let border_sec = ctx.tokens().color_border_secondary();
        let text_c = if self.disabled { ctx.tokens().color_text_quaternary() } else { ctx.tokens().color_text() };
        let white = Color::white();

        let box_size = self.box_size();
        let scale = self.visual_scale();
        let gap = 6.0 * scale;
        let font_size = self.font_size();
        let box_x = frame.x;
        let box_y = frame.y + (frame.h - box_size) * 0.5;
        let box_r = Rect::new(box_x, box_y, box_size, box_size);
        let corner = Some(crate::draw::Radius::uniform(3.0 * scale));

        if self.checked {
            let bg = if self.disabled { primary_border } else if self.hovered { primary_hover } else { primary };
            ctx.fill_rect(box_r, bg, corner);
            let cx = box_x + box_size * 0.5;
            let cy = box_y + box_size * 0.5;
            ctx.canvas_2d().draw_line(cx - 4.0 * scale, cy, cx - scale, cy + 3.0 * scale, white, 2.0 * scale);
            ctx.canvas_2d().draw_line(cx - scale, cy + 3.0 * scale, cx + 4.0 * scale, cy - 2.0 * scale, white, 2.0 * scale);
        } else {
            let border = if self.disabled { border_sec } else if self.hovered { primary_hover } else { border_c };
            ctx.stroke_rect(box_r, border, 1.5, corner);
        }

        if self.focused {
            ctx.stroke_rect(Rect::new(box_x - scale, box_y - scale, box_size + 2.0 * scale, box_size + 2.0 * scale), primary, 1.5, Some(crate::draw::Radius::uniform(4.0 * scale)));
        }

        let label_rect = Rect::new(box_x + box_size + gap, frame.y, (frame.w - box_size - gap).max(0.0), frame.h);
        ctx.text_center(&self.label, label_rect, text_c, font_size);
    }
}

impl Default for Checkbox {
    fn default() -> Self {
        Self::new("")
    }
}

impl Checkbox {
    pub fn new(label: impl Into<String>) -> Self {
        let config = crate::ui::config::use_config();
        Self {
            checked: false,
            checked_binding: None,
            disabled: false,
            label: label.into(),
            hovered: false,
            focused: false,
            checkbox_size: config.size,
            pending_change: Cell::new(None),
        }
    }
    /// 将勾选值绑定到外部 `State<bool>`；用户切换与外部更新保持双向同步。
    pub fn checked(mut self, state: &State<bool>) -> Self {
        self.checked_binding = Some(state.clone());
        self.checked = state.get();
        self
    }
    /// 设置非受控组件的初始勾选值。
    pub fn default_checked(mut self, value: bool) -> Self {
        self.checked_binding = None;
        self.checked = value;
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn size(mut self, size: ControlSize) -> Self {
        self.checkbox_size = size;
        self
    }
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    fn sync_bound_checked(&mut self) {
        if let Some(state) = self.checked_binding.as_ref() {
            self.checked = state.get();
        }
    }

    fn capture_bound_checked_dependency(&self) {
        if let Some(state) = self.checked_binding.as_ref() {
            let _ = state.get();
        }
    }

    fn toggle_checked(&mut self) {
        self.checked = !self.checked;
        if let Some(state) = self.checked_binding.as_ref() {
            if state.get() != self.checked {
                state.set(self.checked);
            }
        }
        self.pending_change.set(Some(self.checked));
    }

    fn intrinsic_size(&self) -> Size {
        let text_w = self.label.len() as f32 * self.font_size() * 0.62;
        Size::new(
            self.box_size() + 6.0 * self.visual_scale() + text_w,
            crate::ui::config::control_height(self.checkbox_size),
        )
    }

    fn visual_scale(&self) -> f32 {
        match self.checkbox_size {
            ControlSize::Small => 0.875,
            ControlSize::Medium => 1.0,
            ControlSize::Large => 1.125,
        }
    }

    fn box_size(&self) -> f32 {
        16.0 * self.visual_scale()
    }

    fn font_size(&self) -> f32 {
        match self.checkbox_size {
            ControlSize::Small => 12.0,
            ControlSize::Medium => 13.0,
            ControlSize::Large => 14.0,
        }
    }
}

impl Checkbox {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Checkbox {
            checked: self.checked,
            disabled: self.disabled,
            label: self.label.clone(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_checked = next.checked_binding.as_ref().map(|_| next.checked);
        self.checked_binding = next.checked_binding;
        if let Some(checked) = controlled_checked {
            self.checked = checked;
        }
        self.disabled = next.disabled;
        self.label = next.label;
        self.checkbox_size = next.checkbox_size;
    }
}

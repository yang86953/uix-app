//! Switch — on/off toggle switch.

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::native::windowing::input::ControlSize;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::reactive::state::State;
use crate::ui::SnapshotFields;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetTree,
};
use std::cell::Cell;

const THUMB_INSET: f32 = 2.0;

component! {
    pub struct Switch {
        checked: bool,
        checked_binding: Option<State<bool>>,
        disabled: bool,
        control_size: ControlSize,
        size: f32,
        hovered: bool,
        focused: bool,
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
            SystemEvent::PointerDown {
                button: MouseButton::Left,
                ..
            } => {
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

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.capture_bound_checked_dependency();
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let primary_border = ctx.tokens().color_primary_border();
        let fill_sec = ctx.tokens().color_fill_secondary();
        let fill_ter = ctx.tokens().color_fill_tertiary();
        let bg_container = ctx.tokens().color_bg_container();
        let bg_elevated = ctx.tokens().color_bg_elevated();

        let h = self.size;
        let w = self.track_width();
        let track_x = frame.x;
        let track_y = frame.y + (frame.h - h) * 0.5;
        let track_r = h * 0.5;
        let thumb_r = track_r - THUMB_INSET;
        let thumb_x = if self.checked {
            track_x + w - THUMB_INSET - thumb_r * 2.0
        } else {
            track_x + THUMB_INSET
        };

        let track_c = if self.checked {
            if self.disabled { primary_border } else if self.hovered { primary_hover } else { primary }
        } else {
            if self.disabled { fill_ter } else { fill_sec }
        };
        let thumb_c = if self.disabled { bg_container } else { bg_elevated };

        let radius = Some(crate::draw::Radius::uniform(track_r));
        ctx.fill_rect(Rect::new(track_x, track_y, w, h), track_c, radius);
        let thumb = Rect::new(
            thumb_x,
            track_y + THUMB_INSET,
            thumb_r * 2.0,
            thumb_r * 2.0,
        );
        ctx.fill_rect(thumb, thumb_c, Some(crate::draw::Radius::uniform(thumb_r)));

        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(Rect::new(track_x - 1.0, track_y - 1.0, w + 2.0, h + 2.0), primary, 1.5, radius);
        }
    }
}

impl Default for Switch {
    fn default() -> Self {
        Self::new()
    }
}

impl Switch {
    pub fn new() -> Self {
        let config = crate::ui::component::config::use_config();
        Self {
            checked: false,
            checked_binding: None,
            disabled: false,
            control_size: config.size,
            size: Self::track_height(config.size),
            hovered: false,
            focused: false,
            pending_change: Cell::new(None),
        }
    }
    /// 将开关值绑定到外部 `State<bool>`；用户切换与外部更新保持双向同步。
    pub fn checked(mut self, state: &State<bool>) -> Self {
        self.checked_binding = Some(state.clone());
        self.checked = state.get();
        self
    }
    /// 设置非受控组件的初始开启值。
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
        self.control_size = size;
        self.size = Self::track_height(size);
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
        Size::new(
            self.track_width(),
            crate::ui::component::config::control_height(self.control_size),
        )
    }

    fn track_height(size: ControlSize) -> f32 {
        match size {
            ControlSize::Small => 16.0,
            ControlSize::Medium => 22.0,
            ControlSize::Large => 28.0,
        }
    }

    fn track_width(&self) -> f32 {
        self.size * 2.0 - 4.0
    }
}

impl Switch {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Switch {
            checked: self.checked,
            disabled: self.disabled,
            size: self.size,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_checked = next.checked_binding.as_ref().map(|_| next.checked);
        self.checked_binding = next.checked_binding;
        if let Some(checked) = controlled_checked {
            self.checked = checked;
        }
        self.disabled = next.disabled;
        self.control_size = next.control_size;
        self.size = next.size;
    }
}

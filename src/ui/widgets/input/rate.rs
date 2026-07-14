//! Rate widget — 星级评分，支持半星、hover 预览、disabled、clearable。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::state::State;
use crate::ui::SnapshotFields;
use crate::ui::{ComponentId, EventResult, KeyCode, SemanticEvent, SystemEvent, WidgetTree};
use std::cell::Cell;

component! {
    /// Rate — 星级评分，点击选择分值。
    pub struct Rate {
        count: usize,
        value: usize,
        value_binding: Option<State<u32>>,
        half: bool,
        disabled: bool,
        clearable: bool,
        hover_value: usize,
        focused: bool,
        pending_change: Cell<Option<usize>>,
        character: String,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_value();
        if self.disabled { return EventResult::NotHandled; }
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                let star_idx = (pos.x / 24.0) as usize;
                if star_idx < self.count {
                    let new_val = if self.half {
                        let in_star_x = pos.x - star_idx as f32 * 24.0;
                        star_idx * 2 + if in_star_x < 12.0 { 1 } else { 2 }
                    } else {
                        star_idx + 1
                    };
                    // clearable: 点击同一个值取消
                    if self.clearable && new_val == self.value {
                        self.set_value(0);
                    } else {
                        self.set_value(new_val);
                    }
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let star_idx = (pos.x / 24.0) as usize;
                if star_idx < self.count {
                    if self.half {
                        let in_star_x = pos.x - star_idx as f32 * 24.0;
                        self.hover_value = star_idx * 2 + if in_star_x < 12.0 { 1 } else { 2 };
                    } else {
                        self.hover_value = star_idx + 1;
                    }
                } else {
                    self.hover_value = 0;
                }
                EventResult::Handled
            }
            SystemEvent::PointerLeave => { self.hover_value = 0; EventResult::Handled }
            SystemEvent::FocusIn => { self.focused = true; EventResult::Handled }
            SystemEvent::FocusOut => { self.focused = false; EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Right | KeyCode::Up => {
                        let max_val = self.max_value();
                        if self.value < max_val {
                            self.set_value(self.value.saturating_add(1));
                        }
                        EventResult::Handled
                    }
                    KeyCode::Left | KeyCode::Down => {
                        if self.value > 0 {
                            self.set_value(self.value - 1);
                        }
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|value| SemanticEvent::change(id, value.to_string()))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.capture_bound_value_dependency();
        let warning = ctx.tokens().color_warning();
        let fill_tertiary = ctx.tokens().color_fill_tertiary();
        let text_quaternary = ctx.tokens().color_text_quaternary();
        let bg = ctx.tokens().color_bg_container();
        let ch = if self.character.is_empty() { "★" } else { &self.character };

        // hover 预览值优先于选中值
        let display_val = if self.hover_value > 0 { self.hover_value } else { self.value };

        for i in 0..self.count {
            let sx = frame.x + i as f32 * 24.0;
            let star_rect = Rect::new(sx, frame.y, 24.0, 24.0);
            let sy = ctx.visual_center_y(star_rect, 18.0);
            let filled = if self.half {
                display_val >= i * 2 + 2
            } else {
                display_val > i
            };

            let star_color = if self.disabled {
                if filled { ctx.tokens().color_warning_border() } else { text_quaternary }
            } else if filled {
                warning
            } else {
                fill_tertiary
            };

            ctx.draw_text(ch, Point::new(sx + 2.0, sy), star_color, 18.0);

            // 半星支持：左半填充
            if self.half && display_val == i * 2 + 1 {
                ctx.draw_text(ch, Point::new(sx + 2.0, sy), star_color, 18.0);
                ctx.fill_rect(Rect::new(sx + 14.0, sy, 10.0, 18.0), bg, None);
            }
        }
    }
}

impl Default for Rate {
    fn default() -> Self {
        Self::new()
    }
}

impl Rate {
    fn set_value(&mut self, value: usize) {
        let value = value.min(self.max_value());
        if self.value == value {
            return;
        }
        self.value = value;
        self.write_bound_value();
        self.pending_change.set(Some(value));
    }

    fn max_value(&self) -> usize {
        if self.half {
            self.count.saturating_mul(2)
        } else {
            self.count
        }
    }

    fn state_value(&self) -> u32 {
        self.value.min(u32::MAX as usize) as u32
    }

    fn sync_bound_value(&mut self) {
        let Some(value) = self.value_binding.as_ref().map(State::get) else {
            return;
        };
        self.value = (value as usize).min(self.max_value());
    }

    fn capture_bound_value_dependency(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            let _ = state.get();
        }
    }

    fn write_bound_value(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            let value = self.state_value();
            if state.get() != value {
                state.set(value);
            }
        }
    }

    pub fn new() -> Self {
        Self {
            count: 5,
            value: 0,
            value_binding: None,
            half: false,
            disabled: false,
            clearable: false,
            hover_value: 0,
            focused: false,
            pending_change: Cell::new(None),
            character: String::new(),
        }
    }
    pub fn count(mut self, n: usize) -> Self {
        self.count = n;
        if self.value_binding.is_some() {
            self.sync_bound_value();
        } else {
            self.value = self.value.min(self.max_value());
        }
        self
    }

    /// 将评分绑定到外部 `State<u32>`。
    pub fn value(mut self, state: &State<u32>) -> Self {
        self.value_binding = Some(state.clone());
        self.sync_bound_value();
        self
    }

    /// 设置非受控评分的初始值。
    pub fn default_value(mut self, value: u32) -> Self {
        self.value_binding = None;
        self.value = (value as usize).min(self.max_value());
        self
    }

    pub fn current_value(&self) -> u32 {
        self.state_value()
    }

    pub fn allow_half(mut self) -> Self {
        self.half = true;
        if self.value_binding.is_some() {
            self.sync_bound_value();
        } else {
            self.value = self.value.min(self.max_value());
        }
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn clearable(mut self) -> Self {
        self.clearable = true;
        self
    }
    pub fn character(mut self, c: impl Into<String>) -> Self {
        self.character = c.into();
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(self.count as f32 * 24.0, 24.0)
    }
}

impl Rate {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Rate {
            count: self.count,
            value: self.value,
            half: self.half,
            disabled: self.disabled,
            clearable: self.clearable,
            character: self.character.clone(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_value = next.value_binding.as_ref().map(|_| next.value);
        self.count = next.count;
        self.value_binding = next.value_binding;
        self.half = next.half;
        self.disabled = next.disabled;
        self.clearable = next.clearable;
        self.character = next.character;
        self.value = controlled_value.unwrap_or_else(|| self.value.min(self.max_value()));
    }
}

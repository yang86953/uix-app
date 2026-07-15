//! Segmented widget — 分段选择器，支持 disabled/hover/keyboard/focus。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Radius;
use crate::native::traits::input::ControlSize;
use crate::ui::state::State;
use crate::ui::{
    ComponentId, EventResult, KeyCode, SemanticEvent, SnapshotFields, SystemEvent, WidgetTree,
};
use std::cell::Cell;

component! {
    /// Segmented — 水平分段选择器。
    pub struct Segmented {
        options: Vec<String>,
        selected: usize,
        value_binding: Option<State<String>>,
        disabled: bool,
        disabled_options: Vec<bool>,
        segmented_size: ControlSize,
        hovered_idx: Option<usize>,
        focused: bool,
        pending_change: Cell<Option<usize>>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_value();
        if self.disabled { return EventResult::NotHandled; }
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                if let Some(idx) = self.segment_at(pos.x) {
                    if self.select_index(idx) {
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                self.hovered_idx = self.segment_at(pos.x);
                EventResult::Handled
            }
            SystemEvent::PointerEnter => { EventResult::Handled }
            SystemEvent::PointerLeave => { self.hovered_idx = None; EventResult::Handled }
            SystemEvent::FocusIn => { self.focused = true; EventResult::Handled }
            SystemEvent::FocusOut => { self.focused = false; EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Right | KeyCode::Down => {
                        let start = if self.selected < self.options.len() {
                            self.selected.saturating_add(1)
                        } else {
                            0
                        };
                        if let Some(next) = (start..self.options.len())
                            .find(|index| !self.is_segment_disabled(*index))
                        {
                            self.select_index(next);
                        }
                        EventResult::Handled
                    }
                    KeyCode::Left | KeyCode::Up => {
                        let end = self.selected.min(self.options.len());
                        if let Some(prev) = (0..end)
                            .rev()
                            .find(|index| !self.is_segment_disabled(*index))
                        {
                            self.select_index(prev);
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
            .map(|idx| SemanticEvent::change(id, idx.to_string()))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.capture_bound_value_dependency();
        let fill = ctx.tokens().color_fill_tertiary();
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_quaternary = ctx.tokens().color_text_quaternary();
        let bg = ctx.tokens().color_bg_elevated();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        let fill_quaternary = ctx.tokens().color_fill_quaternary();

        // 整体背景
        let container_bg = if self.disabled { fill_quaternary } else { fill };
        ctx.fill_rect(frame, container_bg, r);

        let mut x = frame.x;

        for (i, opt) in self.options.iter().enumerate() {
            let seg_w = self.segment_width(opt);
            let seg_disabled = self.is_segment_disabled(i);
            let is_hovered = self.hovered_idx == Some(i) && !seg_disabled;

            if i == self.selected {
                // 选中项：白色背景 + 主色文字
                let thumb_bg = if self.disabled { fill_quaternary } else { bg };
                let inset = 2.0 * self.visual_scale();
                ctx.fill_rect(Rect::new(x + inset, frame.y + inset, seg_w - 2.0 * inset, frame.h - 2.0 * inset), thumb_bg, Some(Radius::uniform(3.0 * self.visual_scale())));
                let tc = if self.disabled { text_quaternary } else { primary };
                ctx.text_center(opt, Rect::new(x, frame.y, seg_w, frame.h), tc, self.font_size());
            } else if seg_disabled {
                ctx.text_center(opt, Rect::new(x, frame.y, seg_w, frame.h), text_quaternary, self.font_size());
            } else if is_hovered {
                ctx.text_center(opt, Rect::new(x, frame.y, seg_w, frame.h), primary_hover, self.font_size());
            } else {
                ctx.text_center(opt, Rect::new(x, frame.y, seg_w, frame.h), text_secondary, self.font_size());
            }

            // 分隔线（非选中项之间）
            if i > 0 && i != self.selected && i - 1 != self.selected && !seg_disabled {
                let divider_color = ctx.tokens().color_border_secondary();
                let inset = 6.0 * self.visual_scale();
                ctx.canvas_2d().draw_line(x, frame.y + inset, x, frame.y + frame.h - inset, divider_color, 1.0);
            }

            x += seg_w;
        }

        // focus 边框指示
        if self.focused {
            ctx.stroke_rect(frame, primary, 1.5, r);
        }
    }
}

impl Segmented {
    fn select_index(&mut self, index: usize) -> bool {
        if index >= self.options.len() || self.is_segment_disabled(index) {
            return false;
        }
        if self.selected != index {
            self.selected = index;
            self.write_bound_value();
            self.pending_change.set(Some(index));
        }
        true
    }

    fn sync_bound_value(&mut self) {
        let Some(value) = self.value_binding.as_ref().map(State::get) else {
            return;
        };
        self.selected = self
            .options
            .iter()
            .position(|option| option == &value)
            .unwrap_or(usize::MAX);
    }

    fn capture_bound_value_dependency(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            let _ = state.get();
        }
    }

    fn write_bound_value(&self) {
        let Some(state) = self.value_binding.as_ref() else {
            return;
        };
        let Some(value) = self.options.get(self.selected) else {
            return;
        };
        if state.get() != *value {
            state.set(value.clone());
        }
    }

    fn intrinsic_size(&self) -> Size {
        if self.options.is_empty() {
            return Size::new(0.0, self.control_height());
        }
        let w = self
            .options
            .iter()
            .map(|o| self.segment_width(o))
            .sum::<f32>();
        Size::new(w, self.control_height())
    }

    fn segment_at(&self, px: f32) -> Option<usize> {
        let mut cum_x = 0.0f32;
        for (i, opt) in self.options.iter().enumerate() {
            let seg_w = self.segment_width(opt);
            if px >= cum_x && px <= cum_x + seg_w {
                return Some(i);
            }
            cum_x += seg_w;
        }
        None
    }

    fn is_segment_disabled(&self, idx: usize) -> bool {
        self.disabled || self.disabled_options.get(idx).copied().unwrap_or(false)
    }

    fn control_height(&self) -> f32 {
        crate::ui::config::control_height(self.segmented_size)
    }

    fn visual_scale(&self) -> f32 {
        self.control_height() / crate::ui::config::control_height(ControlSize::Medium)
    }

    fn font_size(&self) -> f32 {
        13.0 * self.visual_scale().sqrt()
    }

    fn segment_width(&self, option: &str) -> f32 {
        let scale = self.visual_scale();
        option.len() as f32 * 9.0 * scale.sqrt() + 24.0 * scale
    }
}

impl Default for Segmented {
    fn default() -> Self {
        Self::new(Vec::<String>::new())
    }
}

impl Segmented {
    pub fn new<I, S>(options: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let config = crate::ui::config::use_config();
        Self {
            options: options
                .into_iter()
                .map(|option| option.as_ref().to_owned())
                .collect(),
            selected: 0,
            value_binding: None,
            disabled: false,
            disabled_options: Vec::new(),
            segmented_size: config.size,
            hovered_idx: None,
            focused: false,
            pending_change: Cell::new(None),
        }
    }

    pub fn options<I, S>(mut self, options: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.options = options
            .into_iter()
            .map(|option| option.as_ref().to_owned())
            .collect();
        self.sync_bound_value();
        self
    }

    /// 设置非受控分段选择器的初始索引。
    pub fn default_selected(mut self, idx: usize) -> Self {
        self.value_binding = None;
        self.selected = idx;
        self
    }

    /// 将当前选项值绑定到外部 `State<String>`。
    pub fn value(mut self, state: &State<String>) -> Self {
        self.value_binding = Some(state.clone());
        self.sync_bound_value();
        self
    }

    pub fn current_value(&self) -> Option<String> {
        self.options.get(self.selected).cloned()
    }

    pub fn current_index(&self) -> Option<usize> {
        (self.selected < self.options.len()).then_some(self.selected)
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn size(mut self, size: ControlSize) -> Self {
        self.segmented_size = size;
        self
    }
    pub fn disable_option(mut self, idx: usize) -> Self {
        while self.disabled_options.len() <= idx {
            self.disabled_options.push(false);
        }
        self.disabled_options[idx] = true;
        self
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Segmented {
            options: self.options.clone(),
            selected: self.selected,
            disabled: self.disabled,
            disabled_options: self.disabled_options.clone(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_selected = next.value_binding.as_ref().map(|_| next.selected);
        self.options = next.options;
        self.value_binding = next.value_binding;
        self.disabled = next.disabled;
        self.disabled_options = next.disabled_options;
        self.segmented_size = next.segmented_size;
        self.selected = controlled_selected.unwrap_or_else(|| {
            (self.selected < self.options.len())
                .then_some(self.selected)
                .unwrap_or(usize::MAX)
        });
    }
}

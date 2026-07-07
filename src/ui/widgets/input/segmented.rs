//! Segmented widget — 分段选择器，支持 disabled/hover/keyboard/focus。

use crate::core::{Constraints, Rect, Size};
use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::draw::{traits::GraphicsEngine, Radius};
use crate::ui::{EventResult, KeyCode, SemanticEvent, SystemEvent, WidgetId, WidgetTree};
use std::cell::Cell;

define_widget! {
    /// Segmented — 水平分段选择器。
    pub struct Segmented {
        options: Vec<String>,
        selected: usize,
        disabled: bool,
        disabled_options: Vec<bool>,
        hovered_idx: Option<usize>,
        focused: bool,
        pending_change: Cell<Option<usize>>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        self.intrinsic_size()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                if let Some(idx) = self.segment_at(pos.x) {
                    if !self.is_segment_disabled(idx) {
                        if self.selected != idx {
                            self.selected = idx;
                            self.pending_change.set(Some(idx));
                        }
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
                        let mut next = self.selected + 1;
                        while next < self.options.len() && self.is_segment_disabled(next) {
                            next += 1;
                        }
                        if next < self.options.len() {
                            self.selected = next;
                            self.pending_change.set(Some(next));
                        }
                        EventResult::Handled
                    }
                    KeyCode::Left | KeyCode::Up => {
                        let mut prev = if self.selected > 0 { self.selected - 1 } else { 0 };
                        while prev > 0 && self.is_segment_disabled(prev) {
                            prev -= 1;
                        }
                        if !self.is_segment_disabled(prev)
                            && prev < self.options.len()
                            && self.selected != prev
                        {
                            self.selected = prev;
                            self.pending_change.set(Some(prev));
                        }
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|idx| SemanticEvent::change(id, idx.to_string()))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
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
            let seg_w = opt.len() as f32 * 9.0 + 24.0;
            let seg_disabled = self.is_segment_disabled(i);
            let is_hovered = self.hovered_idx == Some(i) && !seg_disabled;

            if i == self.selected {
                // 选中项：白色背景 + 主色文字
                let thumb_bg = if self.disabled { fill_quaternary } else { bg };
                ctx.fill_rect(Rect::new(x + 2.0, frame.y + 2.0, seg_w - 4.0, 28.0), thumb_bg, Some(Radius::uniform(3.0)));
                let tc = if self.disabled { text_quaternary } else { primary };
                ctx.text_center(opt, Rect::new(x, frame.y, seg_w, 32.0), tc, 13.0);
            } else if seg_disabled {
                ctx.text_center(opt, Rect::new(x, frame.y, seg_w, 32.0), text_quaternary, 13.0);
            } else if is_hovered {
                ctx.text_center(opt, Rect::new(x, frame.y, seg_w, 32.0), primary_hover, 13.0);
            } else {
                ctx.text_center(opt, Rect::new(x, frame.y, seg_w, 32.0), text_secondary, 13.0);
            }

            // 分隔线（非选中项之间）
            if i > 0 && i != self.selected && i - 1 != self.selected && !seg_disabled {
                let divider_color = ctx.tokens().color_border_secondary();
                ctx.canvas_2d().draw_line(x, frame.y + 6.0, x, frame.y + 26.0, divider_color, 1.0);
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
    fn intrinsic_size(&self) -> Size {
        if self.options.is_empty() {
            return Size::new(0.0, 32.0);
        }
        let w = self
            .options
            .iter()
            .map(|o| o.len() as f32 * 9.0 + 24.0)
            .sum::<f32>();
        Size::new(w, 32.0)
    }

    fn segment_at(&self, px: f32) -> Option<usize> {
        let mut cum_x = 0.0f32;
        for (i, opt) in self.options.iter().enumerate() {
            let seg_w = opt.len() as f32 * 9.0 + 24.0;
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
}

impl Default for Segmented {
    fn default() -> Self {
        Self::new()
    }
}

impl Segmented {
    pub fn new() -> Self {
        Self {
            options: Vec::new(),
            selected: 0,
            disabled: false,
            disabled_options: Vec::new(),
            hovered_idx: None,
            focused: false,
            pending_change: Cell::new(None),
        }
    }
    pub fn options(mut self, opts: Vec<impl Into<String>>) -> Self {
        self.options = opts.into_iter().map(|s| s.into()).collect();
        self
    }
    pub fn selected(mut self, idx: usize) -> Self {
        self.selected = idx;
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn disable_option(mut self, idx: usize) -> Self {
        while self.disabled_options.len() <= idx {
            self.disabled_options.push(false);
        }
        self.disabled_options[idx] = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_clamps_segmented_size() {
        let measured = Segmented::new()
            .options(vec!["One", "Two"])
            .measure(Constraints::loose(Size::new(70.0, 24.0)));

        assert_eq!(measured, Size::new(70.0, 24.0));
    }
}

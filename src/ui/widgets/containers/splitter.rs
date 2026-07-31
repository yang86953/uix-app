//! Splitter 分割面板 — 可拖拽调整子面板大小。
//!
//! 支持水平/垂直方向，任意数量面板，最小尺寸约束。

use std::cell::{Cell, RefCell};

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::ui::children::WidgetChildren;
use crate::ui::core::paint_context::PaintContext;
use crate::ui::SnapshotFields;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetComponent,
    WidgetTree,
};

component! {
    /// Splitter — 可拖拽分割面板容器。
    ///
    /// 子面板之间显示拖拽手柄，支持水平（左右排列）和垂直（上下排列）方向。
    pub struct Splitter {
        children: WidgetChildren,
        /// 水平（false=Row）或垂直（true=Column）
        vertical: bool,
        /// 面板比例（0.0~1.0 之间，各面板占比）
        ratios: Vec<f32>,
        /// 拖拽中的手柄索引
        dragging: Option<usize>,
        focused: bool,
        active_handle: usize,
        /// 各面板最小尺寸（像素）
        min_sizes: Vec<f32>,
        /// 手柄宽度
        handle_size: f32,
        /// 当前 frame（用于 hit-test）
        last_frame: Cell<Option<Rect>>,
        layout_requested: Cell<bool>,
        pending_change: RefCell<Option<String>>,
    }

    tab_index => (&self) -> i32 { i32::from(self.ratios.len() > 1) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    flex_grow => (&self) -> f32 { 1.0 }

    build => (&self) -> Vec<Box<dyn WidgetComponent>> {
        self.children.take()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(frame) = self.last_frame.get() {
                    if let Some(idx) = self.hit_test_handle(frame, *pos) {
                        self.dragging = Some(idx);
                        self.active_handle = idx;
                        self.focused = true;
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerUp {
                button: MouseButton::Left,
                ..
            } => {
                if self.dragging.is_none() {
                    return EventResult::NotHandled;
                }
                self.dragging = None;
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if let Some(idx) = self.dragging {
                    if let Some(frame) = self.last_frame.get() {
                        self.update_ratios(frame, idx, *pos);
                    }
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.dragging = None;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } if self.ratios.len() > 1 => {
                match key {
                    KeyCode::PageUp => {
                        self.active_handle = self.active_handle.saturating_sub(1);
                        return EventResult::Handled;
                    }
                    KeyCode::PageDown => {
                        self.active_handle =
                            (self.active_handle + 1).min(self.ratios.len().saturating_sub(2));
                        return EventResult::Handled;
                    }
                    _ => {}
                }
                let Some(frame) = self.last_frame.get() else {
                    return EventResult::NotHandled;
                };
                match (self.vertical, key) {
                    (false, KeyCode::Left) | (true, KeyCode::Up) => {
                        self.move_active_handle(frame, -8.0);
                    }
                    (false, KeyCode::Right) | (true, KeyCode::Down) => {
                        self.move_active_handle(frame, 8.0);
                    }
                    (_, KeyCode::Home) => {
                        self.move_active_handle_to_limit(frame, false);
                    }
                    (_, KeyCode::End) => {
                        self.move_active_handle_to_limit(frame, true);
                    }
                    _ => return EventResult::NotHandled,
                }
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|ratios| SemanticEvent::change(id, ratios))
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    wants_continuous_pointer_move => (&self) -> bool { self.dragging.is_some() }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        ctx.fill_rect(frame, ctx.tokens().color_bg_container(), None);

        let n = self.ratios.len();
        if n <= 1 { return; }
        let total = if self.vertical { frame.h } else { frame.w };
        let handle_total = self.handle_size * (n - 1) as f32;
        let content_total = (total - handle_total).max(0.0);
        let handle_color = ctx.tokens().color_border();
        let dot_color = ctx.tokens().color_text_quaternary();
        let primary = ctx.tokens().color_primary();

        let mut pos = 0.0;
        for i in 0..n - 1 {
            pos += self.ratios[i] * content_total;
            let handle_rect = if self.vertical {
                Rect::new(frame.x, frame.y + pos, frame.w, self.handle_size)
            } else {
                Rect::new(frame.x + pos, frame.y, self.handle_size, frame.h)
            };
            let active = self.focused && i == self.active_handle || self.dragging == Some(i);
            ctx.fill_rect(handle_rect, if active { primary } else { handle_color }, None);
            let grip_size = 16.0_f32.min(frame.w).min(frame.h);
            let grip_frame = Rect::new(
                handle_rect.x + (handle_rect.w - grip_size) * 0.5,
                handle_rect.y + (handle_rect.h - grip_size) * 0.5,
                grip_size,
                grip_size,
            );
            crate::ui::widgets::Icon::paint_in_frame(
                ctx,
                if self.vertical {
                    "grip-horizontal"
                } else {
                    "grip-vertical"
                },
                grip_frame,
                dot_color,
                14.0_f32.min(grip_size * 0.85),
            );
            if active {
                ctx.stroke_rect(handle_rect, primary, 1.5, None);
            }
            pos += self.handle_size;
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        let mut result = Vec::new();
        let n = children.len().min(self.ratios.len());
        if n == 0 { return result; }

        let total = if self.vertical { frame.h } else { frame.w };
        let handle_total = self.handle_size * (n - 1) as f32;
        let content_total = (total - handle_total).max(0.0);
        let mut pos = if self.vertical { frame.y } else { frame.x };

        for (child, ratio) in children.iter().zip(&self.ratios).take(n) {
            let size = ratio * content_total;
            let child_frame = if self.vertical {
                Rect::new(frame.x, pos, frame.w, size)
            } else {
                Rect::new(pos, frame.y, size, frame.h)
            };
            result.push((child.id, child_frame));
            pos += size + self.handle_size;
        }
        result
    }
}

impl Default for Splitter {
    fn default() -> Self {
        Self::new()
    }
}

impl Splitter {
    pub fn new() -> Self {
        Self {
            children: WidgetChildren::new(),
            vertical: false,
            ratios: vec![0.5, 0.5],
            dragging: None,
            focused: false,
            active_handle: 0,
            min_sizes: vec![50.0, 50.0],
            handle_size: 6.0,
            last_frame: Cell::new(None),
            layout_requested: Cell::new(false),
            pending_change: RefCell::new(None),
        }
    }

    pub fn panels(mut self, count: usize) -> Self {
        let count = count.max(1);
        let ratio = 1.0 / count as f32;
        self.ratios = vec![ratio; count];
        self.min_sizes = vec![50.0; count];
        self
    }

    pub fn vertical(mut self, v: bool) -> Self {
        self.vertical = v;
        self
    }
    pub fn min_size(mut self, index: usize, size: f32) -> Self {
        if index < self.min_sizes.len() {
            self.min_sizes[index] = Self::normalize_size(size);
        }
        self
    }

    pub fn ratios(&self) -> &[f32] {
        &self.ratios
    }

    pub fn active_handle(&self) -> usize {
        self.active_handle
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(300.0, 200.0)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.vertical = next.vertical;
        self.min_sizes = next.min_sizes;
        self.handle_size = next.handle_size;
        if self.ratios.len() != next.ratios.len() {
            self.ratios = next.ratios;
        }
        if self
            .dragging
            .is_some_and(|idx| idx + 1 >= self.ratios.len())
        {
            self.dragging = None;
        }
        self.active_handle = self.active_handle.min(self.ratios.len().saturating_sub(2));
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Splitter {
            vertical: self.vertical,
            panel_count: self.ratios.len(),
            min_sizes: self.min_sizes.clone(),
            handle_size: self.handle_size,
            ratios: self.ratios.clone(),
            active_handle: self.active_handle,
        }
    }

    fn hit_test_handle(&self, frame: Rect, pos: Point) -> Option<usize> {
        if !frame.contains(pos) {
            return None;
        }
        let n = self.ratios.len();
        if n <= 1 {
            return None;
        }
        let total = if self.vertical { frame.h } else { frame.w };
        let handle_total = self.handle_size * (n - 1) as f32;
        let content_total = total - handle_total;
        if content_total <= 0.0 {
            return None;
        }

        let mut cursor = 0.0;
        for i in 0..n - 1 {
            cursor += self.ratios[i] * content_total;
            let hit = if self.vertical {
                pos.y >= frame.y + cursor && pos.y <= frame.y + cursor + self.handle_size
            } else {
                pos.x >= frame.x + cursor && pos.x <= frame.x + cursor + self.handle_size
            };
            if hit {
                return Some(i);
            }
            cursor += self.handle_size;
        }
        None
    }

    fn update_ratios(&mut self, frame: Rect, idx: usize, pos: Point) -> bool {
        let n = self.ratios.len();
        if idx >= n - 1 {
            return false;
        }
        let total = if self.vertical { frame.h } else { frame.w };
        let handle_total = self.handle_size * (n - 1) as f32;
        let content_total = total - handle_total;
        if content_total <= 0.0 {
            return false;
        }

        let raw_pos = if self.vertical {
            pos.y - frame.y
        } else {
            pos.x - frame.x
        };
        let adjusted = (raw_pos - idx as f32 * self.handle_size)
            .max(0.0)
            .min(content_total);
        let prefix = self.ratios[..idx].iter().sum::<f32>() * content_total;
        self.set_handle_left_size(frame, idx, adjusted - prefix)
    }

    fn move_active_handle(&mut self, frame: Rect, delta: f32) -> bool {
        let Some((_, left, _)) = self.handle_pair_sizes(frame, self.active_handle) else {
            return false;
        };
        self.set_handle_left_size(frame, self.active_handle, left + delta)
    }

    fn move_active_handle_to_limit(&mut self, frame: Rect, towards_end: bool) -> bool {
        let idx = self.active_handle;
        let Some((_, left, right)) = self.handle_pair_sizes(frame, idx) else {
            return false;
        };
        let desired = if towards_end {
            left + right - self.min_sizes[idx + 1]
        } else {
            self.min_sizes[idx]
        };
        self.set_handle_left_size(frame, idx, desired)
    }

    fn set_handle_left_size(&mut self, frame: Rect, idx: usize, desired: f32) -> bool {
        let Some((content_total, left, right)) = self.handle_pair_sizes(frame, idx) else {
            return false;
        };
        let pair_total = left + right;
        let min_left = self.min_sizes[idx];
        let max_left = pair_total - self.min_sizes[idx + 1];
        if min_left > max_left {
            return false;
        }
        let next_left = desired.clamp(min_left, max_left);
        if (next_left - left).abs() <= f32::EPSILON {
            return false;
        }
        self.ratios[idx] = next_left / content_total;
        self.ratios[idx + 1] = (pair_total - next_left) / content_total;
        self.layout_requested.set(true);
        self.pending_change.replace(Some(self.ratios_payload()));
        true
    }

    fn handle_pair_sizes(&self, frame: Rect, idx: usize) -> Option<(f32, f32, f32)> {
        if idx + 1 >= self.ratios.len() {
            return None;
        }
        let total = if self.vertical { frame.h } else { frame.w };
        let handles = self.handle_size * self.ratios.len().saturating_sub(1) as f32;
        let content_total = total - handles;
        (content_total > 0.0).then(|| {
            (
                content_total,
                self.ratios[idx] * content_total,
                self.ratios[idx + 1] * content_total,
            )
        })
    }

    fn ratios_payload(&self) -> String {
        self.ratios
            .iter()
            .map(|ratio| format!("{ratio:.6}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn normalize_size(size: f32) -> f32 {
        if size.is_finite() {
            size.max(0.0)
        } else {
            0.0
        }
    }
}

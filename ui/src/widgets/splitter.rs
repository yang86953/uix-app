//! Splitter 分割面板 — 可拖拽调整子面板大小。
//!
//! 支持水平/垂直方向，任意数量面板，最小尺寸约束。

use std::cell::Cell;

use uix_core::{Point, Rect, Size};
use crate::define_widget;
use uix_graphics::GraphicsEngine;
use crate::children::WidgetChildren;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, Widget, WidgetEvent, WidgetId, WidgetTree};

define_widget! {
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
        /// 各面板最小尺寸（像素）
        min_sizes: Vec<f32>,
        /// 手柄宽度
        handle_size: f32,
        /// 当前 frame（用于 hit-test）
        last_frame: Cell<Option<Rect>>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(300.0, 200.0)
    }

    flex_grow => (&self) -> f32 { 1.0 }

    build => (&self) -> Vec<Box<dyn Widget>> {
        self.children.take()
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                if let Some(frame) = self.last_frame.get() {
                    if let Some(idx) = self.hit_test_handle(frame, *pos) {
                        self.dragging = Some(idx);
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            WidgetEvent::MouseUp { .. } => {
                self.dragging = None;
                EventResult::Handled
            }
            WidgetEvent::MouseMove { pos, .. } => {
                if let Some(idx) = self.dragging {
                    if let Some(frame) = self.last_frame.get() {
                        self.update_ratios(frame, idx, *pos);
                    }
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        self.last_frame.set(Some(frame));
        ctx.fill_rect(frame, ctx.tokens().color_bg_container(), None);

        let n = self.ratios.len();
        if n <= 1 { return; }
        let total = if self.vertical { frame.h } else { frame.w };
        let handle_total = self.handle_size * (n - 1) as f32;
        let content_total = total - handle_total;
        let handle_color = ctx.tokens().color_border();
        let dot_color = ctx.tokens().color_text_quaternary();

        let mut pos = 0.0;
        for i in 0..n - 1 {
            pos += self.ratios[i] * content_total;
            let handle_rect = if self.vertical {
                Rect::new(frame.x, frame.y + pos, frame.w, self.handle_size)
            } else {
                Rect::new(frame.x + pos, frame.y, self.handle_size, frame.h)
            };
            ctx.fill_rect(handle_rect, handle_color, None);
            // 手柄中点
            if self.vertical {
                let cy = handle_rect.y + self.handle_size * 0.5;
                ctx.fill_rect(Rect::new(frame.x + frame.w * 0.5 - 6.0, cy - 1.0, 12.0, 2.0), dot_color, None);
            } else {
                let cx = handle_rect.x + self.handle_size * 0.5;
                ctx.fill_rect(Rect::new(cx - 1.0, frame.y + frame.h * 0.5 - 6.0, 2.0, 12.0), dot_color, None);
            }
            pos += self.handle_size;
        }
    }

    layout_children => (&self, frame: Rect, children: &[WidgetId], _tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        let mut result = Vec::new();
        let n = children.len().min(self.ratios.len());
        if n == 0 { return result; }

        let total = if self.vertical { frame.h } else { frame.w };
        let handle_total = self.handle_size * (n - 1).max(0) as f32;
        let content_total = total - handle_total;
        let mut pos = if self.vertical { frame.y } else { frame.x };

        for i in 0..n {
            let size = self.ratios[i] * content_total;
            let child_frame = if self.vertical {
                Rect::new(frame.x, pos, frame.w, size)
            } else {
                Rect::new(pos, frame.y, size, frame.h)
            };
            result.push((children[i], child_frame));
            pos += size + self.handle_size;
        }
        result
    }
}

impl Splitter {
    pub fn new() -> Self {
        Self {
            children: WidgetChildren::new(),
            vertical: false,
            ratios: vec![0.5, 0.5],
            dragging: None,
            min_sizes: vec![50.0, 50.0],
            handle_size: 6.0,
            last_frame: Cell::new(None),
        }
    }

    pub fn panels(mut self, count: usize) -> Self {
        let ratio = 1.0 / count as f32;
        self.ratios = vec![ratio; count];
        self.min_sizes = vec![50.0; count];
        self
    }

    pub fn vertical(mut self, v: bool) -> Self { self.vertical = v; self }
    pub fn min_size(mut self, index: usize, size: f32) -> Self {
        if index < self.min_sizes.len() { self.min_sizes[index] = size; }
        self
    }

    fn hit_test_handle(&self, frame: Rect, pos: Point) -> Option<usize> {
        let n = self.ratios.len();
        if n <= 1 { return None; }
        let total = if self.vertical { frame.h } else { frame.w };
        let handle_total = self.handle_size * (n - 1) as f32;
        let content_total = total - handle_total;
        if content_total <= 0.0 { return None; }

        let mut cursor = 0.0;
        for i in 0..n - 1 {
            cursor += self.ratios[i] * content_total;
            let hit = if self.vertical {
                pos.y >= frame.y + cursor && pos.y <= frame.y + cursor + self.handle_size
            } else {
                pos.x >= frame.x + cursor && pos.x <= frame.x + cursor + self.handle_size
            };
            if hit { return Some(i); }
            cursor += self.handle_size;
        }
        None
    }

    fn update_ratios(&mut self, frame: Rect, idx: usize, pos: Point) {
        let n = self.ratios.len();
        if idx >= n - 1 { return; }
        let total = if self.vertical { frame.h } else { frame.w };
        let handle_total = self.handle_size * (n - 1) as f32;
        let content_total = total - handle_total;
        if content_total <= 0.0 { return; }

        let raw_pos = if self.vertical { pos.y - frame.y } else { pos.x - frame.x };
        let adjusted = (raw_pos - idx as f32 * self.handle_size).max(0.0).min(content_total);
        let old_left = self.ratios[..=idx].iter().sum::<f32>() * content_total;
        let delta = adjusted - old_left;
        let left = self.ratios[idx] * content_total + delta;
        let right = self.ratios[idx + 1] * content_total - delta;

        if left >= self.min_sizes[idx] && right >= self.min_sizes[idx + 1] {
            self.ratios[idx] = left / content_total;
            self.ratios[idx + 1] = right / content_total;
        }
    }
}

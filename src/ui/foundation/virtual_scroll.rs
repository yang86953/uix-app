//! VirtualScroll - virtual scrolling container.
//!
//! Renders only children near the viewport; useful for large Select, Tree,
//! Table, and similar lists.
use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::{EventResult, SystemEvent, WidgetNode, WidgetTree};

component! {
    pub struct VirtualScroll {
        item_count: usize,
        item_height: f32,
        scroll_offset: f32,
        renderer: Option<Box<dyn FnMut(usize) -> WidgetNode + 'static>>,
        total_height: f32,
        overscan: usize,
        needs_rebuild: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(0.0, self.total_height))
    }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if let SystemEvent::Wheel { delta, .. } = event {
            let max_offset = (self.total_height - 300.0).max(0.0);
            let new_offset = (self.scroll_offset - delta.y * 40.0)
                .clamp(0.0, max_offset);
            if (new_offset - self.scroll_offset).abs() > 0.5 {
                self.scroll_offset = new_offset;
                self.needs_rebuild = true;
            }
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }


}

impl Default for VirtualScroll {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualScroll {
    pub fn new() -> Self {
        Self {
            item_count: 0,
            item_height: 32.0,
            scroll_offset: 0.0,
            renderer: None,
            total_height: 0.0,
            overscan: 3,
            needs_rebuild: false,
        }
    }

    pub fn item_count(mut self, n: usize) -> Self {
        self.item_count = n;
        self.total_height = n as f32 * self.item_height;
        self
    }

    pub fn item_height(mut self, h: f32) -> Self {
        self.item_height = h;
        self.total_height = self.item_count as f32 * h;
        self
    }

    pub fn overscan(mut self, n: usize) -> Self {
        self.overscan = n;
        self
    }

    pub fn renderer<F: FnMut(usize) -> WidgetNode + 'static>(mut self, f: F) -> Self {
        self.renderer = Some(Box::new(f));
        self
    }

    pub fn scroll_range(&self, viewport_height: f32) -> (usize, usize) {
        let first = (self.scroll_offset / self.item_height).floor() as usize;
        let last = ((self.scroll_offset + viewport_height) / self.item_height).ceil() as usize;
        let start = first.saturating_sub(self.overscan);
        let end = (last + self.overscan).min(self.item_count);
        (start, end)
    }

    pub fn build_visible_children(&mut self, viewport_height: f32) -> Vec<WidgetNode> {
        if self.item_count == 0 {
            return Vec::new();
        }
        let (start, end) = self.scroll_range(viewport_height);
        let mut nodes = Vec::with_capacity(end.saturating_sub(start));
        for i in start..end {
            if let Some(ref mut renderer) = self.renderer {
                nodes.push(renderer(i));
            }
        }
        nodes
    }

    pub fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }

    pub fn scroll_ratio(&self, viewport_height: f32) -> f32 {
        let max_scroll = (self.total_height - viewport_height).max(1.0);
        (self.scroll_offset / max_scroll).clamp(0.0, 1.0)
    }
}

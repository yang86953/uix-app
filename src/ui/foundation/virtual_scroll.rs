//! VirtualScroll - virtual scrolling container.
//!
//! Renders only children near the viewport; useful for large Select, Tree,
//! Table, and similar lists.
use std::cell::{Cell, RefCell};

/// Visible index range `[start, end)` for a fixed-height virtual list.
pub fn virtual_list_index_range(
    item_count: usize,
    item_height: f32,
    scroll_offset: f32,
    viewport_height: f32,
    overscan: usize,
) -> (usize, usize) {
    if item_count == 0 || item_height <= 0.0 {
        return (0, 0);
    }
    // A public scroll offset can come from restored state or external input.
    // Keep invalid values at the safe origin instead of converting infinity or
    // NaN into implementation-defined indices.
    let scroll_offset = if scroll_offset.is_finite() {
        scroll_offset.max(0.0)
    } else {
        0.0
    };
    let first = (scroll_offset / item_height).floor() as usize;
    let last = ((scroll_offset + viewport_height) / item_height).ceil() as usize;
    let start = first.saturating_sub(overscan).min(item_count);
    let end = last.saturating_add(overscan).min(item_count).max(start);
    (start, end)
}

/// Scroll state shared by paint-based large lists (Table, Tree) and VirtualScroll.
#[derive(Debug, Clone)]
pub struct VirtualListScroll {
    scroll_offset: f32,
    overscan: usize,
    wheel_step: f32,
}

impl Default for VirtualListScroll {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualListScroll {
    pub fn new() -> Self {
        Self {
            scroll_offset: 0.0,
            overscan: 2,
            wheel_step: 40.0,
        }
    }

    pub fn overscan(mut self, n: usize) -> Self {
        self.overscan = n;
        self
    }

    pub fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }

    pub fn set_scroll_offset(&mut self, offset: f32) {
        self.scroll_offset = if offset.is_finite() {
            offset.max(0.0)
        } else {
            0.0
        };
    }

    pub fn max_scroll_offset(
        &self,
        item_count: usize,
        item_height: f32,
        viewport_height: f32,
    ) -> f32 {
        (item_count as f32 * item_height - viewport_height).max(0.0)
    }

    pub fn scroll_range(
        &self,
        item_count: usize,
        item_height: f32,
        viewport_height: f32,
    ) -> (usize, usize) {
        virtual_list_index_range(
            item_count,
            item_height,
            self.scroll_offset,
            viewport_height,
            self.overscan,
        )
    }

    /// Returns the applied scroll delta (0 when clamped at bounds).
    pub fn scroll_by(
        &mut self,
        dy: f32,
        item_count: usize,
        item_height: f32,
        viewport_height: f32,
    ) -> f32 {
        let old = self.scroll_offset;
        let max = self.max_scroll_offset(item_count, item_height, viewport_height);
        let new = (old + dy).clamp(0.0, max);
        self.scroll_offset = new;
        new - old
    }

    pub fn scroll_by_wheel(
        &mut self,
        wheel_delta_y: f32,
        item_count: usize,
        item_height: f32,
        viewport_height: f32,
    ) -> f32 {
        self.scroll_by(
            -wheel_delta_y * self.wheel_step,
            item_count,
            item_height,
            viewport_height,
        )
    }

    pub fn clamp_to_content(&mut self, item_count: usize, item_height: f32, viewport_height: f32) {
        let max = self.max_scroll_offset(item_count, item_height, viewport_height);
        self.scroll_offset = self.scroll_offset.min(max);
    }
}

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::{PaintContext, PaintPass};
use crate::ui::children::WidgetChildren;
use crate::ui::core::widget::WidgetNode;
use crate::ui::{ComponentId, EventResult, SystemEvent, WidgetTree};

component! {
    pub struct VirtualScroll {
        item_count: usize,
        item_height: f32,
        scroll_offset: f32,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        overscan: usize,
        #[snapshot(skip)]
        renderer: RefCell<Option<Box<dyn FnMut(usize) -> WidgetNode + 'static>>>,
        #[snapshot(skip)]
        children: WidgetChildren,
        pub(crate) visible_start: Cell<usize>,
        pub(crate) last_frame: Cell<Option<Rect>>,
        scroll_delta_strip: Cell<(f32, f32)>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    build => (&self) -> Vec<Box<dyn crate::ui::WidgetComponent>> {
        self.ensure_prepared(self.configured_viewport_height());
        self.children.take()
    }

    has_dynamic_content => (&self) -> bool {
        true
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if let SystemEvent::Wheel { delta, .. } = event {
            let view_h = self.viewport_height();
            let max_offset = (self.total_height() - view_h).max(0.0);
            let new_offset = (self.scroll_offset - delta.y * 40.0).clamp(0.0, max_offset);
            if (new_offset - self.scroll_offset).abs() > 0.5 {
                let old = self.scroll_offset;
                self.scroll_offset = new_offset;
                self.push_scroll_delta(0.0, new_offset - old);
            }
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }

    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        let delta = self.scroll_delta_strip.get();
        if delta.0.abs() > 0.01 || delta.1.abs() > 0.01 {
            self.scroll_delta_strip.set((0.0, 0.0));
            Some(delta)
        } else {
            None
        }
    }

    viewport_scroll_offset => (&self) -> Option<(f32, f32)> {
        Some((0.0, self.scroll_offset))
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        Some(frame)
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        frame
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame.set(Some(frame));
        if ctx.paint_pass() != PaintPass::Content {
            return;
        }
        let bg = ctx.tokens().color_bg_container();
        ctx.fill_rect(frame, bg, None);
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        let start = self.visible_start.get();
        children
            .iter()
            .enumerate()
            .map(|(local_i, child)| {
                let abs_i = start + local_i;
                let y = frame.y + abs_i as f32 * self.item_height - self.scroll_offset;
                (child.id, Rect::new(frame.x, y, frame.w, self.item_height))
            })
            .collect()
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
            fixed_width: None,
            fixed_height: None,
            overscan: 3,
            renderer: RefCell::new(None),
            children: WidgetChildren::new(),
            visible_start: Cell::new(0),
            last_frame: Cell::new(None),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
        }
    }

    /// Apply declarative configuration while retaining framework-owned scroll state.
    pub(crate) fn sync_from(&mut self, next: Self) {
        let viewport_height = self
            .last_frame
            .get()
            .map(|frame| frame.h)
            .unwrap_or(next.fixed_height.unwrap_or(300.0));
        let scroll_offset = self.scroll_offset;

        self.item_count = next.item_count;
        self.item_height = next.item_height;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
        self.overscan = next.overscan;
        self.renderer = next.renderer;
        self.children = WidgetChildren::new();
        self.scroll_offset = scroll_offset.min((self.total_height() - viewport_height).max(0.0));
        if self.item_count == 0 {
            self.visible_start.set(0);
        }
    }

    pub fn item_count(mut self, n: usize) -> Self {
        self.item_count = n;
        self
    }

    pub fn item_height(mut self, h: f32) -> Self {
        self.item_height = h;
        self
    }

    pub fn overscan(mut self, n: usize) -> Self {
        self.overscan = n;
        self
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }

    pub fn renderer<F: FnMut(usize) -> WidgetNode + 'static>(self, f: F) -> Self {
        *self.renderer.borrow_mut() = Some(Box::new(f));
        self
    }

    /// Total scrollable content height (fixed row height contract).
    pub fn total_height(&self) -> f32 {
        self.item_count as f32 * self.item_height
    }

    /// Viewport height from last layout frame or configured fixed height.
    pub fn viewport_height(&self) -> f32 {
        self.last_frame
            .get()
            .map(|f| f.h)
            .unwrap_or(self.fixed_height.unwrap_or(300.0))
    }

    pub fn scroll_range(&self, viewport_height: f32) -> (usize, usize) {
        virtual_list_index_range(
            self.item_count,
            self.item_height,
            self.scroll_offset,
            viewport_height,
            self.overscan,
        )
    }

    /// Whether the visible index window changed for the given viewport height.
    pub fn visible_range_changed(&self, viewport_height: f32) -> bool {
        self.scroll_range(viewport_height).0 != self.visible_start.get()
    }

    /// Populate `WidgetChildren` for the current scroll offset before tree build.
    pub fn prepare_for_build(&self, viewport_height: f32) {
        self.ensure_prepared(viewport_height);
    }

    /// Idempotent: skip when the visible window is already materialized.
    pub(crate) fn ensure_prepared(&self, viewport_height: f32) {
        if self.item_count == 0 {
            self.visible_start.set(0);
            self.children.set_all(Vec::new());
            return;
        }
        let (start, end) = self.scroll_range(viewport_height);
        if self.visible_start.get() == start && self.children.is_set() {
            return;
        }
        self.visible_start.set(start);
        let widgets = self.collect_visible_widgets(start, end);
        self.children.set_all(widgets);
    }

    pub fn build_visible_children(&self, viewport_height: f32) -> Vec<WidgetNode> {
        self.ensure_prepared(viewport_height);
        if self.item_count == 0 {
            return Vec::new();
        }
        let (start, end) = self.scroll_range(viewport_height);
        let mut nodes = Vec::with_capacity(end.saturating_sub(start));
        let mut guard = self.renderer.borrow_mut();
        if let Some(renderer) = guard.as_mut() {
            for i in start..end {
                nodes.push(renderer(i));
            }
        }
        nodes
    }

    /// Returns true when the visible index window no longer matches prepared children.
    pub(crate) fn needs_child_refresh(&self, viewport_height: f32) -> bool {
        self.item_count > 0
            && (!self.children.is_set() || self.visible_range_changed(viewport_height))
    }

    fn configured_viewport_height(&self) -> f32 {
        self.fixed_height.unwrap_or(300.0)
    }

    pub fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }

    pub fn scroll_ratio(&self, viewport_height: f32) -> f32 {
        let max_scroll = (self.total_height() - viewport_height).max(1.0);
        (self.scroll_offset / max_scroll).clamp(0.0, 1.0)
    }

    pub fn visible_start(&self) -> usize {
        self.visible_start.get()
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(
            self.fixed_width.unwrap_or(300.0),
            self.fixed_height.unwrap_or(300.0),
        )
    }

    fn collect_visible_widgets(
        &self,
        start: usize,
        end: usize,
    ) -> Vec<Box<dyn crate::ui::WidgetComponent>> {
        let mut guard = self.renderer.borrow_mut();
        let Some(renderer) = guard.as_mut() else {
            return Vec::new();
        };
        let mut widgets = Vec::with_capacity(end.saturating_sub(start));
        for i in start..end {
            widgets.push(renderer(i).widget);
        }
        widgets
    }

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }
}

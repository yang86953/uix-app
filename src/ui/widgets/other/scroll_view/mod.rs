//! ScrollView widget: a scrollable viewport that clips and scrolls children.

pub mod scrollbar;
#[allow(unused_imports)]
pub use scrollbar::*;

use std::cell::Cell;

use self::scrollbar::{ScrollBar, ScrollbarOrientation};
use crate::core::{Rect, Size};
use crate::define_widget;
use crate::draw::painting::{PaintContext, PaintPass};
use crate::ui::children::WidgetChildren;
use crate::ui::{EventResult, SystemEvent, WidgetComponent, WidgetCore, WidgetId, WidgetTree};

pub use crate::native::traits::input::ScrollDirection;

define_widget! {
    /// A scrollable viewport that clips its children.
    pub struct ScrollView {
        children: WidgetChildren,
        pub scroll_x: f32,
        pub scroll_y: f32,
        direction: ScrollDirection,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        flex_grow_val: f32,
        flex_shrink_val: f32,
        content_bounds: Cell<Option<Size>>,
        scroll_delta_strip: Cell<(f32, f32)>,
        scrollbar_v: ScrollBar,
        scrollbar_h: ScrollBar,
        last_frame: Cell<Option<Rect>>,
    }

    flex_grow => (&self) -> f32 { self.flex_grow_val }

    flex_shrink => (&self) -> f32 { self.flex_shrink_val }

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        Size::new(
            self.fixed_width.unwrap_or(300.0),
            self.fixed_height.unwrap_or(200.0),
        )
    }

    build => (&self) -> Vec<Box<dyn WidgetComponent>> {
        self.children.take()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::Wheel { delta, .. } => {
                let mut handled = false;
                let view = self.last_frame.get();
                let old_x = self.scroll_x;
                let old_y = self.scroll_y;

                if self.direction.can_scroll_y() && delta.y != 0.0 {
                    let view_h = view
                        .map(|f| f.h)
                        .unwrap_or(self.fixed_height.unwrap_or(200.0));
                    self.scroll_y = (self.scroll_y + delta.y * view_h * 0.25)
                        .clamp(0.0, self.max_scroll_y());
                    handled = true;
                }
                if self.direction.can_scroll_x() && delta.x != 0.0 {
                    let view_w = view
                        .map(|f| f.w)
                        .unwrap_or(self.fixed_width.unwrap_or(300.0));
                    self.scroll_x = (self.scroll_x + delta.x * view_w * 0.25)
                        .clamp(0.0, self.max_scroll_x());
                    handled = true;
                }

                self.scroll_delta_strip
                    .set((self.scroll_x - old_x, self.scroll_y - old_y));
                if handled {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerDown { pos, .. } => {
                let frame = match self.last_frame.get() {
                    Some(f) => f,
                    None => return EventResult::NotHandled,
                };
                if !self.scrollbar_v.show && !self.scrollbar_h.show {
                    return EventResult::NotHandled;
                }
                if self.direction.can_scroll_y()
                    && self.max_scroll_y() > 0.0
                    && self
                        .scrollbar_v
                        .hit_test_thumb(frame, *pos, self.scroll_y, self.max_scroll_y())
                {
                    self.scrollbar_v.dragging = true;
                    return EventResult::Handled;
                }
                if self.direction.can_scroll_x()
                    && self.max_scroll_x() > 0.0
                    && self
                        .scrollbar_h
                        .hit_test_thumb(frame, *pos, self.scroll_x, self.max_scroll_x())
                {
                    self.scrollbar_h.dragging = true;
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.scrollbar_v.dragging {
                    let frame = match self.last_frame.get() {
                        Some(f) => f,
                        None => return EventResult::Handled,
                    };
                    let max_y = self.max_scroll_y();
                    if max_y > 0.0 {
                        let old_y = self.scroll_y;
                        self.scroll_y = self
                            .scrollbar_v
                            .scroll_from_drag(frame, pos.y, self.scroll_y, max_y);
                        self.scroll_delta_strip.set((0.0, self.scroll_y - old_y));
                    }
                    return EventResult::Handled;
                }

                if self.scrollbar_h.dragging {
                    let frame = match self.last_frame.get() {
                        Some(f) => f,
                        None => return EventResult::Handled,
                    };
                    let max_x = self.max_scroll_x();
                    if max_x > 0.0 {
                        let old_x = self.scroll_x;
                        self.scroll_x = self
                            .scrollbar_h
                            .scroll_from_drag(frame, pos.x, self.scroll_x, max_x);
                        self.scroll_delta_strip.set((self.scroll_x - old_x, 0.0));
                    }
                    return EventResult::Handled;
                }

                if self.scrollbar_v.show || self.scrollbar_h.show {
                    if let Some(frame) = self.last_frame.get() {
                        let old_hover_v = self.scrollbar_v.hover;
                        let old_hover_h = self.scrollbar_h.hover;
                        self.scrollbar_v.hover = false;
                        self.scrollbar_h.hover = false;

                        if self.direction.can_scroll_y() && self.max_scroll_y() > 0.0 {
                            self.scrollbar_v.hover = self
                                .scrollbar_v
                                .hit_test_thumb(frame, *pos, self.scroll_y, self.max_scroll_y());
                        }
                        if self.direction.can_scroll_x() && self.max_scroll_x() > 0.0 {
                            self.scrollbar_h.hover = self
                                .scrollbar_h
                                .hit_test_thumb(frame, *pos, self.scroll_x, self.max_scroll_x());
                        }
                        if old_hover_v != self.scrollbar_v.hover
                            || old_hover_h != self.scrollbar_h.hover
                        {
                            return EventResult::Handled;
                        }
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerUp { .. } => {
                let was_dragging = self.scrollbar_v.dragging || self.scrollbar_h.dragging;
                self.scrollbar_v.dragging = false;
                self.scrollbar_h.dragging = false;
                if was_dragging {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                self.scrollbar_v.hover = false;
                self.scrollbar_h.hover = false;
                EventResult::NotHandled
            }
            _ => EventResult::NotHandled,
        }
    }

    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        let delta = self.scroll_delta_strip.get();
        if delta.0.abs() > 0.01 || delta.1.abs() > 0.01 {
            Some(delta)
        } else {
            None
        }
    }

    viewport_scroll_offset => (&self) -> Option<(f32, f32)> {
        Some((self.scroll_x, self.scroll_y))
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        Some(frame)
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        frame
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame.set(Some(frame));

        match ctx.paint_pass() {
            PaintPass::Content => {
                let bg = ctx.tokens().color_bg_container();
                ctx.fill_rect(frame, bg, None);
            }
            PaintPass::AfterChildren => {
                let bg = ctx.tokens().color_bg_container();
                if self.scrollbar_v.show && self.direction.can_scroll_y() {
                    let tr = self.scrollbar_v.track_rect_abs(frame);
                    ctx.fill_rect(tr, bg, None);
                    self.scrollbar_v
                        .render(frame, ctx, self.scroll_y, self.max_scroll_y());
                }
                if self.scrollbar_h.show && self.direction.can_scroll_x() {
                    let tr = self.scrollbar_h.track_rect_abs(frame);
                    ctx.fill_rect(tr, bg, None);
                    self.scrollbar_h
                        .render(frame, ctx, self.scroll_x, self.max_scroll_x());
                }
            }
        }
    }

    layout_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        let mut result = Vec::new();
        if children.is_empty() {
            self.content_bounds.set(Some(Size::new(frame.w, frame.h)));
            return result;
        }

        let mut max_right = frame.x;
        let mut max_bottom = frame.y;
        let mut cursor_y = 0.0f32;
        for &cid in children {
            let pref = tree
                .get(cid)
                .map(|c| c.preferred_size(None))
                .unwrap_or_default();
            let w = if pref.w <= 0.0 { frame.w } else { pref.w };
            let current_h = tree.get(cid).map(|c| c.frame().h).unwrap_or(0.0);
            let h = if pref.h > 0.0 {
                pref.h
            } else if current_h > 0.0 {
                current_h
            } else {
                frame.h
            };
            let r = Rect::new(frame.x, frame.y + cursor_y, w, h);
            result.push((cid, r));
            max_right = max_right.max(r.x + r.w);
            max_bottom = max_bottom.max(r.y + r.h);
            cursor_y += h;
        }

        let content_w = (max_right - frame.x).max(frame.w);
        let content_h = (max_bottom - frame.y).max(frame.h);
        self.content_bounds.set(Some(Size::new(content_w, content_h)));
        result
    }
}

impl ScrollView {
    pub fn new(direction: ScrollDirection) -> Self {
        Self {
            children: WidgetChildren::new(),
            scroll_x: 0.0,
            scroll_y: 0.0,
            direction,
            fixed_width: None,
            fixed_height: None,
            flex_grow_val: 0.0,
            flex_shrink_val: 1.0,
            content_bounds: Cell::new(None),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            scrollbar_v: ScrollBar::new(ScrollbarOrientation::Vertical),
            scrollbar_h: ScrollBar::new(ScrollbarOrientation::Horizontal),
            last_frame: Cell::new(None),
        }
    }

    pub fn child(self, w: impl WidgetComponent + 'static) -> Self {
        self.children.add(w);
        self
    }

    pub fn children(self, widgets: Vec<Box<dyn WidgetComponent>>) -> Self {
        self.children.set_all(widgets);
        self
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }

    pub fn flex_grow(mut self, v: f32) -> Self {
        self.flex_grow_val = v;
        self
    }

    pub fn flex_shrink(mut self, v: f32) -> Self {
        self.flex_shrink_val = v;
        self
    }

    pub fn show_scrollbar(mut self, v: bool) -> Self {
        self.scrollbar_v.show = v;
        self.scrollbar_h.show = v;
        self
    }

    pub fn scroll_to(mut self, x: f32, y: f32) -> Self {
        self.scroll_x = x.max(0.0);
        self.scroll_y = y.max(0.0);
        self
    }

    pub fn scroll_x(&self) -> f32 {
        self.scroll_x
    }

    pub fn scroll_y(&self) -> f32 {
        self.scroll_y
    }

    pub fn set_scroll_x(&mut self, x: f32) {
        self.scroll_x = x.max(0.0);
    }

    pub fn set_scroll_y(&mut self, y: f32) {
        self.scroll_y = y.max(0.0);
    }

    pub fn scroll_to_xy(&mut self, x: f32, y: f32) {
        self.scroll_x = x.max(0.0).min(self.max_scroll_x());
        self.scroll_y = y.max(0.0).min(self.max_scroll_y());
    }

    pub fn max_scroll_x(&self) -> f32 {
        match self.content_bounds.get() {
            Some(cs) => {
                let view_w = self
                    .last_frame
                    .get()
                    .map(|f| f.w)
                    .unwrap_or(self.fixed_width.unwrap_or(300.0));
                (cs.w - view_w).max(0.0)
            }
            None => 0.0,
        }
    }

    pub fn max_scroll_y(&self) -> f32 {
        match self.content_bounds.get() {
            Some(cs) => {
                let view_h = self
                    .last_frame
                    .get()
                    .map(|f| f.h)
                    .unwrap_or(self.fixed_height.unwrap_or(200.0));
                (cs.h - view_h).max(0.0)
            }
            None => 0.0,
        }
    }
}

impl Default for ScrollView {
    fn default() -> Self {
        Self::new(ScrollDirection::Vertical)
    }
}

#[cfg(test)]
#[path = "../../../../tests/ui/widgets/other/scroll_view/mod.rs"]
mod tests;


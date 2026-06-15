//! ScrollView widget — a scrollable viewport that clips and scrolls its children.
//!
//! Supports vertical and horizontal scrolling via mouse wheel, with optional
//! scrollbar rendering. Content offsets are managed per-frame via scroll_x/scroll_y.

pub mod scrollbar;
#[allow(unused_imports)]
pub use scrollbar::*;

use std::cell::Cell;

use crate::define_widget;
use crate::base::{Rect, Size};
use crate::ui::children::WidgetChildren;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, Widget, WidgetEvent, WidgetId, WidgetTree};
use self::scrollbar::{ScrollBar, ScrollbarOrientation};

/// Scroll direction for a ScrollView.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    /// Scroll vertically only.
    Vertical,
    /// Scroll horizontally only.
    Horizontal,
    /// Scroll in both directions.
    Both,
}

impl ScrollDirection {
    pub fn can_scroll_x(&self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }
    pub fn can_scroll_y(&self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }
}

define_widget! {
    /// A scrollable viewport that clips its children and supports mouse-wheel
    /// scrolling. Content offset is stored in `scroll_x` / `scroll_y`.
    ///
    /// # Layout
    ///
    /// Children are offset by (-scroll_x, -scroll_y) so that scrolling
    /// reveals different regions of the content. The viewport (visible area)
    /// is defined by the widget's `frame`.
    ///
    /// # Rendering
    ///
    /// A clip rect is pushed to the engine before rendering children, so
    /// content outside the viewport is masked out.
    pub struct ScrollView {
        children: WidgetChildren,
        scroll_x: f32,
        scroll_y: f32,
        /// 前一帧的 scroll 位置（用于计算滚动 delta 做 pixel buffer memmove）
        prev_scroll_x: f32,
        prev_scroll_y: f32,
        /// Velocity-based momentum scrolling: velocity accumulates on
        /// wheel events and decays via friction in on_update.
        velocity_x: f32,
        velocity_y: f32,
        direction: ScrollDirection,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        flex_grow_val: f32,
        flex_shrink_val: f32,
        content_bounds: Cell<Option<Size>>,
        scrollbar_v: ScrollBar,
        scrollbar_h: ScrollBar,
        last_frame: Cell<Option<Rect>>,
    }

    flex_grow => (&self) -> f32 { self.flex_grow_val }

    flex_shrink => (&self) -> f32 { self.flex_shrink_val }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(
            self.fixed_width.unwrap_or(300.0),
            self.fixed_height.unwrap_or(200.0),
        )
    }

    build => (&self) -> Vec<Box<dyn Widget>> {
        self.children.take()
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseWheel { delta } => {
                let mut handled = false;
                let view = self.last_frame.get();
                // Acceleration (pixels/s per wheel unit) scales with
                // viewport so the flick feel is consistent everywhere.
                if self.direction.can_scroll_y() && delta.y != 0.0 {
                    let view_h = view.map(|f| f.h)
                        .unwrap_or(self.fixed_height.unwrap_or(200.0));
                    let accel = view_h * 0.08;
                    self.velocity_y += delta.y * accel;
                    handled = true;
                }
                if self.direction.can_scroll_x() && delta.x != 0.0 {
                    let view_w = view.map(|f| f.w)
                        .unwrap_or(self.fixed_width.unwrap_or(300.0));
                    let accel = view_w * 0.08;
                    self.velocity_x += delta.x * accel;
                    handled = true;
                }
                if handled { EventResult::Handled } else { EventResult::NotHandled }
            }
            WidgetEvent::MouseDown { pos, .. } => {
                let frame = match self.last_frame.get() {
                    Some(f) => f,
                    None => return EventResult::NotHandled,
                };
                if !self.scrollbar_v.show && !self.scrollbar_h.show {
                    return EventResult::NotHandled;
                }
                // Vertical scrollbar thumb
                if self.direction.can_scroll_y() && self.max_scroll_y() > 0.0 {
                    if self.scrollbar_v.hit_test_thumb(frame, *pos, self.scroll_y, self.max_scroll_y()) {
                        self.scrollbar_v.dragging = true;
                        return EventResult::Handled;
                    }
                }
                // Horizontal scrollbar thumb
                if self.direction.can_scroll_x() && self.max_scroll_x() > 0.0 {
                    if self.scrollbar_h.hit_test_thumb(frame, *pos, self.scroll_x, self.max_scroll_x()) {
                        self.scrollbar_h.dragging = true;
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            WidgetEvent::MouseMove { pos } => {
                if self.scrollbar_v.dragging {
                    let frame = match self.last_frame.get() {
                        Some(f) => f,
                        None => return EventResult::Handled,
                    };
                    let max_y = self.max_scroll_y();
                    if max_y > 0.0 {
                        self.scroll_y = self.scrollbar_v.scroll_from_drag(
                            frame, pos.y, self.scroll_y, max_y,
                        );
                        self.velocity_y = 0.0;
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
                        self.scroll_x = self.scrollbar_h.scroll_from_drag(
                            frame, pos.x, self.scroll_x, max_x,
                        );
                        self.velocity_x = 0.0;
                    }
                    return EventResult::Handled;
                }
                // Not dragging: update thumb hover highlight.
                if self.scrollbar_v.show || self.scrollbar_h.show {
                    if let Some(frame) = self.last_frame.get() {
                        let old_hover_v = self.scrollbar_v.hover;
                        let old_hover_h = self.scrollbar_h.hover;
                        self.scrollbar_v.hover = false;
                        self.scrollbar_h.hover = false;

                        if self.direction.can_scroll_y() && self.max_scroll_y() > 0.0 {
                            self.scrollbar_v.hover = self.scrollbar_v.hit_test_thumb(
                                frame, *pos, self.scroll_y, self.max_scroll_y(),
                            );
                        }
                        if self.direction.can_scroll_x() && self.max_scroll_x() > 0.0 {
                            self.scrollbar_h.hover = self.scrollbar_h.hit_test_thumb(
                                frame, *pos, self.scroll_x, self.max_scroll_x(),
                            );
                        }
                        // If hover state changed, the scrollbar needs a repaint.
                        if old_hover_v != self.scrollbar_v.hover
                            || old_hover_h != self.scrollbar_h.hover
                        {
                            return EventResult::Handled;
                        }
                    }
                }
                EventResult::NotHandled
            }
            WidgetEvent::MouseUp { .. } => {
                let was_dragging = self.scrollbar_v.dragging || self.scrollbar_h.dragging;
                self.scrollbar_v.dragging = false;
                self.scrollbar_h.dragging = false;
                if was_dragging { EventResult::Handled } else { EventResult::NotHandled }
            }
            WidgetEvent::HoverLeave => {
                self.scrollbar_v.hover = false;
                self.scrollbar_h.hover = false;
                EventResult::NotHandled
            }
            _ => EventResult::NotHandled,
        }
    }

    on_update => (&mut self, dt: f32) {
        // 保存前一帧的 scroll 位置（像素缓冲滚动计算 delta 使用）
        self.prev_scroll_x = self.scroll_x;
        self.prev_scroll_y = self.scroll_y;

        // Momentum physics: position follows velocity, velocity decays
        // via friction.  This gives natural flick-and-decelerate feel.
        let damp = 1.0 - (8.0 * dt).min(0.95); // ~8s⁻¹ friction
        let threshold = 1.0; // snap when velocity is negligible

        self.scroll_x += self.velocity_x * dt;
        self.velocity_x *= damp;
        if self.velocity_x.abs() < threshold { self.velocity_x = 0.0; }

        self.scroll_y += self.velocity_y * dt;
        self.velocity_y *= damp;
        if self.velocity_y.abs() < threshold { self.velocity_y = 0.0; }

        if self.scroll_x < 0.0 { self.scroll_x = 0.0; self.velocity_x = 0.0; }
        let max_x = self.max_scroll_x();
        if self.scroll_x > max_x { self.scroll_x = max_x; self.velocity_x = 0.0; }
        if self.scroll_y < 0.0 { self.scroll_y = 0.0; self.velocity_y = 0.0; }
        let max_y = self.max_scroll_y();
        if self.scroll_y > max_y { self.scroll_y = max_y; self.velocity_y = 0.0; }
    }

    needs_continuous_update => (&self) -> bool {
        self.scrollbar_v.dragging
            || self.scrollbar_h.dragging
            || self.scrollbar_v.hover
            || self.scrollbar_h.hover
            || self.velocity_x.abs() > 0.5
            || self.velocity_y.abs() > 0.5
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        Some(frame)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // Save frame for scrollbar hit-testing in on_event.
        self.last_frame.set(Some(frame));
        // Background fill so the viewport is always opaque.
        let bg = ctx.tokens().color_bg_container();
        ctx.fill_rect(frame, bg, None);
    }

    post_render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // Scrollbar overlay drawn on top of children.
        if self.scrollbar_v.show || self.scrollbar_h.show {
            if self.direction.can_scroll_y() {
                self.scrollbar_v.render(frame, ctx, self.scroll_y, self.max_scroll_y());
            }
            if self.direction.can_scroll_x() {
                self.scrollbar_h.render(frame, ctx, self.scroll_x, self.max_scroll_x());
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

        // Compute child positions offset by scroll, and track content bounds
        let mut max_right = frame.x;
        let mut max_bottom = frame.y;
        let origin_x = frame.x - self.scroll_x;
        let origin_y = frame.y - self.scroll_y;

        for &cid in children {
            let pref = tree
                .get(cid)
                .map(|c| c.preferred_size(None))
                .unwrap_or_default();
            // When preferred width is 0 (unspecified), stretch to
            // fill the viewport so the child can use flex/Stretch
            // for its own children.
            let w = if pref.w <= 0.0 { frame.w } else { pref.w };
            let h = if pref.h <= 0.0 { frame.h } else { pref.h };
            let r = Rect::new(origin_x, origin_y, w, h);
            result.push((cid, r));
            max_right = max_right.max(r.x + r.w);
            max_bottom = max_bottom.max(r.y + r.h);
        }

        // Store content bounds for scrollbar calculation
        let content_w = (max_right - origin_x).max(frame.w);
        let content_h = (max_bottom - origin_y).max(frame.h);
        self.content_bounds.set(Some(Size::new(content_w, content_h)));

        result
    }

    // ── 像素缓冲滚动：dirty_rect 只返回新增 strip ──
    // 与 scroll_region 使用相同的 dy.round() 语义对齐，
    // 确保缓冲偏移量和脏区域一致。
    dirty_rect => (&self, frame: Rect) -> Rect {
        let dx = self.scroll_x - self.prev_scroll_x;
        let dy = self.scroll_y - self.prev_scroll_y;
        let int_dy = dy.round();
        let int_dx = dx.round();
        if int_dy > 0.0 {
            // 向下滚动：新增 strip 在底部
            let strip_h = int_dy.min(frame.h);
            Rect::new(frame.x, frame.y + frame.h - strip_h, frame.w, strip_h)
        } else if int_dy < 0.0 {
            // 向上滚动：新增 strip 在顶部
            let strip_h = (-int_dy).min(frame.h);
            Rect::new(frame.x, frame.y, frame.w, strip_h)
        } else if int_dx > 0.0 {
            // 向右滚动：新增 strip 在右侧
            let strip_w = int_dx.min(frame.w);
            Rect::new(frame.x + frame.w - strip_w, frame.y, strip_w, frame.h)
        } else if int_dx < 0.0 {
            // 向左滚动：新增 strip 在左侧
            let strip_w = (-int_dx).min(frame.w);
            Rect::new(frame.x, frame.y, strip_w, frame.h)
        } else {
            frame
        }
    }

    scroll_delta => (&self, _frame: Rect) -> Option<(f32, f32)> {
        let dx = self.scroll_x - self.prev_scroll_x;
        let dy = self.scroll_y - self.prev_scroll_y;
        let int_dx = dx.round();
        let int_dy = dy.round();
        if int_dx != 0.0 || int_dy != 0.0 {
            Some((int_dx, int_dy))
        } else {
            None
        }
    }
}

impl ScrollView {
    // ── Constructor ──

    pub fn new(direction: ScrollDirection) -> Self {
        Self {
            children: WidgetChildren::new(),
            scroll_x: 0.0,
            scroll_y: 0.0,
            prev_scroll_x: 0.0,
            prev_scroll_y: 0.0,
            velocity_x: 0.0,
            velocity_y: 0.0,
            direction,
            fixed_width: None,
            fixed_height: None,
            flex_grow_val: 0.0,
            flex_shrink_val: 1.0,
            content_bounds: Cell::new(None),
            scrollbar_v: ScrollBar::new(ScrollbarOrientation::Vertical),
            scrollbar_h: ScrollBar::new(ScrollbarOrientation::Horizontal),
            last_frame: Cell::new(None),
        }
    }

    // ── Builder methods ──

    pub fn child(self, w: impl Widget + 'static) -> Self {
        self.children.add(w);
        self
    }

    pub fn children(self, widgets: Vec<Box<dyn Widget>>) -> Self {
        self.children.set_all(widgets);
        self
    }

    /// Set the viewport size (visible area).
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

    /// Show or hide the scrollbar overlay.
    pub fn show_scrollbar(mut self, v: bool) -> Self {
        self.scrollbar_v.show = v;
        self.scrollbar_h.show = v;
        self
    }

    /// Set scroll offset manually (clamped to valid range).
    pub fn scroll_to(mut self, x: f32, y: f32) -> Self {
        self.scroll_x = x.max(0.0);
        self.scroll_y = y.max(0.0);
        self.velocity_x = 0.0;
        self.velocity_y = 0.0;
        self
    }

    // ── Runtime accessors ──

    pub fn scroll_x(&self) -> f32 { self.scroll_x }
    pub fn scroll_y(&self) -> f32 { self.scroll_y }
    pub fn set_scroll_x(&mut self, x: f32) {
        let v = x.max(0.0);
        self.scroll_x = v;
        self.velocity_x = 0.0;
    }
    pub fn set_scroll_y(&mut self, y: f32) {
        let v = y.max(0.0);
        self.scroll_y = v;
        self.velocity_y = 0.0;
    }

    /// Programmatically scroll to a position (clamped to valid range).
    pub fn scroll_to_xy(&mut self, x: f32, y: f32) {
        let vx = x.max(0.0).min(self.max_scroll_x());
        let vy = y.max(0.0).min(self.max_scroll_y());
        self.scroll_x = vx;
        self.scroll_y = vy;
        self.velocity_x = 0.0;
        self.velocity_y = 0.0;
    }

    // ── Scroll range ──────────────────────────────────────────────────

    /// Maximum scrollable offset along X axis.
    pub fn max_scroll_x(&self) -> f32 {
        match self.content_bounds.get() {
            Some(cs) => {
                let view_w = self.last_frame.get().map(|f| f.w)
                    .unwrap_or(self.fixed_width.unwrap_or(300.0));
                (cs.w - view_w).max(0.0)
            }
            None => f32::MAX,
        }
    }

    /// Maximum scrollable offset along Y axis.
    pub fn max_scroll_y(&self) -> f32 {
        match self.content_bounds.get() {
            Some(cs) => {
                let view_h = self.last_frame.get().map(|f| f.h)
                    .unwrap_or(self.fixed_height.unwrap_or(200.0));
                (cs.h - view_h).max(0.0)
            }
            None => f32::MAX,
        }
    }
}

impl Default for ScrollView {
    fn default() -> Self {
        Self::new(ScrollDirection::Vertical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::Size;
    use crate::ui::widget::{Widget, WidgetId, WidgetTree, WidgetCore};
    use crate::base::{Point, Rect};

    /// A simple fixed-size widget for testing.
    struct FixedWidget {
        size: Size,
        #[allow(dead_code)]
        id: WidgetId,
    }

    impl Widget for FixedWidget {
        fn preferred_size(&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
            self.size
        }
        fn render(
            &self,
            _frame: Rect,
            _ctx: &mut crate::ui::render_context::RenderContext,
            _tree: &WidgetTree,
        ) {
        }
    }

    #[test]
    fn scrollview_default_size() {
        let sv = ScrollView::new(ScrollDirection::Vertical);
        let ps = sv.preferred_size(None);
        assert_eq!(ps, Size::new(300.0, 200.0));
    }

    #[test]
    fn scrollview_custom_size() {
        let sv = ScrollView::new(ScrollDirection::Both).size(400.0, 300.0);
        let ps = sv.preferred_size(None);
        assert_eq!(ps, Size::new(400.0, 300.0));
    }

    #[test]
    fn scrollview_mouse_wheel_vertical_up() {
        let mut sv = ScrollView::new(ScrollDirection::Vertical);
        sv.scroll_y = 60.0;
        assert_eq!(sv.scroll_y, 60.0);

        let result = sv.on_event(&WidgetEvent::MouseWheel {
            delta: Point::new(0.0, 1.0),
        });
        assert_eq!(result, EventResult::Handled);
        assert_eq!(sv.velocity_y, 16.0);
    }

    #[test]
    fn scrollview_mouse_wheel_scrolls_down() {
        let mut sv = ScrollView::new(ScrollDirection::Vertical);
        assert_eq!(sv.scroll_y, 0.0);

        let result = sv.on_event(&WidgetEvent::MouseWheel {
            delta: Point::new(0.0, -1.0),
        });
        assert_eq!(result, EventResult::Handled);
        assert_eq!(sv.velocity_y, -16.0);
    }

    #[test]
    fn scrollview_mouse_wheel_horizontal() {
        let mut sv = ScrollView::new(ScrollDirection::Horizontal);
        assert_eq!(sv.scroll_x, 0.0);

        let result = sv.on_event(&WidgetEvent::MouseWheel {
            delta: Point::new(-1.0, 0.0),
        });
        assert_eq!(result, EventResult::Handled);
        assert_eq!(sv.velocity_x, -24.0);
    }

    #[test]
    fn scrollview_mouse_wheel_both() {
        let mut sv = ScrollView::new(ScrollDirection::Both);

        let result = sv.on_event(&WidgetEvent::MouseWheel {
            delta: Point::new(-1.0, -2.0),
        });
        assert_eq!(result, EventResult::Handled);
        assert_eq!(sv.velocity_x, -24.0);
        assert_eq!(sv.velocity_y, -32.0);
    }

    #[test]
    fn scrollview_scroll_clamped_to_zero() {
        let mut sv = ScrollView::new(ScrollDirection::Vertical);
        sv.scroll_y = 10.0;

        let result = sv.on_event(&WidgetEvent::MouseWheel {
            delta: Point::new(0.0, 1.0),
        });
        assert_eq!(result, EventResult::Handled);
        assert_eq!(sv.velocity_y, 16.0);
    }

    #[test]
    fn scrollview_scroll_to() {
        let sv = ScrollView::new(ScrollDirection::Vertical).scroll_to(0.0, 100.0);
        assert_eq!(sv.scroll_x, 0.0);
        assert_eq!(sv.scroll_y, 100.0);
    }

    #[test]
    fn scrollview_scroll_to_clamped() {
        let sv = ScrollView::new(ScrollDirection::Vertical).scroll_to(-10.0, -50.0);
        assert_eq!(sv.scroll_x, 0.0);
        assert_eq!(sv.scroll_y, 0.0);
    }

    #[test]
    fn scrollview_set_scroll_programmatically() {
        let mut sv = ScrollView::new(ScrollDirection::Both);
        sv.set_scroll_x(50.0);
        sv.set_scroll_y(75.0);
        assert_eq!(sv.scroll_x, 50.0);
        assert_eq!(sv.scroll_y, 75.0);
    }

    #[test]
    fn scrollview_child_builder() {
        let sv = ScrollView::new(ScrollDirection::Vertical)
            .child(FixedWidget { size: Size::new(100.0, 200.0), id: 0 });
        assert!(sv.children.is_set());
        assert_eq!(sv.children.len(), 1);
        let children = sv.children.take();
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn scrollview_layout_children_offsets_by_scroll() {
        let scrollview = ScrollView::new(ScrollDirection::Vertical)
            .size(200.0, 300.0)
            .scroll_to(0.0, 50.0);

        let mut tree = WidgetTree::new();
        let root_id = tree.set_root(Box::new(scrollview));
        let _child_id = tree.add_child(root_id, Box::new(FixedWidget {
            size: Size::new(200.0, 600.0), id: 1,
        }));

        tree.layout();

        let frame = tree.get(root_id).map(|n| n.frame()).unwrap_or(Rect::zero());
        let children = tree.get(root_id).map(|n| n.children().to_vec()).unwrap_or_default();

        let result = tree.get(root_id).unwrap().inner().layout_children(frame, &children, &tree);
        if let Some((_, rect)) = result.first() {
            // Child should be offset by -scroll_y = -50 from the viewport origin
            assert_eq!(rect.x, frame.x);
            assert_eq!(rect.y, frame.y - 50.0);
        } else {
            panic!("Expected at least one child rect");
        }
    }

    #[test]
    fn scrollview_direction_flags() {
        assert!(ScrollDirection::Vertical.can_scroll_y());
        assert!(!ScrollDirection::Vertical.can_scroll_x());
        assert!(ScrollDirection::Horizontal.can_scroll_x());
        assert!(!ScrollDirection::Horizontal.can_scroll_y());
        assert!(ScrollDirection::Both.can_scroll_x());
        assert!(ScrollDirection::Both.can_scroll_y());
    }

    #[test]
    fn scrollview_not_handled_for_non_scroll_events() {
        let mut sv = ScrollView::new(ScrollDirection::Vertical);
        let result = sv.on_event(&WidgetEvent::MouseDown {
            pos: Point::new(10.0, 10.0),
            button: crate::ui::widget::MouseButton::Left,
        });
        assert_eq!(result, EventResult::NotHandled);
    }
}

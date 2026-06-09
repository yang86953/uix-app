//! ScrollView widget — a scrollable viewport that clips and scrolls its children.
//!
//! Supports vertical and horizontal scrolling via mouse wheel, with optional
//! scrollbar rendering. Content offsets are managed per-frame via scroll_x/scroll_y.

use std::cell::Cell;

use crate::define_widget;
use crate::graphics::{Radius, Rect, Size};
use crate::ui::children::WidgetChildren;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, Widget, WidgetEvent, WidgetId, WidgetTree};

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
        direction: ScrollDirection,
        show_scrollbar: bool,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        content_bounds: Cell<Option<Size>>,
    }

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
                if self.direction.can_scroll_y() && delta.y != 0.0 {
                    let step = 30.0; // pixels per wheel notch
                    self.scroll_y = (self.scroll_y - delta.y * step)
                        .max(0.0)
                        .min(self.max_scroll_y());
                    handled = true;
                }
                if self.direction.can_scroll_x() && delta.x != 0.0 {
                    let step = 30.0;
                    self.scroll_x = (self.scroll_x - delta.x * step)
                        .max(0.0)
                        .min(self.max_scroll_x());
                    handled = true;
                }
                if handled { EventResult::Handled } else { EventResult::NotHandled }
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // Clip children to the viewport
        ctx.push_clip_rect(frame);

        // Background fill so the viewport is always opaque
        let bg = ctx.tokens().color_bg_container();
        ctx.fill_rect(frame, bg, None);
    }

    post_render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // Scrollbar overlay drawn AFTER children (on top)
        if self.show_scrollbar {
            self.render_scrollbar(frame, ctx);
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
            let r = Rect::new(origin_x, origin_y, pref.w, pref.h);
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
}

impl ScrollView {
    // ── Constructor ──

    pub fn new(direction: ScrollDirection) -> Self {
        Self {
            children: WidgetChildren::new(),
            scroll_x: 0.0,
            scroll_y: 0.0,
            direction,
            show_scrollbar: true,
            fixed_width: None,
            fixed_height: None,
            content_bounds: Cell::new(None),
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

    /// Show or hide the scrollbar overlay.
    pub fn show_scrollbar(mut self, v: bool) -> Self {
        self.show_scrollbar = v;
        self
    }

    /// Set scroll offset manually (clamped to valid range).
    pub fn scroll_to(mut self, x: f32, y: f32) -> Self {
        self.scroll_x = x.max(0.0);
        self.scroll_y = y.max(0.0);
        self
    }

    // ── Runtime accessors ──

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

    /// Programmatically scroll to a position.
    pub fn scroll_to_xy(&mut self, x: f32, y: f32) {
        self.scroll_x = x.max(0.0);
        self.scroll_y = y.max(0.0);
    }

    // ── Scroll range ──────────────────────────────────────────────────

    /// Maximum scrollable offset along X axis.
    /// Returns `f32::MAX` if content bounds haven't been computed yet
    /// (widget not yet laid out in a tree).
    pub fn max_scroll_x(&self) -> f32 {
        match self.content_bounds.get() {
            Some(cs) => {
                let view_w = self.fixed_width.unwrap_or(300.0);
                (cs.w - view_w).max(0.0)
            }
            None => f32::MAX,
        }
    }

    /// Maximum scrollable offset along Y axis.
    /// Returns `f32::MAX` if content bounds haven't been computed yet.
    pub fn max_scroll_y(&self) -> f32 {
        match self.content_bounds.get() {
            Some(cs) => {
                let view_h = self.fixed_height.unwrap_or(200.0);
                (cs.h - view_h).max(0.0)
            }
            None => f32::MAX,
        }
    }

    // ── Scrollbar rendering ───────────────────────────────────────────

    /// Render scrollbar overlay (track + thumb).
    fn render_scrollbar(&self, frame: Rect, ctx: &mut RenderContext) {
        let sb_w = 6.0;
        let corner = Radius::uniform(3.0);

        // ── Vertical scrollbar ──
        if self.direction.can_scroll_y() {
            let max_y = self.max_scroll_y();
            if max_y > 0.0 {
                let track = Rect::new(
                    frame.x + frame.w - sb_w - 2.0,
                    frame.y + 2.0,
                    sb_w,
                    frame.h - 4.0,
                );
                ctx.fill_rect(track, ctx.tokens().color_fill_tertiary(), Some(corner));

                let thumb_min = 18.0;
                let ratio = frame.h / (frame.h + max_y);
                let thumb_h = (ratio * track.h).max(thumb_min).min(track.h);
                let thumb_y = track.y + (self.scroll_y / max_y) * (track.h - thumb_h);
                let thumb = Rect::new(track.x, thumb_y, sb_w, thumb_h);
                ctx.fill_rect(thumb, ctx.tokens().color_fill_secondary(), Some(corner));
            }
        }

        // ── Horizontal scrollbar ──
        if self.direction.can_scroll_x() {
            let max_x = self.max_scroll_x();
            if max_x > 0.0 {
                let track = Rect::new(
                    frame.x + 2.0,
                    frame.y + frame.h - sb_w - 2.0,
                    frame.w - 4.0,
                    sb_w,
                );
                ctx.fill_rect(track, ctx.tokens().color_fill_tertiary(), Some(corner));

                let thumb_min = 18.0;
                let ratio = frame.w / (frame.w + max_x);
                let thumb_w = (ratio * track.w).max(thumb_min).min(track.w);
                let thumb_x = track.x + (self.scroll_x / max_x) * (track.w - thumb_w);
                let thumb = Rect::new(thumb_x, track.y, thumb_w, sb_w);
                ctx.fill_rect(thumb, ctx.tokens().color_fill_secondary(), Some(corner));
            }
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
    use crate::graphics::Size;
    use crate::ui::widget::{Widget, WidgetId, WidgetTree, WidgetCore};
    use crate::graphics::{Point, Rect};

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
        sv.scroll_y = 60.0; // start scrolled down
        assert_eq!(sv.scroll_y, 60.0);

        // Scroll up (delta.y positive = scroll up in our convention)
        // Formula: scroll_y = (scroll_y - delta.y * step).max(0.0)
        // scroll_y = (60.0 - 1.0 * 30.0).max(0.0) = 30.0
        let result = sv.on_event(&WidgetEvent::MouseWheel {
            delta: Point::new(0.0, 1.0),
        });
        assert_eq!(result, EventResult::Handled);
        assert_eq!(sv.scroll_y, 30.0);
    }

    #[test]
    fn scrollview_mouse_wheel_scrolls_down() {
        let mut sv = ScrollView::new(ScrollDirection::Vertical);
        assert_eq!(sv.scroll_y, 0.0);

        // delta.y negative = scroll down in WidgetEvent convention
        let result = sv.on_event(&WidgetEvent::MouseWheel {
            delta: Point::new(0.0, -1.0),
        });
        assert_eq!(result, EventResult::Handled);
        assert_eq!(sv.scroll_y, 30.0);
    }

    #[test]
    fn scrollview_mouse_wheel_horizontal() {
        let mut sv = ScrollView::new(ScrollDirection::Horizontal);
        assert_eq!(sv.scroll_x, 0.0);

        let result = sv.on_event(&WidgetEvent::MouseWheel {
            delta: Point::new(-1.0, 0.0),
        });
        assert_eq!(result, EventResult::Handled);
        assert_eq!(sv.scroll_x, 30.0);
    }

    #[test]
    fn scrollview_mouse_wheel_both() {
        let mut sv = ScrollView::new(ScrollDirection::Both);

        let result = sv.on_event(&WidgetEvent::MouseWheel {
            delta: Point::new(-1.0, -2.0),
        });
        assert_eq!(result, EventResult::Handled);
        assert_eq!(sv.scroll_x, 30.0);
        assert_eq!(sv.scroll_y, 60.0);
    }

    #[test]
    fn scrollview_scroll_clamped_to_zero() {
        let mut sv = ScrollView::new(ScrollDirection::Vertical);
        sv.scroll_y = 10.0;

        // Scroll up (delta.y = +1, but we use scroll_y -= delta.y * step)
        // scroll_y = (10 - 1 * 30).max(0) = 0
        let result = sv.on_event(&WidgetEvent::MouseWheel {
            delta: Point::new(0.0, 1.0),
        });
        assert_eq!(result, EventResult::Handled);
        assert_eq!(sv.scroll_y, 0.0);
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
        // Children are stored via WidgetChildren
        assert!(sv.children.is_set());
        assert_eq!(sv.children.len(), 1);
        let children = sv.children.take();
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn scrollview_layout_children_offsets_by_scroll() {
        // Create a ScrollView with pre-set scroll offset, then add to tree.
        // We use scroll_to() builder to set the initial scroll offset.
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

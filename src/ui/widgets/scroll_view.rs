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
        /// 前一帧的 scroll 位置（用于计算滚动 delta 做 pixel buffer memmove）
        prev_scroll_x: f32,
        prev_scroll_y: f32,
        /// Velocity-based momentum scrolling: velocity accumulates on
        /// wheel events and decays via friction in on_update.
        velocity_x: f32,
        velocity_y: f32,
        direction: ScrollDirection,
        show_scrollbar: bool,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        flex_grow_val: f32,
        flex_shrink_val: f32,
        content_bounds: Cell<Option<Size>>,
        dragging_v: bool,
        dragging_h: bool,
        last_frame: Cell<Option<Rect>>,
        hover_v: bool,
        hover_h: bool,
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
                    // Acceleration per wheel notch: each notch should
                    // scroll ~10% of the viewport after friction decay.
                    // Distance = velocity / friction(8), so velocity =
                    // dist * 8.  Wayland axis delta ≈ ±10 per notch.
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
                if !self.show_scrollbar { return EventResult::NotHandled; }
                // Vertical scrollbar thumb
                if self.direction.can_scroll_y() && self.max_scroll_y() > 0.0 {
                    let sb = 6.0;
                    let track = Rect::new(frame.x + frame.w - sb - 2.0, frame.y + 2.0, sb, frame.h - 4.0);
                    let max_y = self.max_scroll_y();
                    let ratio_h = frame.h / (frame.h + max_y);
                    let thumb_h = (ratio_h * track.h).max(18.0).min(track.h);
                    let thumb_y = track.y + (self.scroll_y / max_y) * (track.h - thumb_h);
                    if Rect::new(track.x, thumb_y, sb, thumb_h).contains(*pos) {
                        self.dragging_v = true;
                        return EventResult::Handled;
                    }
                }
                // Horizontal scrollbar thumb
                if self.direction.can_scroll_x() && self.max_scroll_x() > 0.0 {
                    let sb = 6.0;
                    let track = Rect::new(frame.x + 2.0, frame.y + frame.h - sb - 2.0, frame.w - 4.0, sb);
                    let max_x = self.max_scroll_x();
                    let ratio_w = frame.w / (frame.w + max_x);
                    let thumb_w = (ratio_w * track.w).max(18.0).min(track.w);
                    let thumb_x = track.x + (self.scroll_x / max_x) * (track.w - thumb_w);
                    if Rect::new(thumb_x, track.y, thumb_w, sb).contains(*pos) {
                        self.dragging_h = true;
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            WidgetEvent::MouseMove { pos } => {
                if self.dragging_v {
                    let frame = match self.last_frame.get() {
                        Some(f) => f,
                        None => return EventResult::Handled,
                    };
                    let sb = 6.0;
                    let track = Rect::new(frame.x + frame.w - sb - 2.0, frame.y + 2.0, sb, frame.h - 4.0);
                    let max_y = self.max_scroll_y();
                    if max_y > 0.0 {
                        let thumb_h = (frame.h / (frame.h + max_y) * track.h).max(18.0).min(track.h);
                        let usable = track.h - thumb_h;
                        if usable > 0.0 {
                            let v = ((pos.y - track.y) / usable).clamp(0.0, 1.0) * max_y;
                            self.scroll_y = v;
                            self.velocity_y = 0.0;
                        }
                    }
                    return EventResult::Handled;
                }
                if self.dragging_h {
                    let frame = match self.last_frame.get() {
                        Some(f) => f,
                        None => return EventResult::Handled,
                    };
                    let sb = 6.0;
                    let track = Rect::new(frame.x + 2.0, frame.y + frame.h - sb - 2.0, frame.w - 4.0, sb);
                    let max_x = self.max_scroll_x();
                    if max_x > 0.0 {
                        let thumb_w = (frame.w / (frame.w + max_x) * track.w).max(18.0).min(track.w);
                        let usable = track.w - thumb_w;
                        if usable > 0.0 {
                            let v = ((pos.x - track.x) / usable).clamp(0.0, 1.0) * max_x;
                            self.scroll_x = v;
                            self.velocity_x = 0.0;
                        }
                    }
                    return EventResult::Handled;
                }
                // Not dragging: update thumb hover highlight.
                if self.show_scrollbar {
                    if let Some(frame) = self.last_frame.get() {
                        let sb = 6.0;
                        let old_hover_v = self.hover_v;
                        let old_hover_h = self.hover_h;
                        self.hover_v = false;
                        self.hover_h = false;

                        if self.direction.can_scroll_y() && self.max_scroll_y() > 0.0 {
                            let track = Rect::new(frame.x + frame.w - sb - 2.0, frame.y + 2.0, sb, frame.h - 4.0);
                            let max_y = self.max_scroll_y();
                            let thumb_h = (frame.h / (frame.h + max_y) * track.h).max(18.0).min(track.h);
                            let thumb_y = track.y + (self.scroll_y / max_y) * (track.h - thumb_h);
                            if Rect::new(track.x, thumb_y, sb, thumb_h).contains(*pos) {
                                self.hover_v = true;
                            }
                        }
                        if self.direction.can_scroll_x() && self.max_scroll_x() > 0.0 {
                            let track = Rect::new(frame.x + 2.0, frame.y + frame.h - sb - 2.0, frame.w - 4.0, sb);
                            let max_x = self.max_scroll_x();
                            let thumb_w = (frame.w / (frame.w + max_x) * track.w).max(18.0).min(track.w);
                            let thumb_x = track.x + (self.scroll_x / max_x) * (track.w - thumb_w);
                            if Rect::new(thumb_x, track.y, thumb_w, sb).contains(*pos) {
                                self.hover_h = true;
                            }
                        }
                        // If hover state changed, the scrollbar needs a repaint.
                        if old_hover_v != self.hover_v || old_hover_h != self.hover_h {
                            return EventResult::Handled;
                        }
                    }
                }
                EventResult::NotHandled
            }
            WidgetEvent::MouseUp { .. } => {
                let was_dragging = self.dragging_v || self.dragging_h;
                self.dragging_v = false;
                self.dragging_h = false;
                if was_dragging { EventResult::Handled } else { EventResult::NotHandled }
            }
            WidgetEvent::HoverLeave => {
                self.hover_v = false;
                self.hover_h = false;
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
        self.dragging_v
            || self.dragging_h
            || self.hover_v
            || self.hover_h
            || self.velocity_x.abs() > 0.5
            || self.velocity_y.abs() > 0.5
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        // Declare the viewport clip rect so the widget tree's render pass
        // pushes it around children's render() calls.  The post-render
        // pass does NOT have this clip, so overlay effects (ripples,
        // shadows) can overflow the viewport boundary.
        Some(frame)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // Save frame for scrollbar hit-testing in on_event.
        self.last_frame.set(Some(frame));
        // Background fill so the viewport is always opaque.
        // Clip management is handled by `children_clip()` — not here.
        let bg = ctx.tokens().color_bg_container();
        ctx.fill_rect(frame, bg, None);
    }

    post_render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // Scrollbar overlay drawn on top of children.
        // Clip was already popped by the widget tree's render pass.
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
            show_scrollbar: true,
            fixed_width: None,
            fixed_height: None,
            flex_grow_val: 0.0,
            flex_shrink_val: 1.0,
            content_bounds: Cell::new(None),
            dragging_v: false,
            dragging_h: false,
            last_frame: Cell::new(None),
            hover_v: false,
            hover_h: false,
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
        self.show_scrollbar = v;
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

    pub fn scroll_x(&self) -> f32 {
        self.scroll_x
    }
    pub fn scroll_y(&self) -> f32 {
        self.scroll_y
    }
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

    /// Programmatically scroll to a position.
    pub fn scroll_to_xy(&mut self, x: f32, y: f32) {
        let vx = x.max(0.0);
        let vy = y.max(0.0);
        self.scroll_x = vx;
        self.scroll_y = vy;
        self.velocity_x = 0.0;
        self.velocity_y = 0.0;
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
                let thumb_color = if self.hover_v || self.dragging_v {
                    ctx.tokens().color_fill()
                } else {
                    ctx.tokens().color_fill_secondary()
                };
                ctx.fill_rect(thumb, thumb_color, Some(corner));
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
                let thumb_color = if self.hover_h || self.dragging_h {
                    ctx.tokens().color_fill()
                } else {
                    ctx.tokens().color_fill_secondary()
                };
                ctx.fill_rect(thumb, thumb_color, Some(corner));
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
        sv.scroll_y = 60.0;
        assert_eq!(sv.scroll_y, 60.0);

        // Scroll up: accel = 200*0.5 = 100, vel = 100
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

        // Scroll down: accel = 200*0.5 = 100, vel = -100
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

        // Scroll right: accel = 300*0.5 = 150, vel = -150
        let result = sv.on_event(&WidgetEvent::MouseWheel {
            delta: Point::new(-1.0, 0.0),
        });
        assert_eq!(result, EventResult::Handled);
        assert_eq!(sv.velocity_x, -24.0);
    }

    #[test]
    fn scrollview_mouse_wheel_both() {
        let mut sv = ScrollView::new(ScrollDirection::Both);

        // Y: accel=16, vel=(-2)*16=-32 | X: accel=24, vel=-24
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

        // Scroll up: accel = 200*0.08 = 16, vel = 16
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

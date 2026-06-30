//! ScrollView widget — a scrollable viewport that clips and scrolls its children.
//!
//! Supports vertical and horizontal scrolling via mouse wheel, with optional
//! scrollbar rendering. Content offsets are managed per-frame via scroll_x/scroll_y.

pub mod scrollbar;
#[allow(unused_imports)]
pub use scrollbar::*;

use std::cell::Cell;

use self::scrollbar::{ScrollBar, ScrollbarOrientation};
use uix_platform::{Rect, Size};
use crate::define_widget;
use crate::children::WidgetChildren;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetCore, WidgetEvent, WidgetComponent, WidgetId, WidgetTree};

/// Scroll direction for a ScrollView.
/// （已统一为 uix_platform::ScrollDirection。）
pub use uix_platform::ScrollDirection;

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
        pub scroll_x: f32,
        pub scroll_y: f32,
        /// 前一帧的 scroll 位置（用于计算滚动 delta 做 pixel buffer memmove）
        pub prev_scroll_x: f32,
        pub prev_scroll_y: f32,
        /// Velocity-based momentum scrolling: velocity accumulates on
        /// wheel events and decays via friction in on_update.
        pub velocity_x: f32,
        pub velocity_y: f32,
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

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(
            self.fixed_width.unwrap_or(300.0),
            self.fixed_height.unwrap_or(200.0),
        )
    }

    build => (&self) -> Vec<Box<dyn WidgetComponent>> {
        self.children.take()
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseWheel { delta, .. } => {
                let mut handled = false;
                let view = self.last_frame.get();
                // 加速度（每滚轮单位增加的像素/秒速度）：
                // 比例因子 0.25 约 = 每次滚轮移动视口 5% 的距离（经阻尼衰减后），
                // 比之前的 0.08(1%) 大幅提升响应感。
                if self.direction.can_scroll_y() && delta.y != 0.0 {
                    let view_h = view.map(|f| f.h)
                        .unwrap_or(self.fixed_height.unwrap_or(200.0));
                    self.velocity_y += delta.y * view_h * 0.25;
                    handled = true;
                }
                if self.direction.can_scroll_x() && delta.x != 0.0 {
                    let view_w = view.map(|f| f.w)
                        .unwrap_or(self.fixed_width.unwrap_or(300.0));
                    self.velocity_x += delta.x * view_w * 0.25;
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

        // 动量物理：速度经指数衰减后积分到位置。
        // 使用 exp(-k⋅dt) 而非线性近似 (1-k⋅dt) 确保帧率无关。
        const DAMPING_K: f32 = 5.0;          // 衰减率（s⁻¹），越小滑动尾越长
        let damp = (-DAMPING_K * dt).exp();
        let threshold = 0.5;                  // 速度低于此值时归零（<1px 不可见）

        self.scroll_x += self.velocity_x * dt;
        self.velocity_x *= damp;
        if self.velocity_x.abs() < threshold { self.velocity_x = 0.0; }

        self.scroll_y += self.velocity_y * dt;
        self.velocity_y *= damp;
        if self.velocity_y.abs() < threshold { self.velocity_y = 0.0; }

        // 边界 clamping：带软停止（velocity 急刹而非硬切）
        if self.scroll_x < 0.0 {
            self.scroll_x = 0.0;
            self.velocity_x = 0.0;
        }
        let max_x = self.max_scroll_x();
        if self.scroll_x > max_x {
            self.scroll_x = max_x;
            self.velocity_x = 0.0;
        }
        if self.scroll_y < 0.0 {
            self.scroll_y = 0.0;
            self.velocity_y = 0.0;
        }
        let max_y = self.max_scroll_y();
        if self.scroll_y > max_y {
            self.scroll_y = max_y;
            self.velocity_y = 0.0;
        }
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
            // 用背景色先清空轨道区域，防止子节点脏时在未清除的半透明 overlay 像素上叠加导致变黑
            let bg = ctx.tokens().color_bg_container();
            if self.scrollbar_v.show && self.direction.can_scroll_y() {
                let tr = self.scrollbar_v.track_rect_abs(frame);
                ctx.fill_rect(tr, bg, None);
                self.scrollbar_v.render(frame, ctx, self.scroll_y, self.max_scroll_y());
            }
            if self.scrollbar_h.show && self.direction.can_scroll_x() {
                let tr = self.scrollbar_h.track_rect_abs(frame);
                ctx.fill_rect(tr, bg, None);
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
        // Children are stacked vertically (沿主轴依次排列) so they don't overlap.
        let mut max_right = frame.x;
        let mut max_bottom = frame.y;
        let origin_x = frame.x - self.scroll_x;
        let origin_y = frame.y - self.scroll_y;
        let mut cursor_y = 0.0f32;
        for &cid in children {
            let pref = tree
                .get(cid)
                .map(|c| c.preferred_size(None))
                .unwrap_or_default();
            // When preferred width is 0 (unspecified), stretch to
            // fill the viewport so the child can use flex/Stretch
            // for its own children.
            let w = if pref.w <= 0.0 { frame.w } else { pref.w };
            // 高度取 preferred_size 和当前实际 frame 高度的较大值，
            // 确保 Phase 2（底部向上扩展）后的尺寸正确反映到 content_bounds。
            let current_h = tree.get(cid).map(|c| c.frame().h).unwrap_or(0.0);
            // 子节点高度优先用 preferred_size 或当前实际高度，
            // 不再强制 ≥ 视口高度(frame.h)，避免与 Phase 4 收缩形成振荡循环。
            // 仅在无任何尺寸信息时回退到视口高度。
            let h = if pref.h > 0.0 {
                pref.h
            } else if current_h > 0.0 {
                current_h
            } else {
                frame.h
            };
            let r = Rect::new(origin_x, origin_y + cursor_y, w, h);
            result.push((cid, r));
            max_right = max_right.max(r.x + r.w);
            max_bottom = max_bottom.max(r.y + r.h);
            cursor_y += h;
        }

        // Store content bounds for scrollbar calculation
        let content_w = (max_right - origin_x).max(frame.w);
        let content_h = (max_bottom - origin_y).max(frame.h);
        self.content_bounds.set(Some(Size::new(content_w, content_h)));
        uix_platform::log::debug_fn(format!("[ScrollView] layout_children: view=({:.0},{:.0}) origin_y={:.0} content=({:.0},{:.0}) scroll=({:.0},{:.0})",
            frame.w, frame.h, origin_y,
            content_w, content_h, self.scroll_x, self.scroll_y,));

        result
    }

    // ── 像素缓冲滚动：dirty_rect 返回新增 strip + 1px 重叠 ──
    // scroll_region 用 dy/dx.round() 做整数偏移，dirty_rect 用同样的 round 算新增区域。
    // +1px 重叠确保 scrolled 内容与新绘制 strip 之间无 1px 间隙（否则会露出透明黑线）。
    // 包含垂直滚动条轨道区域，确保 scroll_region 移动的半透明轨道像素被清空重绘。
    dirty_rect => (&self, frame: Rect) -> Rect {
        let dx = self.scroll_x - self.prev_scroll_x;
        let dy = self.scroll_y - self.prev_scroll_y;
        let int_dy = dy.round();
        let int_dx = dx.round();
        let strip = if int_dy > 0.0 {
            // 向下滚动：新增 strip 在底部 + 1px 向上重叠
            let strip_h = (int_dy + 1.0).min(frame.h);
            Rect::new(frame.x, frame.y + frame.h - strip_h, frame.w, strip_h)
        } else if int_dy < 0.0 {
            // 向上滚动：新增 strip 在顶部 + 1px 向下重叠
            let strip_h = ((-int_dy) + 1.0).min(frame.h);
            Rect::new(frame.x, frame.y, frame.w, strip_h)
        } else if int_dx > 0.0 {
            // 向右滚动：新增 strip 在右侧 + 1px 向左重叠
            let strip_w = (int_dx + 1.0).min(frame.w);
            Rect::new(frame.x + frame.w - strip_w, frame.y, strip_w, frame.h)
        } else if int_dx < 0.0 {
            // 向左滚动：新增 strip 在左侧 + 1px 向右重叠
            let strip_w = ((-int_dx) + 1.0).min(frame.w);
            Rect::new(frame.x, frame.y, strip_w, frame.h)
        } else {
            frame
        };
        // 包含滚动条轨道区域，确保 scroll_region 移动的半透明轨道像素被清空重绘
        let track = Rect::new(
            frame.x + frame.w - 8.0,
            frame.y,
            8.0,
            frame.h,
        );
        strip.union(&track)
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

    pub fn child(self, w: impl WidgetComponent + 'static) -> Self {
        self.children.add(w);
        self
    }

    pub fn children(self, widgets: Vec<Box<dyn WidgetComponent>>) -> Self {
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

    /// Maximum scrollable offset along Y axis.
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
    use uix_platform::{Point, Rect, Size};
    use crate::{AlignItems, FlexDirection};
    use crate::render_context::RenderContext;
    use crate::widget::EventResult;
    use crate::widgets::{Collapse, CollapsePanel, Container, Space};

    /// A simple fixed-size widget for testing.
    struct FixedWidget {
        size: Size,
        #[allow(dead_code)]
        id: WidgetId,
    }

    impl Widget for FixedWidget {
        fn as_any(&self) -> &dyn std::any::Any { self }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
        fn preferred_size(&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
            self.size
        }
        fn render(
            &self,
            _frame: Rect,
            _ctx: &mut crate::render_context::RenderContext,
            _tree: &WidgetTree,
        ) {
        }
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
        let sv = ScrollView::new(ScrollDirection::Vertical).child(FixedWidget {
            size: Size::new(100.0, 200.0),
            id: 0,
        });
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
        let _child_id = tree.add_child(
            root_id,
            Box::new(FixedWidget {
                size: Size::new(200.0, 600.0),
                id: 1,
            }),
        );

        tree.layout();

        let frame = tree.get(root_id).map(|n| n.frame()).unwrap_or(Rect::zero());
        let children = tree
            .get(root_id)
            .map(|n| n.children().to_vec())
            .unwrap_or_default();

        let result = tree
            .get(root_id)
            .unwrap()
            .component()
            .layout_children(frame, &children, &tree);
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
            button: crate::widget::MouseButton::Left,
            mods: uix_platform::KeyMod::NONE,
        });
        assert_eq!(result, EventResult::NotHandled);
    }

    /// 测试：ScrollView 内子节点 expand 后 content_bounds/max_scroll 更新
    #[test]
    fn scrollview_expand_child_updates_content_bounds() {
        struct GrowWidget {
            size: std::cell::Cell<f32>,
        }
        impl Widget for GrowWidget {
            fn as_any(&self) -> &dyn std::any::Any { self }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
            fn preferred_size(&self, _: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
                Size::new(300.0, self.size.get())
            }
            fn on_event(&mut self, event: &WidgetEvent) -> EventResult {
                if matches!(event, WidgetEvent::MouseDown { .. }) {
                    self.size.set(self.size.get() * 2.0);
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            fn render(&self, _: Rect, _: &mut RenderContext, _: &WidgetTree) {}
        }

        let mut tree = WidgetTree::new();
        let sv_id = tree.set_root(Box::new(
            ScrollView::new(ScrollDirection::Vertical).size(300.0, 200.0),
        ));
        let child_id = tree.add_child(sv_id, Box::new(GrowWidget { size: std::cell::Cell::new(100.0) }));

        // 初始布局：子节点100 < 视口200
        tree.layout();
        let max_y = |tree: &WidgetTree| -> f32 {
            let sv = tree.get(sv_id).unwrap();
            let sv_ref: &ScrollView = sv.component().as_any().downcast_ref().unwrap();
            sv_ref.max_scroll_y()
        };

        assert_eq!(max_y(&tree), 0.0, "初始内容<视口");

        // 展开1：100→200, 刚好等于视口
        tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(50.0, 10.0),
            button: crate::widget::MouseButton::Left,
            mods: uix_platform::KeyMod::NONE,
        });
        tree.layout();
        assert_eq!(max_y(&tree), 0.0, "展开到200=视口200");

        // 展开2：200→400, 内容>视口
        tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(50.0, 10.0),
            button: crate::widget::MouseButton::Left,
            mods: uix_platform::KeyMod::NONE,
        });
        tree.layout();
        let max = max_y(&tree);
        assert!(
            (max - 200.0).abs() < 1.0,
            "展开到400>视口200，max_scroll_y应为200, 实际{max}"
        );
    }

    /// 测试：Collapse 展开后 ScrollView 的 max_scroll_y 正确更新
    #[test]
    fn collapse_expand_updates_scrollview_content_bounds() {
        use crate::widget::WidgetTree;
        // 构造与 wrap_page 类似的结构：
        // ScrollView(300x200) -> Container(300x0,Column) -> Space(300x140,Column,Stretch) -> Collapse
        // Container 需要用 WidgetNode / tree.add_child 添加子节点
        let mut tree = WidgetTree::new();
        let sv_id = tree.set_root(Box::new(
            ScrollView::new(ScrollDirection::Vertical).size(300.0, 200.0),
        ));
        let container_id = tree.add_child(sv_id, Box::new(
            Container::new().size(300.0, 0.0).dir(FlexDirection::Column),
        ));
        let space_id = tree.add_child(container_id, Box::new(
            Space::new()
                .width(300.0)
                .height(140.0)
                .direction(FlexDirection::Column)
                .align(AlignItems::Stretch),
        ));
        // 面板B 内容足够长（7行），展开后总高度 > 200px 视口
        // 3个 header (36px) + 展开内容 (7*18+16=142) = 250 > 200
        let long_content = "行1\n行2\n行3\n行4\n行5\n行6\n行7";
        tree.add_child(space_id, Box::new(
            Collapse::new().panels(vec![
                CollapsePanel::new("面板A", "面板A短内容。"),
                CollapsePanel::new("面板B", long_content),
                CollapsePanel::new("面板C", "面板C短内容。"),
            ]),
        ));

        tree.layout();
        let max_before = tree.get(sv_id)
            .and_then(|n| n.component().as_any().downcast_ref::<ScrollView>().map(|sv| sv.max_scroll_y()))
            .unwrap();
        assert_eq!(max_before, 0.0, "面板未展开时内容应不超过视口");
        println!("Before click max_scroll_y: {max_before}");

        // 点击展开第二个面板
        // Collapse 每个 header 36px, 点击 y=45 应在第二个面板 header 区域
        tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(50.0, 45.0), // 面板B的header区域
            button: crate::widget::MouseButton::Left,
            mods: uix_platform::KeyMod::NONE,
        });
        tree.layout();
        let max_after = tree
            .get(sv_id)
            .and_then(|n| n.component().as_any().downcast_ref::<ScrollView>().map(|sv| sv.max_scroll_y()))
            .unwrap();
        println!("After click max_scroll_y: {max_after}");

        // 展开后 max_scroll_y 应增大
        assert!(
            max_after > max_before,
            "面板展开后 max_scroll_y 应增大 (before={max_before}, after={max_after})"
        );
    }
}

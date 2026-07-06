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
#[cfg(test)]
use crate::ui::traits::{EventHandler, WidgetCapabilities, WidgetLayout, WidgetRender};
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
mod tests {
    use super::*;
    use crate::core::{Point, Rect, Size};
    use crate::draw::painting::PaintContext;
    use crate::ui::layout::{AlignItems, FlexDirection};
    use crate::ui::widgets::{Collapse, CollapsePanel, Container, Space};
    use crate::ui::EventResult;

    struct FixedWidget {
        size: Size,
        #[allow(dead_code)]
        id: WidgetId,
    }

    impl WidgetComponent for FixedWidget {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn capabilities(&self) -> WidgetCapabilities {
            WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER)
        }

        crate::wc_upcast!(FixedWidget; WidgetLayout);
        crate::wc_upcast!(FixedWidget; WidgetRender);
    }

    impl WidgetLayout for FixedWidget {
        fn preferred_size(
            &self,
            _engine: Option<&dyn crate::draw::traits::GraphicsEngine>,
        ) -> Size {
            self.size
        }
    }

    impl WidgetRender for FixedWidget {
        fn render(&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}
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
    fn scrollview_layout_children_uses_natural_coordinates() {
        let scrollview = ScrollView::new(ScrollDirection::Vertical)
            .size(200.0, 300.0)
            .scroll_to(0.0, 50.0);

        let mut tree = WidgetTree::new();
        let root_id = tree.set_root(Box::new(scrollview));
        tree.add_child(
            root_id,
            Box::new(FixedWidget {
                size: Size::new(200.0, 600.0),
                id: 1,
            }),
        );

        tree.layout();

        let frame = tree.get(root_id).map(|n| n.frame()).unwrap_or_default();
        let children = tree
            .get(root_id)
            .map(|n| n.children().to_vec())
            .unwrap_or_default();
        let result = tree
            .get(root_id)
            .unwrap()
            .layout_children(frame, &children, &tree);

        let (_, rect) = result.first().expect("expected child rect");
        assert_eq!(rect.x, frame.x);
        assert_eq!(rect.y, frame.y);
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
    fn scrollview_not_handled_for_non_scrollbar_pointer_down() {
        let mut sv = ScrollView::new(ScrollDirection::Vertical);
        let result = sv.on_event(&SystemEvent::PointerDown {
            pos: Point::new(10.0, 10.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        });
        assert_eq!(result, EventResult::NotHandled);
    }

    #[test]
    fn scrollview_expand_child_updates_content_bounds() {
        struct GrowWidget {
            size: std::cell::Cell<f32>,
        }

        impl WidgetComponent for GrowWidget {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }

            fn capabilities(&self) -> WidgetCapabilities {
                WidgetCapabilities::from_bits(
                    WidgetCapabilities::LAYOUT
                        | WidgetCapabilities::RENDER
                        | WidgetCapabilities::EVENT,
                )
            }

            crate::wc_upcast!(GrowWidget; WidgetLayout);
            crate::wc_upcast!(GrowWidget; WidgetRender);
            crate::wc_upcast!(GrowWidget; EventHandler);
        }

        impl WidgetLayout for GrowWidget {
            fn preferred_size(&self, _: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
                Size::new(300.0, self.size.get())
            }
        }

        impl WidgetRender for GrowWidget {
            fn render(&self, _: Rect, _: &mut PaintContext, _: &WidgetTree) {}
        }

        impl EventHandler for GrowWidget {
            fn on_event(&mut self, event: &SystemEvent) -> EventResult {
                if matches!(event, SystemEvent::PointerDown { .. }) {
                    self.size.set(self.size.get() * 2.0);
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
        }

        let mut tree = WidgetTree::new();
        let sv_id = tree.set_root(Box::new(
            ScrollView::new(ScrollDirection::Vertical).size(300.0, 200.0),
        ));
        tree.add_child(
            sv_id,
            Box::new(GrowWidget {
                size: std::cell::Cell::new(100.0),
            }),
        );

        tree.layout();
        let max_y = |tree: &WidgetTree| -> f32 {
            let sv = tree.get(sv_id).unwrap();
            let sv_ref: &ScrollView = sv.component().as_any().downcast_ref().unwrap();
            sv_ref.max_scroll_y()
        };
        assert_eq!(max_y(&tree), 0.0);

        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(50.0, 10.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        });
        tree.layout();
        assert_eq!(max_y(&tree), 0.0);

        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(50.0, 10.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        });
        tree.layout();
        let max = max_y(&tree);
        assert!(
            (max - 200.0).abs() < 1.0,
            "expected max_scroll_y near 200, got {max}"
        );
    }

    #[test]
    fn collapse_expand_updates_scrollview_content_bounds() {
        let mut tree = WidgetTree::new();
        let sv_id = tree.set_root(Box::new(
            ScrollView::new(ScrollDirection::Vertical).size(300.0, 200.0),
        ));
        let container_id = tree.add_child(
            sv_id,
            Box::new(Container::new().size(300.0, 0.0).dir(FlexDirection::Column)),
        );
        let space_id = tree.add_child(
            container_id,
            Box::new(
                Space::new()
                    .width(300.0)
                    .height(140.0)
                    .direction(FlexDirection::Column)
                    .align(AlignItems::Stretch),
            ),
        );
        let long_content = "line\nline\nline\nline\nline\nline\nline";
        tree.add_child(
            space_id,
            Box::new(Collapse::new().panels(vec![
                CollapsePanel::new("Panel A", "short"),
                CollapsePanel::new("Panel B", long_content),
                CollapsePanel::new("Panel C", "short"),
            ])),
        );

        tree.layout();
        let max_before = tree
            .get(sv_id)
            .and_then(|n| {
                n.component()
                    .as_any()
                    .downcast_ref::<ScrollView>()
                    .map(|sv| sv.max_scroll_y())
            })
            .unwrap();
        assert_eq!(max_before, 0.0);

        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(50.0, 45.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        });
        tree.layout();
        let max_after = tree
            .get(sv_id)
            .and_then(|n| {
                n.component()
                    .as_any()
                    .downcast_ref::<ScrollView>()
                    .map(|sv| sv.max_scroll_y())
            })
            .unwrap();

        assert!(
            max_after > max_before,
            "expected max_scroll_y to increase after expand, before={max_before}, after={max_after}"
        );
    }
}

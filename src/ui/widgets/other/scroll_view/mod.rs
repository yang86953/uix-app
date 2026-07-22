//! ScrollView widget: a scrollable viewport that clips and scrolls children.

use crate::ui::core::widget::WidgetCore;
pub mod scrollbar;
#[allow(unused_imports)]
pub use scrollbar::*;

use std::cell::Cell;

use self::scrollbar::{ScrollBar, ScrollbarOrientation};
use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::{PaintContext, PaintPass};
use crate::ui::children::WidgetChildren;
use crate::ui::layout::engine::{child_from_tree_with_constraints, LayoutChild};
use crate::ui::state::State;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SnapshotFields, SystemEvent, WidgetComponent,
    WidgetTree,
};

pub use crate::native::traits::input::ScrollDirection;

component! {
    /// A scrollable viewport that clips its children.
    pub struct ScrollView {
        pub(crate) children: WidgetChildren,
        pub scroll_x: f32,
        pub scroll_y: f32,
        scroll_binding: Option<State<Point>>,
        direction: ScrollDirection,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        flex_grow_val: f32,
        flex_shrink_val: f32,
        pub(crate) content_bounds: Cell<Option<Size>>,
        pub(crate) scroll_delta_strip: Cell<(f32, f32)>,
        scrollbar_v: ScrollBar,
        scrollbar_h: ScrollBar,
        pub(crate) last_frame: Cell<Option<Rect>>,
    }

    flex_grow => (&self) -> f32 { self.flex_grow_val }

    flex_shrink => (&self) -> f32 { self.flex_shrink_val }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    build => (&self) -> Vec<Box<dyn WidgetComponent>> {
        self.children.take()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::Wheel { delta, .. } => {
                let view = self.last_frame.get();
                let mut dx = 0.0;
                let mut dy = 0.0;

                if self.direction.can_scroll_y() && delta.y != 0.0 {
                    let view_h = view
                        .map(|f| f.h)
                        .unwrap_or(self.fixed_height.unwrap_or(200.0));
                    dy = delta.y * view_h * 0.25;
                }
                if self.direction.can_scroll_x() && delta.x != 0.0 {
                    let view_w = view
                        .map(|f| f.w)
                        .unwrap_or(self.fixed_width.unwrap_or(300.0));
                    dx = delta.x * view_w * 0.25;
                }

                if self.scroll_by(dx, dy) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
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
                    self.scrollbar_v
                        .begin_drag(frame, *pos, self.scroll_y, self.max_scroll_y());
                    return EventResult::Handled;
                }
                if self.direction.can_scroll_x()
                    && self.max_scroll_x() > 0.0
                    && self
                        .scrollbar_h
                        .hit_test_thumb(frame, *pos, self.scroll_x, self.max_scroll_x())
                {
                    self.scrollbar_h
                        .begin_drag(frame, *pos, self.scroll_x, self.max_scroll_x());
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
                        self.write_bound_offset();
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
                        self.write_bound_offset();
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
            SystemEvent::PointerUp {
                button: MouseButton::Left,
                ..
            } => {
                let was_dragging = self.scrollbar_v.dragging || self.scrollbar_h.dragging;
                self.scrollbar_v.end_drag();
                self.scrollbar_h.end_drag();
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
            SystemEvent::KeyDown { key, .. } => {
                let view = self.last_frame.get();
                let view_w = view
                    .map(|f| f.w)
                    .unwrap_or(self.fixed_width.unwrap_or(300.0));
                let view_h = view
                    .map(|f| f.h)
                    .unwrap_or(self.fixed_height.unwrap_or(200.0));
                let line_x = (view_w * 0.1).max(16.0);
                let line_y = (view_h * 0.1).max(16.0);
                let page_y = (view_h * 0.9).max(line_y);

                let (dx, dy) = match key {
                    KeyCode::Down if self.direction.can_scroll_y() => (0.0, line_y),
                    KeyCode::Up if self.direction.can_scroll_y() => (0.0, -line_y),
                    KeyCode::PageDown if self.direction.can_scroll_y() => (0.0, page_y),
                    KeyCode::PageUp if self.direction.can_scroll_y() => (0.0, -page_y),
                    KeyCode::End if self.direction.can_scroll_y() => {
                        (0.0, self.max_scroll_y() - self.scroll_y)
                    }
                    KeyCode::Home if self.direction.can_scroll_y() => (0.0, -self.scroll_y),
                    KeyCode::Right if self.direction.can_scroll_x() => (line_x, 0.0),
                    KeyCode::Left if self.direction.can_scroll_x() => (-line_x, 0.0),
                    _ => return EventResult::NotHandled,
                };

                if self.scroll_by(dx, dy) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            _ => EventResult::NotHandled,
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
        Some((self.scroll_x, self.scroll_y))
    }

    scroll_descendant_by => (&mut self, dx: f32, dy: f32) -> bool {
        self.scroll_by(dx, dy)
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        // 裁剪/命中子项时扣除滚动条 gutter，避免点滑块落到内容子树上。
        let need_v = self.needs_v_scrollbar(frame, &[]);
        let need_h = self.needs_h_scrollbar(frame, &[]);
        Some(self.content_frame(frame, need_v, need_h))
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        frame
    }


    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.capture_bound_offset_dependency();
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));

        match ctx.paint_pass() {
            PaintPass::Content => {
                let bg = ctx.tokens().color_bg_container();
                ctx.fill_rect(frame, bg, None);
            }
            PaintPass::AfterChildren => {
                // 仅在需要滚动时绘制 gutter 内轨道/滑块（与 layout 预留一致）。
                if self.needs_v_scrollbar(frame, &[]) {
                    self.scrollbar_v
                        .render(frame, ctx, self.scroll_y, self.max_scroll_y());
                }
                if self.needs_h_scrollbar(frame, &[]) {
                    self.scrollbar_h
                        .render(frame, ctx, self.scroll_x, self.max_scroll_x());
                }
            }
        }
    }

    measure_children => (&self, frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        // gutter 依据上一轮 content_bounds / max_scroll；首帧无溢出信息时先满宽，
        // layout_children 仍会按子项高度决定是否缩进，收敛循环下一轮即可对齐 measure。
        let need_v = self.needs_v_scrollbar(frame, &[]);
        let need_h = self.needs_h_scrollbar(frame, &[]);
        let constraints = self.child_constraints(frame, need_v, need_h);
        children
            .iter()
            .copied()
            .map(|id| child_from_tree_with_constraints(id, tree, constraints))
            .collect()
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        let mut result = Vec::new();
        if children.is_empty() {
            self.content_bounds.set(Some(Size::new(frame.w, frame.h)));
            return result;
        }

        let need_v = self.needs_v_scrollbar(frame, children);
        let need_h = self.needs_h_scrollbar(frame, children);
        let content = self.content_frame(frame, need_v, need_h);

        let mut max_right = content.x;
        let mut max_bottom = content.y;
        let mut cursor_x = 0.0f32;
        let mut cursor_y = 0.0f32;
        let can_scroll_x = self.direction.can_scroll_x();
        let can_scroll_y = self.direction.can_scroll_y();
        let horizontal_flow = can_scroll_x && !can_scroll_y;
        for child in children {
            let cid = child.id;
            let pref = child.measured_size;
            // 非滚动轴填满 content（已扣除 gutter）；滚动轴保留自然尺寸。
            // Both：自然宽与视口取 max —— 窄于视口时拉满，宽于视口时允许横向滚动。
            let w = if can_scroll_x {
                if pref.w <= 0.0 {
                    content.w
                } else if can_scroll_y && need_h {
                    pref.w.max(content.w)
                } else if can_scroll_y {
                    // Both + only vertical overflow: the vertical gutter reduces
                    // the usable cross axis. Keeping the pre-gutter measured width
                    // here would place content underneath the scrollbar.
                    content.w
                } else {
                    pref.w
                }
            } else {
                content.w
            };
            // Dynamic descendants such as Collapse can measure to zero before the
            // convergence loop has propagated their expanded content. Preserve the
            // previous arranged height as viewport state; measured_size itself stays
            // the exact result of this pass and is never overwritten with the frame.
            let current_h = tree.get(cid).map(|c| c.frame().h).unwrap_or(0.0);
            let h = if can_scroll_y {
                if pref.h > 0.0 {
                    pref.h
                } else if current_h > 0.0 {
                    current_h
                } else {
                    content.h
                }
            } else {
                content.h
            };
            let r = if horizontal_flow {
                Rect::new(content.x + cursor_x, content.y, w, h)
            } else {
                Rect::new(content.x, content.y + cursor_y, w, h)
            };
            result.push((cid, r));
            max_right = max_right.max(r.x + r.w);
            max_bottom = max_bottom.max(r.y + r.h);
            if horizontal_flow {
                cursor_x += w;
            } else {
                cursor_y += h;
            }
        }

        let raw_content_w = (max_right - content.x).max(content.w);
        let raw_content_h = (max_bottom - content.y).max(content.h);
        let content_w = if can_scroll_x {
            raw_content_w
        } else {
            // 非横向滚动：content_bounds 宽记视口宽（含 gutter），max_scroll_x 仍为 0。
            frame.w
        };
        let content_h = if can_scroll_y {
            raw_content_h
        } else {
            frame.h
        };
        self.content_bounds.set(Some(Size::new(content_w, content_h)));
        result
    }
}

impl ScrollView {
    pub(crate) fn scroll_direction(&self) -> ScrollDirection {
        self.direction
    }

    pub(crate) fn explicit_size_locks(&self) -> (bool, bool) {
        (self.fixed_width.is_some(), self.fixed_height.is_some())
    }

    fn intrinsic_size(&self) -> Size {
        // flex_grow 视口：未固定边以 0 为 basis，由父级分得剩余客户区；
        // 否则默认 300×200 会阻止窗口缩小时收缩，内容被窗口裁切且 max_scroll=0。
        let grow = self.flex_grow_val > 0.0;
        Size::new(
            self.fixed_width.unwrap_or(if grow { 0.0 } else { 300.0 }),
            self.fixed_height.unwrap_or(if grow { 0.0 } else { 200.0 }),
        )
    }

    /// 内容排布区域：需要滚动条时从视口扣除 gutter，避免卡片与滑块重叠。
    fn content_frame(&self, frame: Rect, need_v: bool, need_h: bool) -> Rect {
        let mut w = frame.w;
        let mut h = frame.h;
        if need_v {
            w = (w - ScrollBar::gutter()).max(0.0);
        }
        if need_h {
            h = (h - ScrollBar::gutter()).max(0.0);
        }
        Rect::new(frame.x, frame.y, w, h)
    }

    fn needs_v_scrollbar(&self, frame: Rect, children: &[LayoutChild]) -> bool {
        if !(self.scrollbar_v.show && self.direction.can_scroll_y()) {
            return false;
        }
        if self.max_scroll_y() > 0.0 {
            return true;
        }
        if self
            .content_bounds
            .get()
            .is_some_and(|b| b.h > frame.h + 0.5)
        {
            return true;
        }
        let content_h: f32 = children.iter().map(|c| c.measured_size.h.max(0.0)).sum();
        content_h > frame.h + 0.5
    }

    pub(crate) fn needs_h_scrollbar(&self, frame: Rect, children: &[LayoutChild]) -> bool {
        if !(self.scrollbar_h.show && self.direction.can_scroll_x()) {
            return false;
        }
        if self.max_scroll_x() > 0.0 {
            return true;
        }
        if self
            .content_bounds
            .get()
            .is_some_and(|b| b.w > frame.w + 0.5)
        {
            return true;
        }
        let content_w: f32 = children.iter().map(|c| c.measured_size.w.max(0.0)).sum();
        // 仅横向流时用子项宽之和；Both 模式取 max 更稳妥
        let content_w = if self.direction.can_scroll_y() {
            children
                .iter()
                .map(|c| c.measured_size.w.max(0.0))
                .fold(0.0f32, f32::max)
        } else {
            content_w
        };
        content_w > frame.w + 0.5
    }

    fn child_constraints(&self, frame: Rect, need_v: bool, need_h: bool) -> Constraints {
        let content = self.content_frame(frame, need_v, need_h);
        let max_w = if self.direction.can_scroll_x() {
            f32::MAX
        } else {
            content.w
        };
        let max_h = if self.direction.can_scroll_y() {
            f32::MAX
        } else {
            content.h
        };
        Constraints::loose(Size::new(max_w, max_h))
    }

    pub fn new(direction: ScrollDirection) -> Self {
        Self {
            children: WidgetChildren::new(),
            scroll_x: 0.0,
            scroll_y: 0.0,
            scroll_binding: None,
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
        self.scroll_binding = None;
        self.scroll_x = Self::normalize_axis(x);
        self.scroll_y = Self::normalize_axis(y);
        self
    }

    /// 将运行态滚动位置双向绑定到应用 State。
    pub fn scroll_offset(mut self, state: &State<Point>) -> Self {
        let offset = state.get();
        self.scroll_x = Self::normalize_axis(offset.x);
        self.scroll_y = Self::normalize_axis(offset.y);
        self.scroll_binding = Some(state.clone());
        self
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::ScrollView {
            direction: self.direction,
            fixed_width: self.fixed_width,
            fixed_height: self.fixed_height,
            flex_grow: self.flex_grow_val,
            flex_shrink: self.flex_shrink_val,
            show_scrollbar: self.scrollbar_v.show || self.scrollbar_h.show,
            scroll_x: self.scroll_x,
            scroll_y: self.scroll_y,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_offset = next
            .scroll_binding
            .as_ref()
            .map(|_| (next.scroll_x, next.scroll_y));
        self.scroll_binding = next.scroll_binding;
        self.direction = next.direction;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
        self.flex_grow_val = next.flex_grow_val;
        self.flex_shrink_val = next.flex_shrink_val;
        self.scrollbar_v.show = next.scrollbar_v.show;
        self.scrollbar_h.show = next.scrollbar_h.show;
        if let Some((scroll_x, scroll_y)) = controlled_offset {
            self.scroll_x = self.clamp_bound_axis(scroll_x, true);
            self.scroll_y = self.clamp_bound_axis(scroll_y, false);
        }
    }

    pub fn scroll_x(&self) -> f32 {
        self.scroll_x
    }

    pub fn scroll_y(&self) -> f32 {
        self.scroll_y
    }

    pub fn set_scroll_x(&mut self, x: f32) {
        let old_x = self.scroll_x;
        self.scroll_x = Self::normalize_axis(x);
        self.push_scroll_delta(self.scroll_x - old_x, 0.0);
        self.write_bound_offset();
    }

    pub fn set_scroll_y(&mut self, y: f32) {
        let old_y = self.scroll_y;
        self.scroll_y = Self::normalize_axis(y);
        self.push_scroll_delta(0.0, self.scroll_y - old_y);
        self.write_bound_offset();
    }

    pub fn scroll_to_xy(&mut self, x: f32, y: f32) {
        let old_x = self.scroll_x;
        let old_y = self.scroll_y;
        self.scroll_x = Self::normalize_axis(x).min(self.max_scroll_x());
        self.scroll_y = Self::normalize_axis(y).min(self.max_scroll_y());
        self.push_scroll_delta(self.scroll_x - old_x, self.scroll_y - old_y);
        self.write_bound_offset();
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

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    fn scroll_by(&mut self, dx: f32, dy: f32) -> bool {
        let old_x = self.scroll_x;
        let old_y = self.scroll_y;
        self.scroll_x = (self.scroll_x + dx).clamp(0.0, self.max_scroll_x());
        self.scroll_y = (self.scroll_y + dy).clamp(0.0, self.max_scroll_y());
        let actual_dx = self.scroll_x - old_x;
        let actual_dy = self.scroll_y - old_y;
        self.push_scroll_delta(actual_dx, actual_dy);
        if actual_dx.abs() > 0.01 || actual_dy.abs() > 0.01 {
            self.write_bound_offset();
        }
        actual_dx.abs() > 0.01 || actual_dy.abs() > 0.01
    }

    fn write_bound_offset(&self) {
        let Some(state) = self.scroll_binding.as_ref() else {
            return;
        };
        let offset = Point::new(self.scroll_x, self.scroll_y);
        if state.get() != offset {
            state.set(offset);
        }
    }

    fn capture_bound_offset_dependency(&self) {
        if let Some(state) = self.scroll_binding.as_ref() {
            let _ = state.get();
        }
    }

    fn clamp_bound_axis(&self, value: f32, horizontal: bool) -> f32 {
        let value = Self::normalize_axis(value);
        if self.content_bounds.get().is_some() {
            value.min(if horizontal {
                self.max_scroll_x()
            } else {
                self.max_scroll_y()
            })
        } else {
            value
        }
    }

    fn normalize_axis(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }
}

impl Default for ScrollView {
    fn default() -> Self {
        Self::new(ScrollDirection::Vertical)
    }
}

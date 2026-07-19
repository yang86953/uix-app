//! Carousel widget for switching between child content.

use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::{PaintContext, PaintPass};

use crate::ui::children::WidgetChildren;
use crate::ui::SnapshotFields;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetComponent,
    WidgetTree,
};

const ARROW_HIT_WIDTH: f32 = 30.0;
const DOT_SLOT_WIDTH: f32 = 18.0;
const DOT_HIT_HEIGHT: f32 = 16.0;
const DOT_HEIGHT: f32 = 6.0;
const DOT_BOTTOM_INSET: f32 = 8.0;
const ARROW_ICON_SIZE: f32 = 16.0;

#[derive(Clone, Copy)]
struct DotStrip {
    hit_rect: Rect,
    slot_width: f32,
    dot_y: f32,
    dot_height: f32,
}

component! {
    /// Displays one child at a time with dot and arrow controls.
    pub struct Carousel {
        children: WidgetChildren,
        current: Cell<usize>,
        child_count: Cell<usize>,
        show_dots: bool,
        show_arrows: bool,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        focused: bool,
        pending_change: Cell<Option<usize>>,
        layout_requested: Cell<bool>,
        last_frame: Cell<Option<Rect>>,
    }

    tab_index => (&self) -> i32 { i32::from(self.child_count.get() > 1) }

    measure => (&self, constraints: Constraints) -> Size {
        let intrinsic = self.intrinsic_size();
        constraints.clamp(Size::new(
            self.fixed_width.unwrap_or(intrinsic.w),
            self.fixed_height.unwrap_or(intrinsic.h),
        ))
    }

    flex_grow => (&self) -> f32 {
        if self.fixed_width.is_some() || self.fixed_height.is_some() { 0.0 } else { 1.0 }
    }

    build => (&self) -> Vec<Box<dyn WidgetComponent>> {
        let children = self.children.take();
        if !children.is_empty() {
            self.set_child_count(children.len());
        }
        children
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(frame) = self.last_frame.get() {
                    if let Some(index) = self.dot_index_at(frame, *pos) {
                        self.select(index);
                        return EventResult::Handled;
                    }

                    let count = self.child_count.get();
                    if self.show_arrows && count > 1 && frame.contains(*pos) {
                        let (left, right) = Self::arrow_frames(frame);
                        if pos.x < left.x + left.w {
                            self.select(self.previous_index());
                            return EventResult::Handled;
                        }
                        if pos.x >= right.x {
                            self.select(self.next_index());
                            return EventResult::Handled;
                        }
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. }
                if self.focused && self.child_count.get() > 1 => match key {
                KeyCode::Left | KeyCode::Up => {
                    self.select(self.previous_index());
                    EventResult::Handled
                }
                KeyCode::Right | KeyCode::Down => {
                    self.select(self.next_index());
                    EventResult::Handled
                }
                KeyCode::Home => {
                    self.select(0);
                    EventResult::Handled
                }
                KeyCode::End => {
                    self.select(self.child_count.get() - 1);
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    wants_capture_phase => (&self) -> bool { true }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|index| SemanticEvent::change(id, index.to_string()))
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let frame = Self::normalized_frame(frame);
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        let bg = ctx.tokens().color_bg_container();
        if ctx.paint_pass() == PaintPass::Content {
            if frame.w > 0.0 && frame.h > 0.0 {
                ctx.fill_rect(frame, bg, None);
            }
            return;
        }

        let count = self.child_count.get();
        if count == 0 || frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let idx = self.current.get().min(count - 1);
        let primary = ctx.tokens().color_primary();
        let dot_color = ctx.tokens().color_text_quaternary();
        ctx.push_clip(frame);

        if self.show_arrows && count > 1 {
            let (left, right) = Self::arrow_frames(frame);
            let icon_size = ARROW_ICON_SIZE
                .min(left.w * 0.6)
                .min(frame.h * 0.6);
            if icon_size >= 1.0 {
                crate::ui::widgets::icon::paint_icon_in_frame(
                    ctx,
                    "chevron-left",
                    left,
                    ctx.tokens().color_text(),
                    icon_size,
                );
                crate::ui::widgets::icon::paint_icon_in_frame(
                    ctx,
                    "chevron-right",
                    right,
                    ctx.tokens().color_text(),
                    icon_size,
                );
            }
        }

        if self.show_dots && count > 1 {
            let strip = Self::dot_strip(frame, count);
            for i in 0..count {
                let is_active = i == idx;
                let preferred_width: f32 = if is_active { 16.0 } else { 8.0 };
                let fill_ratio: f32 = if is_active { 0.88 } else { 0.45 };
                let dot_width = preferred_width.min(strip.slot_width * fill_ratio);
                let dot_rect = Rect::new(
                    strip.hit_rect.x
                        + i as f32 * strip.slot_width
                        + (strip.slot_width - dot_width) * 0.5,
                    strip.dot_y,
                    dot_width,
                    strip.dot_height,
                );
                ctx.fill_rect(
                    dot_rect,
                    if is_active { primary } else { dot_color },
                    Some(crate::draw::Radius::uniform(3.0)),
                );
            }
        }
        if self.focused && tree.keyboard_focus_visible() {
            let inset = 0.75_f32.min(frame.w * 0.5).min(frame.h * 0.5);
            let focus_rect = Rect::new(
                frame.x + inset,
                frame.y + inset,
                (frame.w - inset * 2.0).max(0.0),
                (frame.h - inset * 2.0).max(0.0),
            );
            if focus_rect.w > 0.0 && focus_rect.h > 0.0 {
                ctx.stroke_rect(
                    focus_rect,
                    primary,
                    1.5,
                    Some(crate::draw::Radius::uniform(ctx.tokens().border_radius_sm())),
                );
            }
        }
        ctx.pop_clip();
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        let frame = Self::normalized_frame(frame);
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        self.set_child_count(children.len());
        let current = self.current.get();
        children
            .iter()
            .enumerate()
            .map(|(index, child)| {
                let child_frame = if index == current {
                    frame
                } else {
                    Rect::new(frame.x, frame.y, 0.0, 0.0)
                };
                (child.id, child_frame)
            })
            .collect()
    }

    child_visible => (&self, index: usize) -> bool {
        index == self.current.get()
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        Some(frame)
    }
}

impl Default for Carousel {
    fn default() -> Self {
        Self::new()
    }
}

impl Carousel {
    fn intrinsic_size(&self) -> Size {
        Size::new(300.0, 200.0)
    }

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0))
    }

    fn arrow_frames(frame: Rect) -> (Rect, Rect) {
        let width = ARROW_HIT_WIDTH.min(frame.w * 0.5);
        (
            Rect::new(frame.x, frame.y, width, frame.h),
            Rect::new(frame.x + frame.w - width, frame.y, width, frame.h),
        )
    }

    fn dot_strip(frame: Rect, count: usize) -> DotStrip {
        let count_f = count.max(1) as f32;
        let total_width = (count_f * DOT_SLOT_WIDTH).min(frame.w);
        let slot_width = total_width / count_f;
        let dot_height = DOT_HEIGHT.min(frame.h).min((slot_width * 0.75).max(1.0));
        let bottom_inset = DOT_BOTTOM_INSET.min((frame.h - dot_height).max(0.0));
        let dot_y = frame.y + frame.h - bottom_inset - dot_height;
        let hit_height = DOT_HIT_HEIGHT.min(frame.h);
        let hit_y = (dot_y + dot_height * 0.5 - hit_height * 0.5)
            .clamp(frame.y, frame.y + frame.h - hit_height);
        DotStrip {
            hit_rect: Rect::new(
                frame.x + (frame.w - total_width) * 0.5,
                hit_y,
                total_width,
                hit_height,
            ),
            slot_width,
            dot_y,
            dot_height,
        }
    }

    pub fn new() -> Self {
        Self {
            children: WidgetChildren::new(),
            current: Cell::new(0),
            child_count: Cell::new(0),
            show_dots: true,
            show_arrows: true,
            fixed_width: None,
            fixed_height: None,
            focused: false,
            pending_change: Cell::new(None),
            layout_requested: Cell::new(false),
            last_frame: Cell::new(None),
        }
    }

    pub fn show_dots(mut self, v: bool) -> Self {
        self.show_dots = v;
        self
    }

    pub fn show_arrows(mut self, v: bool) -> Self {
        self.show_arrows = v;
        self
    }

    /// Sets the preferred Carousel viewport size.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.fixed_width = Some(width.max(0.0));
        self.fixed_height = Some(height.max(0.0));
        self
    }

    pub fn current_index(&self) -> usize {
        self.current.get()
    }

    pub fn slide_count(&self) -> usize {
        self.child_count.get()
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.show_dots = next.show_dots;
        self.show_arrows = next.show_arrows;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Carousel {
            show_dots: self.show_dots,
            show_arrows: self.show_arrows,
            fixed_width: self.fixed_width,
            fixed_height: self.fixed_height,
            current: self.current.get(),
            slide_count: self.child_count.get(),
        }
    }

    fn set_child_count(&self, count: usize) {
        self.child_count.set(count);
        let current = if count == 0 {
            0
        } else {
            self.current.get().min(count - 1)
        };
        self.current.set(current);
    }

    fn select(&self, index: usize) {
        let count = self.child_count.get();
        if count == 0 {
            return;
        }
        let index = index.min(count - 1);
        if index != self.current.get() {
            self.current.set(index);
            self.pending_change.set(Some(index));
            self.layout_requested.set(true);
        }
    }

    fn previous_index(&self) -> usize {
        let count = self.child_count.get();
        if count == 0 {
            return 0;
        }
        (self.current.get() + count - 1) % count
    }

    fn next_index(&self) -> usize {
        let count = self.child_count.get();
        if count == 0 {
            return 0;
        }
        (self.current.get() + 1) % count
    }

    fn dot_index_at(&self, frame: Rect, pos: Point) -> Option<usize> {
        let count = self.child_count.get();
        if !self.show_dots || count <= 1 || !frame.contains(pos) {
            return None;
        }
        let strip = Self::dot_strip(frame, count);
        if strip.slot_width <= 0.0
            || pos.x < strip.hit_rect.x
            || pos.x >= strip.hit_rect.x + strip.hit_rect.w
            || pos.y < strip.hit_rect.y
            || pos.y >= strip.hit_rect.y + strip.hit_rect.h
        {
            return None;
        }
        let index = ((pos.x - strip.hit_rect.x) / strip.slot_width) as usize;
        (index < count).then_some(index)
    }
}

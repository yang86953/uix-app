//! Carousel widget for switching between child content.

use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::{PaintContext, PaintPass};

use crate::ui::children::WidgetChildren;
use crate::ui::SnapshotFields;
use crate::ui::{
    ComponentId, EventResult, KeyCode, SemanticEvent, SystemEvent, WidgetComponent, WidgetTree,
};

const ARROW_HIT_WIDTH: f32 = 30.0;
const DOT_SLOT_WIDTH: f32 = 18.0;
const DOT_HIT_HEIGHT: f32 = 16.0;

component! {
    /// Displays one child at a time with dot and arrow controls.
    pub struct Carousel {
        children: WidgetChildren,
        current: Cell<usize>,
        child_count: Cell<usize>,
        show_dots: bool,
        show_arrows: bool,
        focused: bool,
        pending_change: Cell<Option<usize>>,
        layout_requested: Cell<bool>,
        last_frame: Cell<Option<Rect>>,
    }

    tab_index => (&self) -> i32 { i32::from(self.child_count.get() > 1) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    flex_grow => (&self) -> f32 { 1.0 }

    build => (&self) -> Vec<Box<dyn WidgetComponent>> {
        let children = self.children.take();
        if !children.is_empty() {
            self.set_child_count(children.len());
        }
        children
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                if let Some(frame) = self.last_frame.get() {
                    if let Some(index) = self.dot_index_at(frame, *pos) {
                        self.select(index);
                        return EventResult::Handled;
                    }

                    let count = self.child_count.get();
                    if self.show_arrows && count > 1 && frame.contains(*pos) {
                        if pos.x - frame.x < ARROW_HIT_WIDTH {
                            self.select(self.previous_index());
                            return EventResult::Handled;
                        }
                        if frame.x + frame.w - pos.x < ARROW_HIT_WIDTH {
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
            SystemEvent::KeyDown { key, .. } if self.child_count.get() > 1 => match key {
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

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|index| SemanticEvent::change(id, index.to_string()))
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        let bg = ctx.tokens().color_bg_container();
        if ctx.paint_pass() == PaintPass::Content {
            ctx.fill_rect(frame, bg, None);
            return;
        }

        let count = self.child_count.get();
        if count == 0 {
            return;
        }
        let idx = self.current.get().min(count - 1);
        let primary = ctx.tokens().color_primary();
        let dot_color = ctx.tokens().color_text_quaternary();

        if self.show_arrows && count > 1 {
            let arrow_y = ctx.visual_center_y(frame, 14.0);
            ctx.draw_text("<", Point::new(frame.x + 10.0, arrow_y), ctx.tokens().color_text(), 14.0);
            ctx.draw_text(">", Point::new(frame.x + frame.w - 22.0, arrow_y), ctx.tokens().color_text(), 14.0);
        }

        if self.show_dots && count > 1 {
            let dot_y = frame.y + frame.h - 14.0;
            let total_dot_w = count as f32 * DOT_SLOT_WIDTH;
            let start_x = frame.x + (frame.w - total_dot_w) * 0.5;

            for i in 0..count {
                let is_active = i == idx;
                let dot_width = if is_active { 16.0 } else { 8.0 };
                let dot_rect = Rect::new(
                    start_x + i as f32 * DOT_SLOT_WIDTH + (DOT_SLOT_WIDTH - dot_width) * 0.5,
                    dot_y,
                    dot_width,
                    6.0,
                );
                ctx.fill_rect(
                    dot_rect,
                    if is_active { primary } else { dot_color },
                    Some(crate::draw::Radius::uniform(3.0)),
                );
            }
        }
        if self.focused {
            ctx.stroke_rect(
                frame,
                primary,
                1.5,
                Some(crate::draw::Radius::uniform(ctx.tokens().border_radius_sm())),
            );
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
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

    pub fn new() -> Self {
        Self {
            children: WidgetChildren::new(),
            current: Cell::new(0),
            child_count: Cell::new(0),
            show_dots: true,
            show_arrows: true,
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

    pub fn current_index(&self) -> usize {
        self.current.get()
    }

    pub fn slide_count(&self) -> usize {
        self.child_count.get()
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.show_dots = next.show_dots;
        self.show_arrows = next.show_arrows;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Carousel {
            show_dots: self.show_dots,
            show_arrows: self.show_arrows,
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
        let total_width = count as f32 * DOT_SLOT_WIDTH;
        let start_x = frame.x + (frame.w - total_width) * 0.5;
        let start_y = frame.y + frame.h - 19.0;
        if pos.x < start_x
            || pos.x >= start_x + total_width
            || pos.y < start_y
            || pos.y >= start_y + DOT_HIT_HEIGHT
        {
            return None;
        }
        let index = ((pos.x - start_x) / DOT_SLOT_WIDTH) as usize;
        (index < count).then_some(index)
    }
}

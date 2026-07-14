//! Carousel widget for switching between child content.

use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;

use crate::ui::children::WidgetChildren;
use crate::ui::SnapshotFields;
use crate::ui::{ComponentId, EventResult, SystemEvent, WidgetComponent, WidgetTree};

component! {
    /// Displays one child at a time with dot and arrow controls.
    pub struct Carousel {
        children: WidgetChildren,
        current: Cell<usize>,
        show_dots: bool,
        show_arrows: bool,
        last_frame: Cell<Option<Rect>>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    flex_grow => (&self) -> f32 { 1.0 }

    build => (&self) -> Vec<Box<dyn WidgetComponent>> {
        self.children.take()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                if let Some(frame) = self.last_frame.get() {
                    let dot_area_y = frame.y + frame.h - 20.0;
                    if pos.y >= dot_area_y && pos.y <= dot_area_y + 12.0 {
                        let count = self.children.len();
                        if count > 0 {
                            let dot_w = frame.w / count as f32;
                            let idx = ((pos.x - frame.x) / dot_w) as usize;
                            if idx < count && idx != self.current.get() {
                                self.current.set(idx);
                            }
                        }
                        return EventResult::Handled;
                    }

                    if self.show_arrows {
                        let arrow_area = 30.0;
                        let count = self.children.len();
                        if count > 0 && pos.x - frame.x < arrow_area {
                            let new_idx = (self.current.get() + count - 1) % count;
                            self.current.set(new_idx);
                            return EventResult::Handled;
                        }
                        if count > 0 && frame.x + frame.w - pos.x < arrow_area {
                            self.current.set((self.current.get() + 1) % count);
                            return EventResult::Handled;
                        }
                    }
                }
                EventResult::NotHandled
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame.set(Some(frame));
        let count = self.children.len();
        if count == 0 {
            return;
        }

        let idx = self.current.get().min(count - 1);
        let bg = ctx.tokens().color_bg_container();
        let primary = ctx.tokens().color_primary();
        let dot_color = ctx.tokens().color_text_quaternary();

        ctx.fill_rect(frame, bg, None);

        if self.show_arrows && count > 1 {
            let arrow_y = ctx.visual_center_y(frame, 14.0);
            ctx.draw_text("<", Point::new(frame.x + 10.0, arrow_y), ctx.tokens().color_text(), 14.0);
            ctx.draw_text(">", Point::new(frame.x + frame.w - 22.0, arrow_y), ctx.tokens().color_text(), 14.0);
        }

        if self.show_dots && count > 1 {
            let dot_y = frame.y + frame.h - 14.0;
            let total_dot_w = count as f32 * 12.0;
            let start_x = frame.x + (frame.w - total_dot_w) * 0.5;

            for i in 0..count {
                let is_active = i == idx;
                let dot_rect = Rect::new(
                    start_x + i as f32 * 12.0,
                    dot_y,
                    if is_active { 16.0 } else { 8.0 },
                    6.0,
                );
                ctx.fill_rect(
                    dot_rect,
                    if is_active { primary } else { dot_color },
                    Some(crate::draw::Radius::uniform(3.0)),
                );
            }
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        children.iter().map(|child| (child.id, frame)).collect()
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
            show_dots: true,
            show_arrows: true,
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

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.show_dots = next.show_dots;
        self.show_arrows = next.show_arrows;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Carousel {
            show_dots: self.show_dots,
            show_arrows: self.show_arrows,
        }
    }
}

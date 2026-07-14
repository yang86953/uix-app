//! Floating message container.

use std::cell::RefCell;
use std::rc::Rc;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::native::traits::system::StatusLevel;
use crate::ui::core::widget::WidgetTree;
use crate::ui::{Placement, SnapshotFields};

#[derive(Debug, Clone)]
pub struct MessageItem {
    pub type_: StatusLevel,
    pub content: String,
    pub duration_ms: u64,
    pub closable: bool,
}

component! {
    /// Global floating message container.
    pub struct Message {
        queue: Rc<RefCell<Vec<MessageItem>>>,
        placement: Placement,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let queue = self.queue.borrow();
        if queue.is_empty() {
            return;
        }

        let radius = Some(crate::draw::Radius::uniform(ctx.tokens().border_radius_lg()));
        let shadow = ctx.tokens().box_shadow();
        let bg = ctx.tokens().color_bg_elevated();
        let text_c = ctx.tokens().color_text();

        for (msg_rect, item) in self.message_rects(frame, &queue) {
            let (icon, accent) = match item.type_ {
                StatusLevel::Success => ("+", ctx.tokens().color_success()),
                StatusLevel::Info => ("i", ctx.tokens().color_info()),
                StatusLevel::Warning => ("!", ctx.tokens().color_warning()),
                StatusLevel::Error => ("x", ctx.tokens().color_error()),
            };
            if shadow.layer_1.2 > 0.0 {
                ctx.draw_box_shadow(
                    msg_rect,
                    shadow.layer_1.2,
                    shadow.layer_1.0,
                    shadow.layer_1.1,
                    shadow.layer_1.3,
                    radius,
                );
            }
            ctx.fill_rect(msg_rect, bg, radius);
            ctx.fill_rect(
                Rect::new(msg_rect.x, msg_rect.y + 4.0, 3.0, 32.0),
                accent,
                Some(crate::draw::Radius::uniform(1.5)),
            );
            let icon_y = ctx.visual_center_y(msg_rect, 14.0);
            ctx.draw_text(
                icon,
                Point::new(msg_rect.x + 14.0, icon_y),
                accent,
                14.0,
            );

            let text_x = msg_rect.x + 36.0;
            let content_y = ctx.visual_center_y(msg_rect, 13.0);
            ctx.draw_text(&item.content, Point::new(text_x, content_y), text_c, 13.0);
            if item.closable {
                let close_y = ctx.visual_center_y(msg_rect, 12.0);
                ctx.draw_text(
                    "x",
                    Point::new(msg_rect.x + msg_rect.w - 22.0, close_y),
                    ctx.tokens().color_text_quaternary(),
                    12.0,
                );
            }
        }
    }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        self.hit_bounds(frame)
            .unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, 0.0))
    }
}

impl Default for Message {
    fn default() -> Self {
        Self::new()
    }
}

impl Message {
    const MSG_DURATION_SUCCESS: u64 = 3000;
    const MSG_DURATION_INFO: u64 = 3000;
    const MSG_DURATION_WARNING: u64 = 4000;
    const MSG_DURATION_ERROR: u64 = 5000;
    const MSG_WIDTH: f32 = 380.0;
    const MSG_HEIGHT: f32 = 40.0;
    const MSG_HEIGHT_PER_ITEM: f32 = 48.0;
    const MSG_INSET: f32 = 12.0;

    pub fn new() -> Self {
        Self {
            queue: Rc::new(RefCell::new(Vec::new())),
            placement: Placement::Top,
        }
    }

    pub fn placement(mut self, p: Placement) -> Self {
        self.placement = p;
        self
    }

    pub fn add(&self, item: MessageItem) {
        self.queue.borrow_mut().push(item);
    }

    pub fn success(&self, content: &str) {
        self.add(MessageItem {
            type_: StatusLevel::Success,
            content: content.into(),
            duration_ms: Self::MSG_DURATION_SUCCESS,
            closable: true,
        });
    }

    pub fn info(&self, content: &str) {
        self.add(MessageItem {
            type_: StatusLevel::Info,
            content: content.into(),
            duration_ms: Self::MSG_DURATION_INFO,
            closable: true,
        });
    }

    pub fn warning(&self, content: &str) {
        self.add(MessageItem {
            type_: StatusLevel::Warning,
            content: content.into(),
            duration_ms: Self::MSG_DURATION_WARNING,
            closable: true,
        });
    }

    pub fn error(&self, content: &str) {
        self.add(MessageItem {
            type_: StatusLevel::Error,
            content: content.into(),
            duration_ms: Self::MSG_DURATION_ERROR,
            closable: true,
        });
    }

    pub fn queue(&self) -> Rc<RefCell<Vec<MessageItem>>> {
        self.queue.clone()
    }

    fn intrinsic_size(&self) -> Size {
        Size::zero()
    }

    fn message_rects<'a>(
        &self,
        frame: Rect,
        queue: &'a [MessageItem],
    ) -> impl Iterator<Item = (Rect, &'a MessageItem)> + 'a {
        let message_width = (frame.w - 40.0).clamp(0.0, Self::MSG_WIDTH);
        let stack_height = if queue.is_empty() {
            0.0
        } else {
            Self::MSG_HEIGHT + (queue.len() - 1) as f32 * Self::MSG_HEIGHT_PER_ITEM
        };
        let start_x = frame.x
            + self
                .placement
                .horizontal_start(frame.w, message_width, Self::MSG_INSET);
        let start_y = frame.y
            + self
                .placement
                .vertical_start(frame.h, stack_height, Self::MSG_INSET);

        queue.iter().enumerate().map(move |(index, item)| {
            let y = start_y + index as f32 * Self::MSG_HEIGHT_PER_ITEM;
            (Rect::new(start_x, y, message_width, Self::MSG_HEIGHT), item)
        })
    }

    pub(crate) fn hit_bounds(&self, frame: Rect) -> Option<Rect> {
        let queue = self.queue.borrow();
        self.message_rects(frame, &queue)
            .map(|(rect, _)| rect)
            .reduce(|acc, rect| acc.union(&rect))
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.placement = next.placement;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Message {
            placement: self.placement,
        }
    }
}

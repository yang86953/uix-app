//! Floating message container.

use std::cell::RefCell;
use std::rc::Rc;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::native::traits::system::StatusLevel;
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessagePlacement {
    Top,
    TopLeft,
    TopRight,
}

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
        placement: MessagePlacement,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let queue = self.queue.borrow();
        if queue.is_empty() {
            return;
        }

        let cw = frame.w;
        let msg_w = (380.0f32).min(cw - 40.0);
        let start_x = frame.x
            + match self.placement {
                MessagePlacement::Top => (cw - msg_w) * 0.5,
                MessagePlacement::TopLeft => 12.0,
                MessagePlacement::TopRight => cw - msg_w - 12.0,
            };
        let mut y = frame.y + 12.0;
        let radius = Some(crate::draw::Radius::uniform(ctx.tokens().border_radius_lg()));
        let shadow = ctx.tokens().box_shadow();
        let bg = ctx.tokens().color_bg_elevated();
        let text_c = ctx.tokens().color_text();

        for item in queue.iter() {
            let (icon, accent) = match item.type_ {
                StatusLevel::Success => ("+", ctx.tokens().color_success()),
                StatusLevel::Info => ("i", ctx.tokens().color_info()),
                StatusLevel::Warning => ("!", ctx.tokens().color_warning()),
                StatusLevel::Error => ("x", ctx.tokens().color_error()),
            };
            let msg_rect = Rect::new(start_x, y, msg_w, 40.0);
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
                Rect::new(start_x, y + 4.0, 3.0, 32.0),
                accent,
                Some(crate::draw::Radius::uniform(1.5)),
            );
            let icon_y = ctx.visual_center_y(msg_rect, 14.0);
            ctx.draw_text(icon, Point::new(start_x + 14.0, icon_y), accent, 14.0);

            let text_x = start_x + 36.0;
            let content_y = ctx.visual_center_y(msg_rect, 13.0);
            ctx.draw_text(&item.content, Point::new(text_x, content_y), text_c, 13.0);
            if item.closable {
                let close_y = ctx.visual_center_y(msg_rect, 12.0);
                ctx.draw_text(
                    "x",
                    Point::new(start_x + msg_w - 22.0, close_y),
                    ctx.tokens().color_text_quaternary(),
                    12.0,
                );
            }
            y += Self::MSG_HEIGHT_PER_ITEM;
        }
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
    const MSG_HEIGHT_PER_ITEM: f32 = 48.0;

    pub fn new() -> Self {
        Self {
            queue: Rc::new(RefCell::new(Vec::new())),
            placement: MessagePlacement::Top,
        }
    }

    pub fn placement(mut self, p: MessagePlacement) -> Self {
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

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.placement = next.placement;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Message {
            placement: self.placement,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_preserves_message_zero_layout_footprint() {
        let measured = Message::new().measure(Constraints::loose(Size::new(200.0, 80.0)));

        assert_eq!(measured, Size::zero());
    }
}

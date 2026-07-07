//! Floating notification container.

use std::cell::RefCell;
use std::rc::Rc;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::native::traits::system::StatusLevel;
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NotifPlacement {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

#[derive(Debug, Clone)]
pub struct NotificationItem {
    pub type_: StatusLevel,
    pub title: String,
    pub description: String,
    pub duration_ms: u64,
    pub closable: bool,
}

component! {
    pub struct Notification {
        queue: Rc<RefCell<Vec<NotificationItem>>>,
        placement: NotifPlacement,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let queue = self.queue.borrow();
        if queue.is_empty() {
            return;
        }

        let notif_w = 384.0;
        let start_x = frame.x
            + match self.placement {
                NotifPlacement::TopRight | NotifPlacement::BottomRight => frame.w - notif_w - 24.0,
                NotifPlacement::TopLeft | NotifPlacement::BottomLeft => 24.0,
            };
        let mut y = frame.y
            + match self.placement {
                NotifPlacement::TopRight | NotifPlacement::TopLeft => 12.0,
                NotifPlacement::BottomRight | NotifPlacement::BottomLeft => frame.h - 12.0,
            };
        let grows_up = matches!(
            self.placement,
            NotifPlacement::BottomRight | NotifPlacement::BottomLeft
        );

        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border_secondary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_lg()));

        for item in queue.iter() {
            let desc_h = if item.description.is_empty() { 0.0 } else { 18.0 };
            let notif_h = 48.0 + desc_h;
            let notif_y = if grows_up { y - notif_h } else { y };
            let (icon, accent) = match item.type_ {
                StatusLevel::Success => ("+", ctx.tokens().color_success()),
                StatusLevel::Info => ("i", ctx.tokens().color_info()),
                StatusLevel::Warning => ("!", ctx.tokens().color_warning()),
                StatusLevel::Error => ("x", ctx.tokens().color_error()),
            };
            let notif_rect = Rect::new(start_x, notif_y, notif_w, notif_h);
            ctx.draw_box_shadow(notif_rect, 8.0, 0.0, 4.0, Color::from_rgba(0, 0, 0, 40), r);
            ctx.fill_rect(notif_rect, bg, r);
            ctx.stroke_rect(notif_rect, border, 1.0, r);
            ctx.fill_rect(
                Rect::new(start_x, notif_y + 6.0, 3.0, notif_h - 12.0),
                accent,
                Some(Radius::uniform(1.5)),
            );

            let icon_y = ctx.visual_center_y(notif_rect, 16.0);
            ctx.draw_text(icon, Point::new(start_x + 16.0, icon_y), accent, 16.0);
            let title_y = ctx.visual_center_y(notif_rect, 14.0);
            ctx.draw_text(&item.title, Point::new(start_x + 42.0, title_y), text, 14.0);
            if !item.description.is_empty() {
                ctx.draw_text(
                    &item.description,
                    Point::new(start_x + 42.0, notif_y + 28.0),
                    text_sec,
                    12.0,
                );
            }
            if item.closable {
                let close_y = ctx.visual_center_y(notif_rect, 12.0);
                ctx.draw_text("x", Point::new(start_x + notif_w - 22.0, close_y), text_sec, 12.0);
            }

            if grows_up {
                y -= notif_h + 12.0;
            } else {
                y += notif_h + 12.0;
            }
        }
    }
}

impl Default for Notification {
    fn default() -> Self {
        Self::new()
    }
}

impl Notification {
    pub fn new() -> Self {
        Self {
            queue: Rc::new(RefCell::new(Vec::new())),
            placement: NotifPlacement::TopRight,
        }
    }

    pub fn placement(mut self, p: NotifPlacement) -> Self {
        self.placement = p;
        self
    }

    pub fn add(&self, item: NotificationItem) {
        self.queue.borrow_mut().push(item);
    }

    pub fn open(&self, title: &str, desc: &str, type_: StatusLevel) {
        self.add(NotificationItem {
            type_,
            title: title.to_string(),
            description: desc.to_string(),
            duration_ms: 4500,
            closable: true,
        });
    }

    pub fn success(&self, title: &str, desc: &str) {
        self.open(title, desc, StatusLevel::Success);
    }

    pub fn info(&self, title: &str, desc: &str) {
        self.open(title, desc, StatusLevel::Info);
    }

    pub fn warning(&self, title: &str, desc: &str) {
        self.open(title, desc, StatusLevel::Warning);
    }

    pub fn error(&self, title: &str, desc: &str) {
        self.open(title, desc, StatusLevel::Error);
    }

    pub fn queue(&self) -> Rc<RefCell<Vec<NotificationItem>>> {
        self.queue.clone()
    }

    fn intrinsic_size(&self) -> Size {
        Size::zero()
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Notification {
            placement: self.placement,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_preserves_notification_zero_layout_footprint() {
        let measured = Notification::new().measure(Constraints::loose(Size::new(200.0, 80.0)));

        assert_eq!(measured, Size::zero());
    }
}

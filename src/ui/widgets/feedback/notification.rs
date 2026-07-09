//! Floating notification container.

use std::cell::RefCell;
use std::rc::Rc;

use crate::component;
use crate::core::error::{Error, Result as CoreResult};
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::native::notification::NotificationService;
use crate::native::notification::ToastEntry;
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

impl NotificationItem {
    pub fn from_toast_entry(toast: &ToastEntry) -> Self {
        Self {
            type_: toast.level,
            title: toast.title.clone(),
            description: toast.message.clone(),
            duration_ms: toast.duration_ms as u64,
            closable: true,
        }
    }
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

        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border_secondary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_lg()));

        for (notif_rect, item) in self.toast_rects(frame, &queue) {
            let (icon, accent) = match item.type_ {
                StatusLevel::Success => ("+", ctx.tokens().color_success()),
                StatusLevel::Info => ("i", ctx.tokens().color_info()),
                StatusLevel::Warning => ("!", ctx.tokens().color_warning()),
                StatusLevel::Error => ("x", ctx.tokens().color_error()),
            };
            ctx.draw_box_shadow(notif_rect, 8.0, 0.0, 4.0, Color::from_rgba(0, 0, 0, 40), r);
            ctx.fill_rect(notif_rect, bg, r);
            ctx.stroke_rect(notif_rect, border, 1.0, r);
            ctx.fill_rect(
                Rect::new(notif_rect.x, notif_rect.y + 6.0, 3.0, notif_rect.h - 12.0),
                accent,
                Some(Radius::uniform(1.5)),
            );

            let icon_y = ctx.visual_center_y(notif_rect, 16.0);
            ctx.draw_text(icon, Point::new(notif_rect.x + 16.0, icon_y), accent, 16.0);
            let title_y = ctx.visual_center_y(notif_rect, 14.0);
            ctx.draw_text(
                &item.title,
                Point::new(notif_rect.x + 42.0, title_y),
                text,
                14.0,
            );
            if !item.description.is_empty() {
                ctx.draw_text(
                    &item.description,
                    Point::new(notif_rect.x + 42.0, notif_rect.y + 28.0),
                    text_sec,
                    12.0,
                );
            }
            if item.closable {
                let close_y = ctx.visual_center_y(notif_rect, 12.0);
                ctx.draw_text(
                    "x",
                    Point::new(notif_rect.x + notif_rect.w - 22.0, close_y),
                    text_sec,
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

    pub fn replace_from_toasts<'a, I>(&self, toasts: I)
    where
        I: IntoIterator<Item = &'a ToastEntry>,
    {
        let mut queue = self.queue.borrow_mut();
        queue.clear();
        queue.extend(
            toasts
                .into_iter()
                .filter(|toast| toast.visible)
                .map(NotificationItem::from_toast_entry),
        );
    }

    pub fn replace_from_service(&self, service: &mut NotificationService) {
        let toasts = service.update();
        self.replace_from_toasts(&toasts);
    }

    pub fn notify_error_from_service(
        &self,
        service: &mut NotificationService,
        error: &Error,
    ) -> Option<u64> {
        let id = service.notify_error(error);
        self.replace_from_service(service);
        id
    }

    pub fn notify_result_error_from_service<T>(
        &self,
        service: &mut NotificationService,
        result: &CoreResult<T>,
    ) -> Option<u64> {
        let id = service.notify_result_error(result);
        self.replace_from_service(service);
        id
    }

    fn intrinsic_size(&self) -> Size {
        Size::zero()
    }

    fn toast_rects<'a>(
        &self,
        frame: Rect,
        queue: &'a [NotificationItem],
    ) -> impl Iterator<Item = (Rect, &'a NotificationItem)> + 'a {
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

        queue.iter().map(move |item| {
            let desc_h = if item.description.is_empty() { 0.0 } else { 18.0 };
            let notif_h = 48.0 + desc_h;
            let notif_y = if grows_up { y - notif_h } else { y };
            let notif_rect = Rect::new(start_x, notif_y, notif_w, notif_h);
            if grows_up {
                y -= notif_h + 12.0;
            } else {
                y += notif_h + 12.0;
            }
            (notif_rect, item)
        })
    }

    fn hit_bounds(&self, frame: Rect) -> Option<Rect> {
        let queue = self.queue.borrow();
        self.toast_rects(frame, &queue)
            .map(|(rect, _)| rect)
            .reduce(|acc, rect| acc.union(&rect))
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.placement = next.placement;
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
    use crate::core::error::{Errc, Error};
    use crate::ui::traits::WidgetLayout;
    use std::time::Instant;

    #[test]
    fn measure_preserves_notification_zero_layout_footprint() {
        let measured = Notification::new().measure(Constraints::loose(Size::new(200.0, 80.0)));

        assert_eq!(measured, Size::zero());
    }

    #[test]
    fn item_from_toast_entry_maps_visible_fields() {
        let toast = ToastEntry {
            id: 7,
            title: "Save failed".to_string(),
            message: "Disk is read-only".to_string(),
            level: StatusLevel::Error,
            duration_ms: 6000,
            visible: true,
            created_at: Instant::now(),
        };

        let item = NotificationItem::from_toast_entry(&toast);

        assert_eq!(item.type_, StatusLevel::Error);
        assert_eq!(item.title, "Save failed");
        assert_eq!(item.description, "Disk is read-only");
        assert_eq!(item.duration_ms, 6000);
        assert!(item.closable);
    }

    #[test]
    fn replace_from_toasts_keeps_only_visible_toasts() {
        let notification = Notification::new();
        notification.info("Old message", "will be replaced");
        let now = Instant::now();
        let toasts = vec![
            ToastEntry {
                id: 1,
                title: "Visible".to_string(),
                message: "shown".to_string(),
                level: StatusLevel::Warning,
                duration_ms: 5000,
                visible: true,
                created_at: now,
            },
            ToastEntry {
                id: 2,
                title: "Hidden".to_string(),
                message: "skipped".to_string(),
                level: StatusLevel::Info,
                duration_ms: 4000,
                visible: false,
                created_at: now,
            },
        ];

        notification.replace_from_toasts(&toasts);
        let queue = notification.queue();
        let queue = queue.borrow();

        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].title, "Visible");
        assert_eq!(queue[0].description, "shown");
        assert_eq!(queue[0].type_, StatusLevel::Warning);
    }

    #[test]
    fn notify_error_from_service_syncs_non_fatal_error_to_queue() {
        let notification = Notification::new();
        let mut service = NotificationService::new();

        let id = notification.notify_error_from_service(
            &mut service,
            &Error::warn(Errc::InvalidState, "cache is stale"),
        );

        assert!(id.is_some());
        let queue = notification.queue();
        let queue = queue.borrow();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].type_, StatusLevel::Warning);
        assert_eq!(queue[0].title, "Warning");
        assert!(queue[0].description.contains("cache is stale"));
    }

    #[test]
    fn notify_result_error_from_service_keeps_ok_and_fatal_silent() {
        let notification = Notification::new();
        let mut service = NotificationService::new();
        let ok: crate::core::Result<u32> = Ok(7);
        let fatal: crate::core::Result<u32> = Err(Error::fatal(Errc::InvalidState, "must abort"));

        assert_eq!(
            notification.notify_result_error_from_service(&mut service, &ok),
            None
        );
        assert_eq!(
            notification.notify_result_error_from_service(&mut service, &fatal),
            None
        );

        assert!(notification.queue().borrow().is_empty());
        assert!(service.visible_toasts().is_empty());
    }

    #[test]
    fn empty_notification_does_not_block_hit_test() {
        let notification = Notification::new();
        let frame = Rect::new(0.0, 0.0, 800.0, 600.0);
        let hit = notification.hit_bounds(frame).unwrap_or_default();
        assert_eq!(hit, Rect::new(0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn visible_notification_hit_bounds_cover_toast_stack() {
        let notification = Notification::new();
        notification.info("Saved", "done");
        let frame = Rect::new(0.0, 0.0, 800.0, 600.0);
        let hit = notification.hit_bounds(frame).expect("toast hit bounds");
        assert!(hit.w > 0.0 && hit.h > 0.0);
        assert!(hit.x >= frame.x);
        assert!(hit.y >= frame.y);
        assert!(hit.x + hit.w <= frame.x + frame.w);
        assert!(hit.y + hit.h <= frame.y + frame.h);
    }
}

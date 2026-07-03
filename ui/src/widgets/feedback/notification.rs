//! Notification widget — 通知提醒框，Ant Design 风格。
//!
//! 与 Message 不同，Notification 从屏幕右上角弹出，更持久的通知展示，
//! 支持标题、描述、类型图标、自动关闭。通过静态队列管理。

use crate::define_widget;
use crate::render_context::RenderContext;
use crate::widget::WidgetTree;
use std::cell::RefCell;
use std::rc::Rc;
use uix_graphics::{Color, Radius};
use uix_platform::{Point, Rect, Size};

/// 通知类型。
/// （已统一为 uix_platform::StatusLevel，保留别名以兼容旧代码。）
pub use uix_platform::StatusLevel as NotificationType;

/// 通知弹出位置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NotifPlacement {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

/// 单条通知数据。
#[derive(Debug, Clone)]
pub struct NotificationItem {
    pub type_: NotificationType,
    pub title: String,
    pub description: String,
    pub duration_ms: u64,
    pub closable: bool,
}

// Notification — 通知提醒框容器。
define_widget! {
    pub struct Notification {
        queue: Rc<RefCell<Vec<NotificationItem>>>,
        remaining: Rc<RefCell<Vec<u64>>>,
        placement: NotifPlacement,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::traits::GraphicsEngine>) -> Size {
        Size::zero()
    }

    on_update => (&mut self, dt: f64) {
        let dt_ms = (dt * 1000.0) as u64;
        let mut i = 0;
        let len = self.queue.borrow().len();
        while i < len {
            let mut remain = self.remaining.borrow_mut();
            let mut queue = self.queue.borrow_mut();
            if i >= queue.len() || i >= remain.len() { break; }
            if queue[i].duration_ms > 0 {
                remain[i] = remain[i].saturating_sub(dt_ms);
                if remain[i] == 0 {
                    queue.remove(i);
                    remain.remove(i);
                    continue;
                }
            }
            i += 1;
        }
    }

    needs_continuous_update => (&self) -> bool {
        !self.queue.borrow().is_empty()
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let queue = self.queue.borrow();
        if queue.is_empty() { return; }
        let notif_w = 384.0;
        let start_x = match self.placement {
            NotifPlacement::TopRight | NotifPlacement::BottomRight => frame.w - notif_w - 24.0,
            NotifPlacement::TopLeft | NotifPlacement::BottomLeft => 24.0,
        };
        let mut y = match self.placement {
            NotifPlacement::TopRight | NotifPlacement::TopLeft => 12.0,
            NotifPlacement::BottomRight | NotifPlacement::BottomLeft => frame.h - 12.0,
        };
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border_secondary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_lg()));

        for item in queue.iter() {
            let desc_h = if item.description.is_empty() { 0.0 } else { 18.0 };
            let notif_h = 48.0 + desc_h;
            let (icon, accent) = match item.type_ {
                NotificationType::Success => ("✓", ctx.tokens().color_success()),
                NotificationType::Info    => ("ℹ", ctx.tokens().color_info()),
                NotificationType::Warning => ("⚠", ctx.tokens().color_warning()),
                NotificationType::Error   => ("✗", ctx.tokens().color_error()),
            };
            let notif_rect = Rect::new(start_x, y, notif_w, notif_h);
            // 阴影
            ctx.draw_box_shadow(notif_rect, 8.0, 0.0, 4.0, Color::from_rgba(0, 0, 0, 40), r);
            ctx.fill_rect(notif_rect, bg, r);
            ctx.stroke_rect(notif_rect, border, 1.0, r);
            // 左侧强调条
            ctx.fill_rect(Rect::new(start_x, y + 6.0, 3.0, notif_h - 12.0), accent, Some(Radius::uniform(1.5)));
            // 图标
            let icon_y = ctx.visual_center_y(notif_rect, 16.0);
            ctx.draw_text(icon, Point::new(start_x + 16.0, icon_y), accent, 16.0);
            // 标题
            let title_y = ctx.visual_center_y(notif_rect, 14.0);
            ctx.draw_text(&item.title, Point::new(start_x + 42.0, title_y), text, 14.0);
            // 描述
            if !item.description.is_empty() {
                ctx.draw_text(&item.description, Point::new(start_x + 42.0, y + 28.0), text_sec, 12.0);
            }
            // 关闭
            if item.closable {
                let close_y = ctx.visual_center_y(notif_rect, 12.0);
                ctx.draw_text("✕", Point::new(start_x + notif_w - 22.0, close_y), text_sec, 12.0);
            }
            y += notif_h + 12.0;
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
            remaining: Rc::new(RefCell::new(Vec::new())),
            placement: NotifPlacement::TopRight,
        }
    }

    pub fn placement(mut self, p: NotifPlacement) -> Self {
        self.placement = p;
        self
    }

    pub fn add(&self, item: NotificationItem) {
        self.queue.borrow_mut().push(item);
        self.remaining.borrow_mut().push(
            self.queue
                .borrow()
                .last()
                .map(|i| i.duration_ms)
                .unwrap_or(4500),
        );
    }

    pub fn open(&self, title: &str, desc: &str, type_: NotificationType) {
        self.add(NotificationItem {
            type_,
            title: title.to_string(),
            description: desc.to_string(),
            duration_ms: 4500,
            closable: true,
        });
    }
    pub fn success(&self, title: &str, desc: &str) {
        self.open(title, desc, NotificationType::Success);
    }
    pub fn info(&self, title: &str, desc: &str) {
        self.open(title, desc, NotificationType::Info);
    }
    pub fn warning(&self, title: &str, desc: &str) {
        self.open(title, desc, NotificationType::Warning);
    }
    pub fn error(&self, title: &str, desc: &str) {
        self.open(title, desc, NotificationType::Error);
    }
    pub fn queue(&self) -> Rc<RefCell<Vec<NotificationItem>>> {
        self.queue.clone()
    }
}

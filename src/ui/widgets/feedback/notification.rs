//! 浮动通知容器。

use std::cell::{Cell, RefCell};

use crate::component;
use crate::core::error::{Error, Result as CoreResult};
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::native::notification::{NotificationService, ToastEntry};
use crate::native::traits::system::StatusLevel;
use crate::ui::animation::AnimationConfig;
use crate::ui::core::widget::WidgetTree;
use crate::ui::{EventResult, MouseButton, Placement, SnapshotFields, SystemEvent};

use super::toast_motion::{ToastMotion, ToastMotionEntry, ToastQueue};

#[derive(Debug, Clone)]
pub struct NotificationItem {
    pub type_: StatusLevel,
    pub title: String,
    pub description: String,
    /// 展示时长；`0` 表示仅由调用方或关闭按钮移除。
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

/// 向已挂载的 [`Notification`] 队列增删通知。
#[derive(Clone)]
pub struct NotificationHandle {
    queue: ToastQueue<NotificationItem>,
}

impl NotificationHandle {
    pub(crate) fn new() -> Self {
        Self {
            queue: ToastQueue::new(),
        }
    }

    /// 添加通知并返回稳定 ID。
    pub fn add(&self, item: NotificationItem) -> u64 {
        let duration_ms = item.duration_ms;
        self.queue.push(item, duration_ms)
    }

    pub fn open(
        &self,
        title: impl Into<String>,
        description: impl Into<String>,
        type_: StatusLevel,
    ) -> u64 {
        self.add(NotificationItem {
            type_,
            title: title.into(),
            description: description.into(),
            duration_ms: Notification::DEFAULT_DURATION_MS,
            closable: true,
        })
    }

    pub fn success(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.open(title, description, StatusLevel::Success)
    }

    pub fn info(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.open(title, description, StatusLevel::Info)
    }

    pub fn warning(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.open(title, description, StatusLevel::Warning)
    }

    pub fn error(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.open(title, description, StatusLevel::Error)
    }

    /// 请求移除由此 handle 添加的通知。
    pub fn dismiss(&self, id: u64) -> bool {
        self.queue.remove_local(id)
    }

    /// 请求移除由 [`NotificationService`] 提供稳定 ID 的通知。
    pub fn dismiss_external(&self, id: u64) -> bool {
        self.queue.remove_external(id)
    }

    pub fn clear(&self) {
        self.queue.clear();
    }

    pub fn items(&self) -> Vec<NotificationItem> {
        self.queue.values()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn replace_from_toasts<'a, I>(&self, toasts: I)
    where
        I: IntoIterator<Item = &'a ToastEntry>,
    {
        self.queue
            .replace_external(
                toasts
                    .into_iter()
                    .filter(|toast| toast.visible)
                    .map(|toast| {
                        (
                            toast.id,
                            NotificationItem::from_toast_entry(toast),
                            u64::from(toast.duration_ms),
                        )
                    }),
            );
    }

    pub(crate) fn push_external(&self, id: u64, item: NotificationItem) {
        let duration_ms = item.duration_ms;
        self.queue.push_external(id, item, duration_ms);
    }

    pub(crate) fn retain_latest(&self, maximum: usize) {
        self.queue.retain_latest(maximum);
    }
}

component! {
    pub struct Notification {
        queue: ToastQueue<NotificationItem>,
        motion: RefCell<ToastMotion<NotificationItem>>,
        placement: Placement,
        enter_animation: Option<AnimationConfig>,
        leave_animation: Option<AnimationConfig>,
        last_frame: Cell<Rect>,
        motion_dirty: Cell<bool>,
        motion_dirty_bounds: Cell<Rect>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_motion();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let frame = self.last_frame.get();
                let clicked = {
                    let motion = self.motion.borrow();
                    let clicked = self.toast_rects(frame, motion.entries()).find_map(
                        |(rect, entry)| {
                            (entry.item().closable && Self::close_rect(rect).contains(*pos))
                                .then_some(entry.key())
                        },
                    );
                    clicked
                };
                if let Some(key) = clicked {
                    self.queue.remove_keys(&[key]);
                    self.sync_motion();
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::Timer { id } => {
                let expired = self
                    .motion
                    .borrow_mut()
                    .fire_timer(*id, self.resolved_leave_animation());
                if expired.is_empty() {
                    EventResult::NotHandled
                } else {
                    self.queue.remove_keys(&expired);
                    EventResult::Handled
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    active_timer => (&self) -> Option<(u64, std::time::Duration)> {
        self.sync_motion();
        self.motion.borrow().active_timer()
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame.set(frame);
        self.sync_motion();
        let motion = self.motion.borrow();
        if motion.is_empty() {
            return;
        }

        let radius = Some(Radius::uniform(ctx.tokens().border_radius_lg()));
        for (base_rect, entry) in self.toast_rects(frame, motion.entries()) {
            let opacity = entry.opacity().clamp(0.0, 1.0);
            let notif_rect = transitioned_rect(base_rect, entry.offset(), entry.scale());
            let item = entry.item();
            let bg = fade_color(ctx.tokens().color_bg_elevated(), opacity);
            let border = fade_color(ctx.tokens().color_border_secondary(), opacity);
            let text = fade_color(ctx.tokens().color_text(), opacity);
            let text_secondary = fade_color(ctx.tokens().color_text_secondary(), opacity);
            let (icon, accent) = match item.type_ {
                StatusLevel::Success => ("+", ctx.tokens().color_success()),
                StatusLevel::Info => ("i", ctx.tokens().color_info()),
                StatusLevel::Warning => ("!", ctx.tokens().color_warning()),
                StatusLevel::Error => ("x", ctx.tokens().color_error()),
            };
            let accent = fade_color(accent, opacity);
            ctx.draw_box_shadow(
                notif_rect,
                8.0,
                0.0,
                4.0,
                Color::from_rgba(0, 0, 0, (40.0 * opacity).round() as u8),
                radius,
            );
            ctx.fill_rect(notif_rect, bg, radius);
            ctx.stroke_rect(notif_rect, border, 1.0, radius);
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
                    text_secondary,
                    12.0,
                );
            }
            if item.closable {
                let close_y = ctx.visual_center_y(notif_rect, 12.0);
                ctx.draw_text(
                    "x",
                    Point::new(notif_rect.x + notif_rect.w - 22.0, close_y),
                    text_secondary,
                    12.0,
                );
            }
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        self.sync_motion();
        self.motion_paint_bounds(frame)
    }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        self.hit_bounds(frame).unwrap_or_else(Rect::zero)
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        self.last_frame.set(frame);
        self.sync_motion();
        let bounds = self.motion_paint_bounds(frame);
        (bounds.w > 0.0 && bounds.h > 0.0).then(|| {
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Notification)
                .bounds(bounds)
                .z_index(1210)
        })
    }

    update_animation => (&mut self, dt: f64) -> bool {
        let frame = self.last_frame.get();
        let before = self.motion_paint_bounds(frame);
        let sync_changed = self.sync_motion();
        let update = self.motion.borrow_mut().update(dt);
        let changed = sync_changed || update.changed;
        self.motion_dirty.set(changed);
        if changed {
            let after = self.motion_paint_bounds(frame);
            self.motion_dirty_bounds.set(union_nonempty(before, after));
        }
        update.active
    }

    dirty_bounds => (&self, _frame: Rect) -> Rect {
        if self.motion_dirty.get() {
            self.motion_dirty_bounds.get()
        } else {
            Rect::zero()
        }
    }
}

impl Default for Notification {
    fn default() -> Self {
        Self::new()
    }
}

impl Notification {
    const DEFAULT_DURATION_MS: u64 = 4500;
    const WIDTH: f32 = 384.0;
    const HORIZONTAL_INSET: f32 = 24.0;
    const VERTICAL_INSET: f32 = 12.0;
    const GAP: f32 = 12.0;
    const SHADOW_MARGIN: f32 = 12.0;

    pub fn new() -> Self {
        Self::from_handle(NotificationHandle::new())
    }

    pub(crate) fn from_handle(handle: NotificationHandle) -> Self {
        Self {
            queue: handle.queue,
            motion: RefCell::new(ToastMotion::default()),
            placement: Placement::TopRight,
            enter_animation: None,
            leave_animation: None,
            last_frame: Cell::new(Rect::zero()),
            motion_dirty: Cell::new(false),
            motion_dirty_bounds: Cell::new(Rect::zero()),
        }
    }

    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// 设置逐项进入动画。
    pub fn enter_animation(mut self, animation: AnimationConfig) -> Self {
        self.enter_animation = Some(animation);
        self
    }

    /// 设置逐项离场动画。
    pub fn leave_animation(mut self, animation: AnimationConfig) -> Self {
        self.leave_animation = Some(animation);
        self
    }

    pub fn handle(&self) -> NotificationHandle {
        NotificationHandle {
            queue: self.queue.clone(),
        }
    }

    pub fn add(&self, item: NotificationItem) -> u64 {
        self.handle().add(item)
    }

    pub fn open(
        &self,
        title: impl Into<String>,
        description: impl Into<String>,
        type_: StatusLevel,
    ) -> u64 {
        self.handle().open(title, description, type_)
    }

    pub fn success(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.handle().success(title, description)
    }

    pub fn info(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.handle().info(title, description)
    }

    pub fn warning(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.handle().warning(title, description)
    }

    pub fn error(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.handle().error(title, description)
    }

    pub fn dismiss(&self, id: u64) -> bool {
        self.handle().dismiss(id)
    }

    pub fn dismiss_external(&self, id: u64) -> bool {
        self.handle().dismiss_external(id)
    }

    pub fn clear(&self) {
        self.handle().clear();
    }

    pub fn items(&self) -> Vec<NotificationItem> {
        self.queue.values()
    }

    pub fn replace_from_toasts<'a, I>(&self, toasts: I)
    where
        I: IntoIterator<Item = &'a ToastEntry>,
    {
        self.handle().replace_from_toasts(toasts);
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

    fn resolved_enter_animation(&self) -> AnimationConfig {
        self.enter_animation
            .unwrap_or_else(|| AnimationConfig::fade_in(0.2))
    }

    fn resolved_leave_animation(&self) -> AnimationConfig {
        self.leave_animation
            .unwrap_or_else(|| AnimationConfig::fade_out(0.15))
    }

    fn sync_motion(&self) -> bool {
        let frame = self.last_frame.get();
        let before = self.motion_paint_bounds(frame);
        let changed = self.motion.borrow_mut().sync(
            &self.queue,
            self.resolved_enter_animation(),
            self.resolved_leave_animation(),
        );
        if changed {
            let after = self.motion_paint_bounds(frame);
            self.motion_dirty.set(true);
            self.motion_dirty_bounds.set(union_nonempty(before, after));
        }
        changed
    }

    fn intrinsic_size(&self) -> Size {
        Size::zero()
    }

    fn toast_rects<'a>(
        &self,
        frame: Rect,
        entries: &'a [ToastMotionEntry<NotificationItem>],
    ) -> impl Iterator<Item = (Rect, &'a ToastMotionEntry<NotificationItem>)> + 'a {
        let width = (frame.w - Self::HORIZONTAL_INSET * 2.0).clamp(0.0, Self::WIDTH);
        let stack_height = Self::stack_height(entries);
        let start_x = frame.x
            + self
                .placement
                .horizontal_start(frame.w, width, Self::HORIZONTAL_INSET);
        let mut y = frame.y
            + self
                .placement
                .vertical_start(frame.h, stack_height, Self::VERTICAL_INSET);

        entries.iter().map(move |entry| {
            let height = Self::toast_height(entry.item());
            let rect = Rect::new(start_x, y, width, height);
            y += height + Self::GAP;
            (rect, entry)
        })
    }

    fn stack_height(entries: &[ToastMotionEntry<NotificationItem>]) -> f32 {
        entries
            .iter()
            .map(|entry| Self::toast_height(entry.item()))
            .sum::<f32>()
            + entries.len().saturating_sub(1) as f32 * Self::GAP
    }

    fn toast_height(item: &NotificationItem) -> f32 {
        if item.description.is_empty() {
            48.0
        } else {
            66.0
        }
    }

    fn motion_paint_bounds(&self, frame: Rect) -> Rect {
        let motion = self.motion.borrow();
        let mut bounds: Option<Rect> = None;
        for (rect, entry) in self.toast_rects(frame, motion.entries()) {
            let (from, to) = entry.offset_endpoints();
            let sweep = rect
                .union(&translated_rect(rect, from))
                .union(&translated_rect(rect, to));
            bounds = Some(bounds.map_or(sweep, |current| current.union(&sweep)));
        }
        bounds.map_or_else(Rect::zero, |bounds| {
            expand_rect(bounds, Self::SHADOW_MARGIN)
        })
    }

    fn close_rect(rect: Rect) -> Rect {
        Rect::new(rect.x + rect.w - 40.0, rect.y, 40.0, rect.h)
    }

    pub(crate) fn hit_bounds(&self, frame: Rect) -> Option<Rect> {
        self.last_frame.set(frame);
        self.sync_motion();
        let motion = self.motion.borrow();
        self.toast_rects(frame, motion.entries())
            .map(|(rect, entry)| transitioned_rect(rect, entry.offset(), entry.scale()))
            .reduce(|acc, rect| acc.union(&rect))
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.placement = next.placement;
        self.enter_animation = next.enter_animation;
        self.leave_animation = next.leave_animation;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Notification {
            placement: self.placement,
        }
    }
}

fn transitioned_rect(rect: Rect, offset: Point, scale: f32) -> Rect {
    let scale = scale.max(0.0);
    let width = rect.w * scale;
    let height = rect.h * scale;
    Rect::new(
        rect.x + (rect.w - width) * 0.5 + offset.x,
        rect.y + (rect.h - height) * 0.5 + offset.y,
        width,
        height,
    )
}

fn translated_rect(rect: Rect, offset: Point) -> Rect {
    Rect::new(rect.x + offset.x, rect.y + offset.y, rect.w, rect.h)
}

fn expand_rect(rect: Rect, margin: f32) -> Rect {
    Rect::new(
        rect.x - margin,
        rect.y - margin,
        rect.w + margin * 2.0,
        rect.h + margin * 2.0,
    )
}

fn union_nonempty(first: Rect, second: Rect) -> Rect {
    match (
        first.w > 0.0 && first.h > 0.0,
        second.w > 0.0 && second.h > 0.0,
    ) {
        (true, true) => first.union(&second),
        (true, false) => first,
        (false, true) => second,
        (false, false) => Rect::zero(),
    }
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

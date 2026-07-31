//! 浮动通知容器。

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::component;
use crate::core::error::{Error, Result as CoreResult};
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius};
use crate::native::capabilities::system::StatusLevel;
use crate::native::notification::{NotificationService, ToastEntry};
use crate::ui::animation::AnimationConfig;
use crate::ui::core::paint_context::PaintContext;
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
        hovered_close: Cell<Option<super::toast_motion::ToastKey>>,
        pressed_close: Cell<Option<super::toast_motion::ToastKey>>,
        pressed_action: Cell<Option<super::toast_motion::ToastKey>>,
        action_label: Option<String>,
        action_callback: Option<Rc<dyn Fn()>>,
        icon_name: Option<String>,
        close_label: Option<String>,
        offset: Point,
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
                let target = self.close_target_at(*pos);
                self.hovered_close.set(target);
                self.pressed_close.set(target);
                let action_target = if target.is_none() {
                    self.action_target_at(*pos)
                } else {
                    None
                };
                self.pressed_action.set(action_target);
                if target.is_some() || action_target.is_some() {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let armed_close = self.pressed_close.replace(None);
                let armed_action = self.pressed_action.replace(None);
                let target = self.close_target_at(*pos);
                self.hovered_close.set(target);
                if let Some(key) = armed_close {
                    if target == Some(key) {
                        self.queue.remove_keys(&[key]);
                        self.sync_motion();
                    }
                    EventResult::Handled
                } else if let Some(key) = armed_action {
                    if self.action_target_at(*pos) == Some(key) {
                        if let Some(callback) = self.action_callback.as_ref() {
                            callback();
                        }
                    }
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerMove { pos, .. } => {
                let target = self.close_target_at(*pos);
                if self.hovered_close.replace(target) != target {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave | SystemEvent::FocusOut => {
                let changed = self.hovered_close.replace(None).is_some()
                    | self.pressed_close.replace(None).is_some()
                    | self.pressed_action.replace(None).is_some();
                if changed {
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
        let frame = self.remember_frame(frame);
        self.sync_motion();
        let motion = self.motion.borrow();
        if motion.is_empty() || frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }

        let radius = Some(Radius::uniform(ctx.tokens().border_radius_lg()));
        let shadow = ctx.tokens().box_shadow();
        ctx.push_clip(frame);
        for (base_rect, entry) in self.notification_rects(frame, motion.entries()) {
            let opacity = entry.opacity().clamp(0.0, 1.0);
            let notif_rect = transitioned_rect(base_rect, entry.offset(), entry.scale());
            if notif_rect.w <= 0.0 || notif_rect.h <= 0.0 {
                continue;
            }
            let item = entry.item();
            let bg = fade_color(ctx.tokens().color_bg_elevated(), opacity);
            let border = fade_color(ctx.tokens().color_border_secondary(), opacity);
            let text = fade_color(ctx.tokens().color_text(), opacity);
            let text_secondary = fade_color(ctx.tokens().color_text_secondary(), opacity);
            let (default_icon, accent) = match item.type_ {
                StatusLevel::Success => ("check-circle", ctx.tokens().color_success()),
                StatusLevel::Info => ("info", ctx.tokens().color_info()),
                StatusLevel::Warning => ("alert-triangle", ctx.tokens().color_warning()),
                StatusLevel::Error => ("x-circle", ctx.tokens().color_error()),
            };
            let icon = self.icon_name.as_deref().unwrap_or(default_icon);
            let accent = fade_color(accent, opacity);
            if shadow.layer_1.2 > 0.0 {
                ctx.draw_box_shadow(
                    notif_rect,
                    shadow.layer_1.2,
                    shadow.layer_1.0,
                    shadow.layer_1.1,
                    fade_color(shadow.layer_1.3, opacity),
                    radius,
                );
            }
            ctx.fill_rect(notif_rect, bg, radius);
            ctx.stroke_rect(notif_rect, border, 1.0, radius);
            ctx.fill_rect(
                Rect::new(
                    notif_rect.x,
                    notif_rect.y + 6.0_f32.min(notif_rect.h * 0.5),
                    notif_rect.w.min(3.0),
                    (notif_rect.h - 12.0).max(0.0),
                ),
                accent,
                Some(Radius::uniform(1.5)),
            );
            let geometry = self.item_geometry(notif_rect, item);
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                icon,
                geometry.icon,
                accent,
                16.0,
            );
            Self::paint_elided_text(ctx, &item.title, geometry.title, text, 14.0);
            if !item.description.is_empty() {
                Self::paint_elided_text(
                    ctx,
                    &item.description,
                    geometry.description,
                    text_secondary,
                    12.0,
                );
            }
            if let (Some(action), Some(action_rect)) =
                (self.action_label.as_deref(), geometry.action)
            {
                let action_button = Self::inset_rect(action_rect, 4.0);
                if self.pressed_action.get() == Some(entry.key()) {
                    ctx.fill_rect(
                        action_button,
                        fade_color(ctx.tokens().color_fill_secondary(), opacity),
                        Some(Radius::uniform(ctx.tokens().border_radius_sm())),
                    );
                }
                Self::paint_centered_elided_text(
                    ctx,
                    action,
                    action_button,
                    text,
                    Self::ACTION_FONT_SIZE,
                );
            }
            if item.closable {
                let close_button = Self::inset_rect(geometry.close, 6.0);
                if self.pressed_close.get() == Some(entry.key()) {
                    ctx.fill_rect(
                        close_button,
                        fade_color(ctx.tokens().color_fill_secondary(), opacity),
                        Some(Radius::uniform(ctx.tokens().border_radius_sm())),
                    );
                } else if self.hovered_close.get() == Some(entry.key()) {
                    ctx.fill_rect(
                        close_button,
                        fade_color(ctx.tokens().color_fill_tertiary(), opacity),
                        Some(Radius::uniform(ctx.tokens().border_radius_sm())),
                    );
                }
                if let Some(label) = self.close_label.as_deref() {
                    Self::paint_centered_elided_text(
                        ctx,
                        label,
                        close_button,
                        fade_color(ctx.tokens().color_text_quaternary(), opacity),
                        Self::CLOSE_FONT_SIZE,
                    );
                } else {
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        "x",
                        close_button,
                        fade_color(ctx.tokens().color_text_quaternary(), opacity),
                        13.0,
                    );
                }
            }
        }
        ctx.pop_clip();
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        self.sync_motion();
        self.motion_paint_bounds(frame)
    }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        self.hit_bounds(frame).unwrap_or_else(Rect::zero)
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        let frame = self.remember_frame(frame);
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
    const ACTION_FONT_SIZE: f32 = 12.0;
    const CLOSE_FONT_SIZE: f32 = 11.0;
    const ACTION_HORIZONTAL_PADDING: f32 = 16.0;
    const CLOSE_HORIZONTAL_PADDING: f32 = 16.0;
    const ACTION_MIN_WIDTH: f32 = 40.0;
    const ACTION_MAX_WIDTH: f32 = 112.0;
    const CLOSE_MIN_WIDTH: f32 = 40.0;
    const CLOSE_MAX_WIDTH: f32 = 112.0;
    const CONTROL_GAP: f32 = 4.0;
    const CONTENT_TRAILING_GAP: f32 = 8.0;

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
            hovered_close: Cell::new(None),
            pressed_close: Cell::new(None),
            pressed_action: Cell::new(None),
            action_label: None,
            action_callback: None,
            icon_name: None,
            close_label: None,
            offset: Point::new(0.0, 0.0),
        }
    }

    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    pub fn action<F>(mut self, label: impl Into<String>, action: F) -> Self
    where
        F: Fn() + 'static,
    {
        let label = label.into();
        if !label.trim().is_empty() {
            self.action_label = Some(label);
            self.action_callback = Some(Rc::new(action));
        }
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon_name = Some(icon.into());
        self
    }

    pub fn close_text(mut self, text: impl Into<String>) -> Self {
        let text = text.into();
        self.close_label = (!text.trim().is_empty()).then_some(text);
        self
    }

    pub fn offset(mut self, horizontal: f32, vertical: f32) -> Self {
        self.offset = Point::new(finite_or_zero(horizontal), finite_or_zero(vertical));
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
        let mut motion = self.motion.borrow_mut();
        let changed = motion.sync(
            &self.queue,
            self.resolved_enter_animation(),
            self.resolved_leave_animation(),
        );
        if changed {
            let after = self.motion_paint_bounds_for_entries(frame, motion.entries());
            self.motion_dirty.set(true);
            self.motion_dirty_bounds.set(union_nonempty(before, after));
        }
        for interaction in [&self.hovered_close, &self.pressed_close] {
            if interaction.get().is_some_and(|key| {
                !self
                    .notification_rects(frame, motion.entries())
                    .any(|(_, entry)| {
                        entry.key() == key && entry.item().closable && !entry.is_leaving()
                    })
            }) {
                interaction.set(None);
            }
        }
        if self.pressed_action.get().is_some_and(|key| {
            !self
                .notification_rects(frame, motion.entries())
                .any(|(_, entry)| {
                    entry.key() == key && !entry.is_leaving() && self.action_label.is_some()
                })
        }) {
            self.pressed_action.set(None);
        }
        changed
    }

    fn intrinsic_size(&self) -> Size {
        Size::zero()
    }

    fn notification_rects<'a>(
        &self,
        frame: Rect,
        entries: &'a [ToastMotionEntry<NotificationItem>],
    ) -> impl Iterator<Item = (Rect, &'a ToastMotionEntry<NotificationItem>)> + 'a {
        let frame = Self::normalize_frame(frame);
        let width = frame.w.clamp(0.0, Self::WIDTH);
        let visible_start = Self::visible_start(frame.h, entries);
        let visible = &entries[visible_start..];
        let stack_height = Self::stack_height(visible, frame.h);
        let start_x = frame.x
            + self
                .placement
                .horizontal_start(frame.w, width, Self::HORIZONTAL_INSET)
            + self.offset.x;
        let mut y = frame.y
            + self
                .placement
                .vertical_start(frame.h, stack_height, Self::VERTICAL_INSET)
            + self.offset.y;

        visible.iter().map(move |entry| {
            let height = Self::toast_height(entry.item()).min(frame.h);
            let rect = Rect::new(start_x, y, width, height);
            y += height + Self::GAP;
            (rect, entry)
        })
    }

    fn stack_height(entries: &[ToastMotionEntry<NotificationItem>], frame_height: f32) -> f32 {
        if entries.is_empty() || frame_height <= 0.0 {
            return 0.0;
        }
        entries
            .iter()
            .map(|entry| Self::toast_height(entry.item()).min(frame_height))
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
        self.motion_paint_bounds_for_entries(frame, motion.entries())
    }

    fn motion_paint_bounds_for_entries(
        &self,
        frame: Rect,
        entries: &[ToastMotionEntry<NotificationItem>],
    ) -> Rect {
        let frame = Self::normalize_frame(frame);
        let mut bounds: Option<Rect> = None;
        for (rect, entry) in self.notification_rects(frame, entries) {
            let (from, to) = entry.offset_endpoints();
            let sweep = rect
                .union(&translated_rect(rect, from))
                .union(&translated_rect(rect, to));
            bounds = Some(bounds.map_or(sweep, |current| current.union(&sweep)));
        }
        bounds
            .map(|bounds| expand_rect(bounds, Self::SHADOW_MARGIN))
            .and_then(|bounds| bounds.intersect(&frame))
            .unwrap_or_else(Rect::zero)
    }

    fn close_rect(&self, rect: Rect, closable: bool) -> Rect {
        if !closable {
            return Rect::new(rect.x + rect.w, rect.y, 0.0, rect.h);
        }
        let desired = self.close_label.as_deref().map_or(40.0, |label| {
            Self::measured_control_width(
                label,
                Self::CLOSE_FONT_SIZE,
                Self::CLOSE_HORIZONTAL_PADDING,
                Self::CLOSE_MIN_WIDTH,
                Self::CLOSE_MAX_WIDTH,
            )
        });
        let width = desired.min(rect.w.max(0.0));
        Rect::new(rect.x + rect.w - width, rect.y, width, rect.h)
    }

    fn action_rect(&self, rect: Rect, closable: bool) -> Option<Rect> {
        let label = self.action_label.as_deref()?;
        let close = self.close_rect(rect, closable);
        let gap = if close.w > 0.0 {
            Self::CONTROL_GAP.min((close.x - rect.x).max(0.0))
        } else {
            Self::CONTENT_TRAILING_GAP.min(rect.w.max(0.0))
        };
        let end = (close.x - gap).max(rect.x);
        let available = (end - rect.x).max(0.0);
        let width = Self::measured_control_width(
            label,
            Self::ACTION_FONT_SIZE,
            Self::ACTION_HORIZONTAL_PADDING,
            Self::ACTION_MIN_WIDTH,
            Self::ACTION_MAX_WIDTH,
        )
        .min(available);
        (width > 0.0).then(|| Rect::new(end - width, rect.y, width, rect.h))
    }

    pub(crate) fn hit_bounds(&self, frame: Rect) -> Option<Rect> {
        let frame = self.remember_frame(frame);
        self.sync_motion();
        let motion = self.motion.borrow();
        self.notification_rects(frame, motion.entries())
            .map(|(rect, entry)| transitioned_rect(rect, entry.offset(), entry.scale()))
            .reduce(|acc, rect| acc.union(&rect))
            .and_then(|bounds| bounds.intersect(&frame))
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        if self.placement != next.placement
            || self.offset != next.offset
            || self.close_label != next.close_label
        {
            self.hovered_close.set(None);
            self.pressed_close.set(None);
        }
        self.placement = next.placement;
        self.enter_animation = next.enter_animation;
        self.leave_animation = next.leave_animation;
        self.action_label = next.action_label;
        self.action_callback = next.action_callback;
        self.icon_name = next.icon_name;
        self.close_label = next.close_label;
        self.offset = next.offset;
        self.pressed_action.set(None);
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        let items = self.queue.values();
        SnapshotFields::Notification {
            placement: self.placement,
            titles: items.iter().map(|item| item.title.clone()).collect(),
            descriptions: items.iter().map(|item| item.description.clone()).collect(),
        }
    }

    fn visible_start(frame_height: f32, entries: &[ToastMotionEntry<NotificationItem>]) -> usize {
        let frame_height = Self::normalize_dimension(frame_height);
        if entries.is_empty() || frame_height <= 0.0 {
            return entries.len();
        }
        let mut start = entries.len();
        let mut height = 0.0;
        for index in (0..entries.len()).rev() {
            let item_height = Self::toast_height(entries[index].item()).min(frame_height);
            let candidate = if start == entries.len() {
                item_height
            } else {
                item_height + Self::GAP + height
            };
            if candidate > frame_height && start < entries.len() {
                break;
            }
            start = index;
            height = candidate;
        }
        start
    }

    fn remember_frame(&self, frame: Rect) -> Rect {
        let frame = Self::normalize_frame(frame);
        let previous = self.last_frame.replace(frame);
        if previous != Rect::zero() && previous != frame {
            self.hovered_close.set(None);
            self.pressed_close.set(None);
            self.pressed_action.set(None);
        }
        frame
    }

    fn normalize_frame(frame: Rect) -> Rect {
        Rect::new(
            if frame.x.is_finite() { frame.x } else { 0.0 },
            if frame.y.is_finite() { frame.y } else { 0.0 },
            Self::normalize_dimension(frame.w),
            Self::normalize_dimension(frame.h),
        )
    }

    fn normalize_dimension(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }

    fn close_target_at(&self, local_pos: Point) -> Option<super::toast_motion::ToastKey> {
        let frame = self.last_frame.get();
        let pos = Point::new(local_pos.x + frame.x, local_pos.y + frame.y);
        if !frame.contains(pos) {
            return None;
        }
        let motion = self.motion.borrow();
        let target = self
            .notification_rects(frame, motion.entries())
            .find_map(|(rect, entry)| {
                let rect = transitioned_rect(rect, entry.offset(), entry.scale());
                (entry.item().closable
                    && !entry.is_leaving()
                    && self.close_rect(rect, true).contains(pos))
                .then_some(entry.key())
            });
        target
    }

    #[cfg(test)]
    pub(crate) fn interaction_rects_for_test(
        &self,
        frame: Rect,
    ) -> Vec<(Rect, Option<Rect>, Option<Rect>)> {
        let frame = self.remember_frame(frame);
        self.sync_motion();
        let motion = self.motion.borrow();
        self.notification_rects(frame, motion.entries())
            .map(|(rect, entry)| {
                let rect = transitioned_rect(rect, entry.offset(), entry.scale());
                let geometry = self.item_geometry(rect, entry.item());
                (
                    rect,
                    geometry.action,
                    entry.item().closable.then_some(geometry.close),
                )
            })
            .collect()
    }

    fn action_target_at(&self, local_pos: Point) -> Option<super::toast_motion::ToastKey> {
        let frame = self.last_frame.get();
        let pos = Point::new(local_pos.x + frame.x, local_pos.y + frame.y);
        if !frame.contains(pos) {
            return None;
        }
        let motion = self.motion.borrow();
        let target = self
            .notification_rects(frame, motion.entries())
            .find_map(|(rect, entry)| {
                let rect = transitioned_rect(rect, entry.offset(), entry.scale());
                (!entry.is_leaving()
                    && self
                        .action_rect(rect, entry.item().closable)
                        .is_some_and(|action| action.contains(pos)))
                .then_some(entry.key())
            });
        target
    }

    fn item_geometry(&self, rect: Rect, item: &NotificationItem) -> NotificationGeometry {
        let close = self.close_rect(rect, item.closable);
        let action = self.action_rect(rect, item.closable);
        let trailing_start = action.map_or(close.x, |action| action.x);
        let content_end = (trailing_start
            - Self::CONTENT_TRAILING_GAP.min((trailing_start - rect.x).max(0.0)))
        .max(rect.x);
        let icon_x = rect.x + 10.0_f32.min(rect.w);
        let icon_width = (content_end - icon_x).clamp(0.0, 24.0);
        let icon = Rect::new(icon_x, rect.y, icon_width, rect.h);
        let content_x = (icon.x + icon.w + 8.0).min(content_end);
        let content_width = (content_end - content_x).max(0.0);
        let (title, description) = if item.description.is_empty() {
            (
                Rect::new(content_x, rect.y, content_width, rect.h),
                Rect::zero(),
            )
        } else {
            let title_height = rect.h.min(30.0);
            (
                Rect::new(content_x, rect.y + 4.0, content_width, title_height),
                Rect::new(
                    content_x,
                    rect.y + title_height,
                    content_width,
                    (rect.h - title_height).max(0.0),
                ),
            )
        };
        NotificationGeometry {
            icon,
            title,
            description,
            action,
            close,
        }
    }

    fn measured_control_width(
        value: &str,
        font_size: f32,
        horizontal_padding: f32,
        minimum: f32,
        maximum: f32,
    ) -> f32 {
        let value = value.replace(['\r', '\n'], " ");
        let text_width = crate::draw::resources::font::text_backend::estimate_text_metrics(
            &value,
            f32::INFINITY,
            font_size,
        )
        .max_line_width;
        (text_width + horizontal_padding).clamp(minimum, maximum)
    }

    fn inset_rect(frame: Rect, inset: f32) -> Rect {
        let inset_x = inset.min(frame.w * 0.5);
        let inset_y = inset.min(frame.h * 0.5);
        Rect::new(
            frame.x + inset_x,
            frame.y + inset_y,
            (frame.w - inset_x * 2.0).max(0.0),
            (frame.h - inset_y * 2.0).max(0.0),
        )
    }

    fn paint_elided_text(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
    ) {
        let Some(value) = Self::elide_single_line(ctx, value, font_size, frame.w) else {
            return;
        };
        if frame.h <= 0.0 {
            return;
        }
        ctx.push_clip(frame);
        let y = ctx.visual_center_y(frame, font_size);
        ctx.draw_text(&value, Point::new(frame.x, y), color, font_size);
        ctx.pop_clip();
    }

    fn paint_centered_elided_text(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
    ) {
        let Some(value) = Self::elide_single_line(ctx, value, font_size, frame.w) else {
            return;
        };
        if frame.h <= 0.0 {
            return;
        }
        ctx.push_clip(frame);
        ctx.text_center(&value, frame, color, font_size);
        ctx.pop_clip();
    }

    fn elide_single_line(
        ctx: &mut PaintContext,
        value: &str,
        font_size: f32,
        max_width: f32,
    ) -> Option<String> {
        if !max_width.is_finite() || max_width <= 0.0 {
            return None;
        }
        let value = value.replace(['\r', '\n'], " ");
        if Self::text_width(ctx, &value, font_size) <= max_width {
            return Some(value);
        }
        const ELLIPSIS: &str = "…";
        if Self::text_width(ctx, ELLIPSIS, font_size) > max_width {
            return None;
        }
        let mut visible = String::new();
        for ch in value.chars() {
            visible.push(ch);
            visible.push_str(ELLIPSIS);
            let fits = Self::text_width(ctx, &visible, font_size) <= max_width;
            visible.pop();
            if !fits {
                visible.pop();
                break;
            }
        }
        visible.push_str(ELLIPSIS);
        Some(visible)
    }

    fn text_width(ctx: &mut PaintContext, value: &str, font_size: f32) -> f32 {
        ctx.measure_text(value, font_size).w.max(
            crate::draw::resources::font::text_backend::estimate_text_metrics(
                value,
                f32::INFINITY,
                font_size,
            )
            .max_line_width,
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct NotificationGeometry {
    icon: Rect,
    title: Rect,
    description: Rect,
    action: Option<Rect>,
    close: Rect,
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

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

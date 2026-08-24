//! 浮动通知容器。

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::error::{Error, Result as CoreResult};
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::platform::capabilities::StatusLevel;
use crate::platform::services::{NotificationSource, ToastEntry};
use crate::ui::animation::AnimationConfig;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::ui::{EventResult, MouseButton, Placement, SystemEvent};
use crate::widget;

// 引入关闭原因以区分用户关闭与到期关闭。
use super::declaration::FeedbackCloseReason;
use super::toast_motion::{ToastMotion, ToastQueue};

mod item;
mod layout;
mod presentation;

use self::presentation::*;

pub use self::item::{NotificationHandle, NotificationItem};
use self::layout::{fade_color, finite_or_zero, transitioned_rect, union_nonempty};

widget! {
    /// 拥有窗口内通知队列、过渡动画与放置方向的浮层宿主组件。
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
        // 全部实例只保存指向 UIX 唯一视觉静态项的共享引用。
        #[snapshot(skip)]
        visual: &'static NotificationVisual,
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
                        // 用户点击关闭产生 Manual 关闭事实。
                        self.queue
                            .close_keys(&[key], FeedbackCloseReason::Manual);
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
                    // 计时到期产生 Timeout 关闭事实。
                    self.queue
                        .close_keys(&expired, FeedbackCloseReason::Timeout);
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

        // 同帧全部可见通知共享一次主题解析。
        let visual = self.visual;
        let resolved = visual.resolve(ctx.tokens());
        let layout = &visual.layout;
        let radius = Some(Radius::uniform(resolved.radius));
        let shadow = resolved.shadow;
        ctx.push_clip(frame);
        for (base_rect, entry) in self.notification_rects(frame, motion.entries()) {
            let opacity = entry.opacity().clamp(0.0, 1.0);
            let notif_rect = transitioned_rect(base_rect, entry.offset(), entry.scale());
            if notif_rect.w <= 0.0 || notif_rect.h <= 0.0 {
                continue;
            }
            let item = entry.item();
            let bg = fade_color(resolved.background, opacity);
            let border = fade_color(resolved.border, opacity);
            let text = fade_color(resolved.text, opacity);
            let text_secondary = fade_color(resolved.text_secondary, opacity);
            let (default_icon, accent) = visual.status_visual(&resolved, item.type_);
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
            ctx.stroke_rect(
                notif_rect,
                border,
                visual.chrome.panel_stroke,
                radius,
            );
            ctx.fill_rect(
                Rect::new(
                    notif_rect.x,
                    notif_rect.y + layout.accent_vertical_inset.min(notif_rect.h * 0.5),
                    notif_rect.w.min(layout.accent_width),
                    (notif_rect.h - layout.accent_vertical_inset * 2.0).max(0.0),
                ),
                accent,
                Some(Radius::uniform(layout.accent_radius)),
            );
            let geometry = self.item_geometry(notif_rect, item);
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                icon,
                geometry.icon,
                accent,
                visual.typography.status_icon,
            );
            Self::paint_elided_text(
                ctx,
                &item.title,
                geometry.title,
                text,
                resolved.title_font_size,
            );
            if !item.description.is_empty() {
                Self::paint_elided_text(
                    ctx,
                    &item.description,
                    geometry.description,
                    text_secondary,
                    visual.typography.description,
                );
            }
            if let (Some(action), Some(action_rect)) =
                (self.action_label.as_deref(), geometry.action)
            {
                let action_button = Self::inset_rect(action_rect, layout.action_inset);
                if self.pressed_action.get() == Some(entry.key()) {
                    ctx.fill_rect(
                        action_button,
                        fade_color(resolved.fill_secondary, opacity),
                        Some(Radius::uniform(resolved.control_radius)),
                    );
                }
                Self::paint_centered_elided_text(
                    ctx,
                    action,
                    action_button,
                    text,
                    visual.typography.action,
                );
            }
            if item.closable {
                let close_button = Self::inset_rect(geometry.close, layout.close_inset);
                if self.pressed_close.get() == Some(entry.key()) {
                    ctx.fill_rect(
                        close_button,
                        fade_color(resolved.fill_secondary, opacity),
                        Some(Radius::uniform(resolved.control_radius)),
                    );
                } else if self.hovered_close.get() == Some(entry.key()) {
                    ctx.fill_rect(
                        close_button,
                        fade_color(resolved.fill_tertiary, opacity),
                        Some(Radius::uniform(resolved.control_radius)),
                    );
                }
                if let Some(label) = self.close_label.as_deref() {
                    Self::paint_centered_elided_text(
                        ctx,
                        label,
                        close_button,
                        fade_color(resolved.text_quaternary, opacity),
                        visual.typography.close_label,
                    );
                } else {
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        visual.icons.close,
                        close_button,
                        fade_color(resolved.text_quaternary, opacity),
                        visual.typography.close_icon,
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

    overlay_entry => (&self, id: crate::ui::WidgetId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        let frame = self.remember_frame(frame);
        self.sync_motion();
        let bounds = self.motion_paint_bounds(frame);
        (bounds.w > 0.0 && bounds.h > 0.0).then(|| {
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Notification)
                .bounds(bounds)
                .z_index(self.visual.chrome.overlay_z)
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
    // 向反馈声明与句柄暴露 UIX 唯一默认时长，不泄漏完整视觉结构。
    pub(super) const fn default_duration_ms() -> u64 {
        NOTIFICATION_VISUAL_REF.defaults.duration_ms
    }

    /// 创建拥有独立通知队列、默认位于右上角的通知容器。
    pub fn new() -> Self {
        // Rust 直接构造获得独立队列，不再修改任何进程级注册表。
        Self::from_handle(NotificationHandle::new())
    }

    pub(crate) fn from_handle(handle: NotificationHandle) -> Self {
        // Host 只接收 Application System 分配给目标窗口的窄句柄。
        let visual = NOTIFICATION_VISUAL_REF;
        Self {
            queue: handle.queue,
            motion: RefCell::new(ToastMotion::default()),
            placement: visual.defaults.placement,
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
            visual,
        }
    }

    /// 设置通知队列相对于宿主区域的停靠位置。
    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// 设置每条通知共用的动作标签与点击回调。
    ///
    /// 空白标签会被忽略，并保留无动作按钮的状态。
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

    /// 设置每条通知使用的图标名称，覆盖按状态选择的默认图标。
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon_name = Some(icon.into());
        self
    }

    /// 设置可关闭通知的关闭按钮文本。
    ///
    /// 空白文本会隐藏关闭按钮标签。
    pub fn close_text(mut self, text: impl Into<String>) -> Self {
        let text = text.into();
        self.close_label = (!text.trim().is_empty()).then_some(text);
        self
    }

    /// 设置通知容器相对停靠位置的水平与垂直偏移。
    ///
    /// 非有限数值会归零。
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

    /// 获取共享当前通知队列的命令句柄。
    pub fn handle(&self) -> NotificationHandle {
        NotificationHandle {
            queue: self.queue.clone(),
        }
    }

    /// 将通知条目加入本地队列并返回其稳定 ID。
    pub fn add(&self, item: NotificationItem) -> u64 {
        self.handle().add(item)
    }

    /// 使用给定标题、描述和状态创建一条可关闭的定时通知。
    ///
    /// 返回值是此容器本地队列中的稳定 ID。
    pub fn open(
        &self,
        title: impl Into<String>,
        description: impl Into<String>,
        type_: StatusLevel,
    ) -> u64 {
        self.handle().open(title, description, type_)
    }

    /// 创建一条成功状态通知并返回其稳定 ID。
    pub fn success(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.handle().success(title, description)
    }

    /// 创建一条信息状态通知并返回其稳定 ID。
    pub fn info(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.handle().info(title, description)
    }

    /// 创建一条警告状态通知并返回其稳定 ID。
    pub fn warning(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.handle().warning(title, description)
    }

    /// 创建一条错误状态通知并返回其稳定 ID。
    pub fn error(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.handle().error(title, description)
    }

    /// 请求移除由此容器或其句柄添加的本地通知。
    ///
    /// 找到对应 ID 时返回 `true`。
    pub fn dismiss(&self, id: u64) -> bool {
        self.handle().dismiss(id)
    }

    /// 请求移除由 `NotificationService` 提供稳定 ID 的外部通知。
    ///
    /// 找到对应 ID 时返回 `true`。
    pub fn dismiss_external(&self, id: u64) -> bool {
        self.handle().dismiss_external(id)
    }

    /// 清空当前通知队列。
    pub fn clear(&self) {
        self.handle().clear();
    }

    /// 返回当前队列中通知条目的快照。
    pub fn items(&self) -> Vec<NotificationItem> {
        self.queue.values()
    }

    /// 用可见的服务 Toast 替换当前外部通知集合。
    ///
    /// 本地添加的通知不会被移除。
    pub fn replace_from_toasts<'a, I>(&self, toasts: I)
    where
        I: IntoIterator<Item = &'a ToastEntry>,
    {
        self.handle().replace_from_toasts(toasts);
    }

    /// 更新服务的过期状态，并将其当前可见 Toast 同步到容器。
    pub fn replace_from_service(&self, service: &mut (impl NotificationSource + ?Sized)) {
        let toasts = service.update_notifications();
        self.replace_from_toasts(&toasts);
    }

    /// 将非致命框架错误交给服务转换为通知并同步容器。
    ///
    /// 致命错误或无需展示的错误返回 `None`。
    pub fn notify_error_from_service(
        &self,
        service: &mut (impl NotificationSource + ?Sized),
        error: &Error,
    ) -> Option<u64> {
        let id = service.notify_error(error);
        self.replace_from_service(service);
        id
    }

    /// 当结果为错误时将其交给服务转换为通知并同步容器。
    ///
    /// 成功结果保持静默并返回 `None`。
    pub fn notify_result_error_from_service<T>(
        &self,
        service: &mut (impl NotificationSource + ?Sized),
        result: &CoreResult<T>,
    ) -> Option<u64> {
        let id = result
            .as_ref()
            .err()
            .and_then(|error| service.notify_error(error));
        self.replace_from_service(service);
        id
    }

    fn resolved_enter_animation(&self) -> AnimationConfig {
        self.enter_animation
            .unwrap_or_else(|| AnimationConfig::fade_in(self.visual.motion.enter_duration))
    }

    fn resolved_leave_animation(&self) -> AnimationConfig {
        self.leave_animation
            .unwrap_or_else(|| AnimationConfig::fade_out(self.visual.motion.leave_duration))
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
}

// 把 Rust 运行内核与 UIX 生成的唯一静态视觉项融合为根节点。
fn build_notification_view(
    mut kernel: Notification,
    visual: &'static NotificationVisual,
) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

// 让公开 View 构建统一进入同目录 UIX 文档。
fn build_notification_uix_root(kernel: Notification) -> ViewNode {
    crate::uix!("src/ui/widgets/feedback/notification/notification.uix")
}

impl View for Notification {
    fn build(self) -> ViewNode {
        build_notification_uix_root(self)
    }
}

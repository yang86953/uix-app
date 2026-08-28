//! 浮动全局提示容器。

use std::borrow::Cow;
use std::cell::{Cell, RefCell};

mod geometry;
mod item;
mod presentation;

use self::geometry::*;
pub use self::item::{MessageHandle, MessageItem};
use self::presentation::*;

use std::rc::Rc;

use crate::core::{Constraints, Point, Rect, Size};
use crate::platform::capabilities::StatusLevel;
use crate::ui::animation::AnimationConfig;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::ui::{EventResult, MouseButton, Placement, SnapshotFields, SystemEvent};
use crate::widget;

// 引入关闭原因以区分用户关闭与到期关闭。
use super::declaration::FeedbackCloseReason;
use super::toast_motion::{ToastMotion, ToastMotionEntry, ToastQueue};
// 复用反馈组件共享的阴影外扩、颜色衰减与省略文本绘制辅助。
use super::{expand_rect, fade_token_color, paint_elided_text};

widget! {
    /// 全局浮动提示容器。
    pub struct Message {
        queue: ToastQueue<MessageItem>,
        motion: RefCell<ToastMotion<MessageItem>>,
        placement: Placement,
        enter_animation: Option<AnimationConfig>,
        leave_animation: Option<AnimationConfig>,
        last_frame: Cell<Rect>,
        motion_dirty: Cell<bool>,
        dirty_item_count: Cell<usize>,
        hovered_close: Cell<Option<super::toast_motion::ToastKey>>,
        pressed_close: Cell<Option<super::toast_motion::ToastKey>>,
        pressed_action: Cell<Option<super::toast_motion::ToastKey>>,
        // 构建后不再增长，盒装字符串避免为每个实例保留 String 容量字段。
        action_label: Option<Box<str>>,
        action_callback: Option<Rc<dyn Fn()>>,
        // 自定义图标名同样只读保存，使用精确容量盒装字符串。
        icon_name: Option<Box<str>>,
        // 所有实例只借用同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static MessageVisual,
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

        // 同帧全部提示共享一次主题解析，避免逐项重复动态令牌访问。
        let resolved = self.visual.resolve(ctx.tokens());
        let radius = Some(crate::draw::Radius::uniform(resolved.radius));
        let control_radius = Some(crate::draw::Radius::uniform(resolved.control_radius));
        let shadow = resolved.shadow;
        let layout = &self.visual.layout;
        let typography = &self.visual.typography;

        ctx.push_clip(frame);
        for (base_rect, entry) in self.message_rects(frame, motion.entries()) {
            let opacity = entry.opacity().clamp(0.0, 1.0);
            let msg_rect = transitioned_rect(base_rect, entry.offset(), entry.scale());
            if msg_rect.w <= 0.0 || msg_rect.h <= 0.0 {
                continue;
            }
            let item = entry.item();
            let bg = fade_token_color(resolved.background, opacity);
            let text_c = fade_token_color(resolved.text, opacity);
            let (default_icon, accent) = self.visual.status_visual(&resolved, item.type_);
            let icon = self.icon_name.as_deref().unwrap_or(default_icon);
            let accent = fade_token_color(accent, opacity);
            if shadow.layer_1.2 > 0.0 {
                ctx.draw_box_shadow(
                    msg_rect,
                    shadow.layer_1.2,
                    shadow.layer_1.0,
                    shadow.layer_1.1,
                    fade_token_color(shadow.layer_1.3, opacity),
                    radius,
                );
            }
            ctx.fill_rect(msg_rect, bg, radius);
            ctx.fill_rect(
                Rect::new(
                    msg_rect.x,
                    msg_rect.y + layout.accent_vertical_inset.min(msg_rect.h * 0.5),
                    msg_rect.w.min(layout.accent_width),
                    (msg_rect.h - layout.accent_vertical_inset * 2.0).max(0.0),
                ),
                accent,
                Some(crate::draw::Radius::uniform(layout.accent_radius)),
            );
            let geometry = self.item_geometry(msg_rect, item.closable);
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                icon,
                geometry.icon,
                accent,
                typography.status_icon,
            );
            paint_elided_text(
                ctx,
                &item.content,
                geometry.content,
                text_c,
                typography.body,
                false,
            );
            if let (Some(action), Some(action_rect)) =
                (self.action_label.as_deref(), geometry.action)
            {
                let action_button = Self::inset_rect(action_rect, layout.action_inset);
                if self.pressed_action.get() == Some(entry.key()) {
                    ctx.fill_rect(
                        action_button,
                        fade_token_color(resolved.fill_secondary, opacity),
                        control_radius,
                    );
                }
                paint_elided_text(
                    ctx,
                    action,
                    action_button,
                    text_c,
                    typography.action,
                    true,
                );
            }
            if item.closable {
                let close_button = Self::inset_rect(geometry.close, layout.close_inset);
                if self.pressed_close.get() == Some(entry.key()) {
                    ctx.fill_rect(
                        close_button,
                        fade_token_color(resolved.fill_secondary, opacity),
                        control_radius,
                    );
                } else if self.hovered_close.get() == Some(entry.key()) {
                    ctx.fill_rect(
                        close_button,
                        fade_token_color(resolved.fill_tertiary, opacity),
                        control_radius,
                    );
                }
                crate::ui::widgets::icon::Icon::paint_in_frame(
                    ctx,
                    self.visual.icons.close,
                    close_button,
                    fade_token_color(resolved.text_quaternary, opacity),
                    typography.close_icon,
                );
            }
        }
        ctx.pop_clip();
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        self.sync_motion();
        let item_count = self.motion.borrow().len();
        self.motion_paint_bounds(frame, item_count)
    }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        self.hit_bounds(frame).unwrap_or_else(Rect::zero)
    }

    overlay_entry => (&self, id: crate::ui::WidgetId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        let frame = self.remember_frame(frame);
        self.sync_motion();
        let item_count = self.motion.borrow().len();
        let bounds = self.motion_paint_bounds(frame, item_count);
        (bounds.w > 0.0 && bounds.h > 0.0).then(|| {
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Message)
                .bounds(bounds)
                .z_index(self.visual.overlay_z())
        })
    }

    update_animation => (&mut self, dt: f64) -> bool {
        self.dirty_item_count.set(self.motion.borrow().len());
        let sync_changed = self.sync_motion();
        let update = self.motion.borrow_mut().update(dt);
        let changed = sync_changed || update.changed;
        self.motion_dirty.set(changed);
        if changed {
            self.dirty_item_count.set(
                self.dirty_item_count
                    .get()
                    .max(update.previous_len)
                    .max(update.current_len),
            );
        }
        update.active
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.motion_dirty.get() {
            self.motion_paint_bounds(frame, self.dirty_item_count.get())
        } else {
            Rect::zero()
        }
    }
}

impl Default for Message {
    fn default() -> Self {
        Self::new()
    }
}

impl Message {
    /// 创建拥有独立队列且默认显示在顶部的全局提示容器。
    pub fn new() -> Self {
        // Rust 直接构造获得独立队列，不再修改任何进程级注册表。
        Self::from_handle(MessageHandle::new())
    }

    pub(crate) fn from_handle(handle: MessageHandle) -> Self {
        // Host 只接收 Application System 分配给目标窗口的窄句柄。
        Self {
            queue: handle.queue,
            motion: RefCell::new(ToastMotion::default()),
            placement: MESSAGE_VISUAL_REF.defaults.placement,
            enter_animation: None,
            leave_animation: None,
            last_frame: Cell::new(Rect::zero()),
            motion_dirty: Cell::new(false),
            dirty_item_count: Cell::new(0),
            hovered_close: Cell::new(None),
            pressed_close: Cell::new(None),
            pressed_action: Cell::new(None),
            action_label: None,
            action_callback: None,
            icon_name: None,
            visual: MESSAGE_VISUAL_REF,
        }
    }

    /// 设置提示队列在宿主区域中的放置位置。
    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// 设置每条提示的操作标签及回调；空白标签会被忽略。
    pub fn action<F>(mut self, label: impl Into<String>, action: F) -> Self
    where
        F: Fn() + 'static,
    {
        let label = label.into();
        if !label.trim().is_empty() {
            self.action_label = Some(label.into_boxed_str());
            self.action_callback = Some(Rc::new(action));
        }
        self
    }

    /// 设置每条提示使用的自定义图标名称。
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon_name = Some(icon.into().into_boxed_str());
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

    /// 返回共享当前提示队列的窄操作句柄。
    pub fn handle(&self) -> MessageHandle {
        MessageHandle {
            queue: self.queue.clone(),
        }
    }

    /// 添加提示项并返回其稳定标识。
    pub fn add(&self, item: MessageItem) -> u64 {
        let duration_ms = item.duration_ms;
        self.queue.push(item, duration_ms)
    }

    /// 添加使用标准时长的成功提示并返回其稳定标识。
    pub fn success(&self, content: impl Into<String>) -> u64 {
        self.add_status(StatusLevel::Success, content)
    }

    /// 添加使用标准时长的信息提示并返回其稳定标识。
    pub fn info(&self, content: impl Into<String>) -> u64 {
        self.add_status(StatusLevel::Info, content)
    }

    /// 添加使用标准时长的警告提示并返回其稳定标识。
    pub fn warning(&self, content: impl Into<String>) -> u64 {
        self.add_status(StatusLevel::Warning, content)
    }

    /// 添加使用标准时长的错误提示并返回其稳定标识。
    pub fn error(&self, content: impl Into<String>) -> u64 {
        self.add_status(StatusLevel::Error, content)
    }

    /// 请求移除指定提示；返回该标识是否对应现有本地提示。
    pub fn dismiss(&self, id: u64) -> bool {
        self.handle().dismiss(id)
    }

    /// 请求移除当前队列中的全部提示。
    pub fn clear(&self) {
        self.handle().clear();
    }

    /// 返回当前提示项的值快照。
    pub fn items(&self) -> Vec<MessageItem> {
        self.queue.values()
    }

    // 直接写入实例队列，避免便捷方法仅为一次 push 克隆共享句柄。
    fn add_status(&self, status: StatusLevel, content: impl Into<String>) -> u64 {
        self.add(MessageItem {
            type_: status,
            content: content.into(),
            duration_ms: self.visual.duration_ms(status),
            closable: true,
        })
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
        let mut motion = self.motion.borrow_mut();
        let previous_len = motion.len();
        let changed = motion.sync(
            &self.queue,
            self.resolved_enter_animation(),
            self.resolved_leave_animation(),
        );
        if changed {
            self.motion_dirty.set(true);
            self.dirty_item_count.set(
                self.dirty_item_count
                    .get()
                    .max(previous_len)
                    .max(motion.len()),
            );
        }
        for interaction in [&self.hovered_close, &self.pressed_close] {
            if interaction.get().is_some_and(|key| {
                !self
                    .message_rects(self.last_frame.get(), motion.entries())
                    .any(|(_, entry)| {
                        entry.key() == key && entry.item().closable && !entry.is_leaving()
                    })
            }) {
                interaction.set(None);
            }
        }
        if self.pressed_action.get().is_some_and(|key| {
            !self
                .message_rects(self.last_frame.get(), motion.entries())
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

    fn message_rects<'a>(
        &self,
        frame: Rect,
        entries: &'a [ToastMotionEntry<MessageItem>],
    ) -> impl Iterator<Item = (Rect, &'a ToastMotionEntry<MessageItem>)> + 'a + use<'a> {
        let frame = Self::normalize_frame(frame);
        let layout = &self.visual.layout;
        let item_height = frame.h.min(layout.item_height);
        let message_width = (frame.w - layout.surface_inset * 2.0).clamp(0.0, layout.max_width);
        let visible_count = self.visible_count(frame.h, entries.len());
        let visible = &entries[entries.len().saturating_sub(visible_count)..];
        let stack_height = self.stack_height(visible.len(), item_height);
        let start_x = frame.x
            + self
                .placement
                .horizontal_start(frame.w, message_width, layout.surface_inset);
        let start_y = frame.y
            + self
                .placement
                .vertical_start(frame.h, stack_height, layout.surface_inset);

        visible.iter().enumerate().map(move |(index, entry)| {
            let y = start_y + index as f32 * layout.item_stride;
            (Rect::new(start_x, y, message_width, item_height), entry)
        })
    }

    fn stack_height(&self, item_count: usize, item_height: f32) -> f32 {
        if item_count == 0 || item_height <= 0.0 {
            0.0
        } else {
            item_height + (item_count - 1) as f32 * self.visual.layout.item_stride
        }
    }

    fn base_bounds_for_count(&self, frame: Rect, item_count: usize) -> Rect {
        let frame = Self::normalize_frame(frame);
        let layout = &self.visual.layout;
        let visible_count = self.visible_count(frame.h, item_count);
        if visible_count == 0 {
            return Rect::zero();
        }
        let width = (frame.w - layout.surface_inset * 2.0).clamp(0.0, layout.max_width);
        let item_height = frame.h.min(layout.item_height);
        let height = self.stack_height(visible_count, item_height);
        Rect::new(
            frame.x
                + self
                    .placement
                    .horizontal_start(frame.w, width, layout.surface_inset),
            frame.y
                + self
                    .placement
                    .vertical_start(frame.h, height, layout.surface_inset),
            width,
            height,
        )
    }

    fn motion_paint_bounds(&self, frame: Rect, minimum_count: usize) -> Rect {
        let frame = Self::normalize_frame(frame);
        let motion = self.motion.borrow();
        let mut bounds = self.base_bounds_for_count(frame, minimum_count.max(motion.len()));
        for (rect, entry) in self.message_rects(frame, motion.entries()) {
            let (from, to) = entry.offset_endpoints();
            bounds = bounds
                .union(&translated_rect(rect, from))
                .union(&translated_rect(rect, to));
        }
        expand_rect(bounds, self.visual.layout.shadow_margin)
            .intersect(&frame)
            .unwrap_or_else(Rect::zero)
    }

    fn close_rect(&self, rect: Rect) -> Rect {
        let width = rect.w.min(self.visual.layout.close_width);
        Rect::new(rect.x + rect.w - width, rect.y, width, rect.h)
    }

    fn action_rect(&self, rect: Rect, closable: bool) -> Option<Rect> {
        let label = self.action_label.as_deref()?;
        let close = if closable {
            self.close_rect(rect)
        } else {
            Rect::new(rect.x + rect.w, rect.y, 0.0, rect.h)
        };
        let gap = if close.w > 0.0 {
            self.visual
                .layout
                .control_gap
                .min((close.x - rect.x).max(0.0))
        } else {
            self.visual.layout.content_trailing_gap.min(rect.w.max(0.0))
        };
        let end = (close.x - gap).max(rect.x);
        let available = (end - rect.x).max(0.0);
        let width = Self::measured_control_width(
            label,
            self.visual.typography.action,
            self.visual.layout.action_horizontal_padding,
            self.visual.layout.action_min_width,
            self.visual.layout.action_max_width,
        )
        .min(available);
        (width > 0.0).then(|| Rect::new(end - width, rect.y, width, rect.h))
    }

    pub(crate) fn hit_bounds(&self, frame: Rect) -> Option<Rect> {
        let frame = self.remember_frame(frame);
        self.sync_motion();
        let motion = self.motion.borrow();
        self.message_rects(frame, motion.entries())
            .map(|(rect, entry)| transitioned_rect(rect, entry.offset(), entry.scale()))
            .reduce(|acc, rect| acc.union(&rect))
            .and_then(|bounds| bounds.intersect(&frame))
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        if self.placement != next.placement {
            self.hovered_close.set(None);
            self.pressed_close.set(None);
        }
        self.placement = next.placement;
        self.enter_animation = next.enter_animation;
        self.leave_animation = next.leave_animation;
        self.action_label = next.action_label;
        self.action_callback = next.action_callback;
        self.icon_name = next.icon_name;
        self.visual = next.visual;
        self.pressed_action.set(None);
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Message {
            placement: self.placement,
            contents: self
                .queue
                .values()
                .into_iter()
                .map(|item| item.content)
                .collect(),
        }
    }

    fn visible_count(&self, frame_height: f32, item_count: usize) -> usize {
        let frame_height = Self::normalize_dimension(frame_height);
        if item_count == 0 || frame_height <= 0.0 {
            return 0;
        }
        let item_height = frame_height.min(self.visual.layout.item_height);
        let capacity =
            1 + ((frame_height - item_height) / self.visual.layout.item_stride).floor() as usize;
        item_count.min(capacity)
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

        self.message_rects(frame, motion.entries())
            .find_map(|(rect, entry)| {
                let rect = transitioned_rect(rect, entry.offset(), entry.scale());
                (entry.item().closable
                    && !entry.is_leaving()
                    && self.close_rect(rect).contains(pos))
                .then_some(entry.key())
            })
    }

    // 测试目标保留消息交互区域观测入口，供反馈组件命中测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn interaction_rects_for_test(
        &self,
        frame: Rect,
    ) -> Vec<(Rect, Option<Rect>, Option<Rect>)> {
        let frame = self.remember_frame(frame);
        self.sync_motion();
        let motion = self.motion.borrow();
        self.message_rects(frame, motion.entries())
            .map(|(rect, entry)| {
                let rect = transitioned_rect(rect, entry.offset(), entry.scale());
                let geometry = self.item_geometry(rect, entry.item().closable);
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

        self.message_rects(frame, motion.entries())
            .find_map(|(rect, entry)| {
                let rect = transitioned_rect(rect, entry.offset(), entry.scale());
                (!entry.is_leaving()
                    && self
                        .action_rect(rect, entry.item().closable)
                        .is_some_and(|action| action.contains(pos)))
                .then_some(entry.key())
            })
    }

    fn item_geometry(&self, rect: Rect, closable: bool) -> MessageGeometry {
        let close = if closable {
            self.close_rect(rect)
        } else {
            Rect::new(rect.x + rect.w, rect.y, 0.0, rect.h)
        };
        let action = self.action_rect(rect, closable);
        let trailing_start = action.map_or(close.x, |action| action.x);
        let content_end = (trailing_start
            - self
                .visual
                .layout
                .content_trailing_gap
                .min((trailing_start - rect.x).max(0.0)))
        .max(rect.x);
        let icon_x = rect.x + self.visual.layout.icon_inset.min(rect.w);
        let icon_width = (content_end - icon_x).clamp(0.0, self.visual.layout.icon_max_width);
        let icon = Rect::new(icon_x, rect.y, icon_width, rect.h);
        let content_x = (icon.x + icon.w + self.visual.layout.icon_content_gap).min(content_end);
        MessageGeometry {
            icon,
            content: Rect::new(
                content_x,
                rect.y,
                (content_end - content_x).max(0.0),
                rect.h,
            ),
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
        let value = Self::normalized_single_line(value);
        let text_width = crate::draw::resources::font::text_backend::estimate_text_metrics(
            value.as_ref(),
            f32::INFINITY,
            font_size,
        )
        .max_line_width;
        (text_width + horizontal_padding).clamp(minimum, maximum)
    }

    // 普通单行文本保持借用，仅在确有换行符时分配归一化结果。
    fn normalized_single_line(value: &str) -> Cow<'_, str> {
        if value.contains(['\r', '\n']) {
            Cow::Owned(value.replace(['\r', '\n'], " "))
        } else {
            Cow::Borrowed(value)
        }
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
}

// 把 Rust 运行内核与 UIX 生成的唯一静态视觉项融合为根节点。
fn build_message_view(mut kernel: Message, visual: &'static MessageVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

// 让公开 View 构建统一进入同目录 UIX 文档。
fn build_message_uix_root(kernel: Message) -> ViewNode {
    crate::uix!("src/ui/widgets/feedback/message/message.uix")
}

impl View for Message {
    fn build(self) -> ViewNode {
        build_message_uix_root(self)
    }
}

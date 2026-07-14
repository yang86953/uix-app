//! 浮动全局提示容器。

use std::cell::{Cell, RefCell};

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::native::traits::system::StatusLevel;
use crate::ui::animation::AnimationConfig;
use crate::ui::core::widget::WidgetTree;
use crate::ui::{EventResult, Placement, SnapshotFields, SystemEvent};

use super::toast_motion::{ToastMotion, ToastMotionEntry, ToastQueue};

#[derive(Debug, Clone)]
pub struct MessageItem {
    pub type_: StatusLevel,
    pub content: String,
    /// 展示时长；`0` 表示仅由调用方或关闭按钮移除。
    pub duration_ms: u64,
    pub closable: bool,
}

/// 向已挂载的 [`Message`] 队列增删提示，不暴露内部动画与定时器。
#[derive(Clone)]
pub struct MessageHandle {
    queue: ToastQueue<MessageItem>,
}

impl MessageHandle {
    /// 添加提示并返回稳定 ID。
    pub fn add(&self, item: MessageItem) -> u64 {
        let duration_ms = item.duration_ms;
        self.queue.push(item, duration_ms)
    }

    pub fn success(&self, content: impl Into<String>) -> u64 {
        self.add(MessageItem {
            type_: StatusLevel::Success,
            content: content.into(),
            duration_ms: Message::MSG_DURATION_SUCCESS,
            closable: true,
        })
    }

    pub fn info(&self, content: impl Into<String>) -> u64 {
        self.add(MessageItem {
            type_: StatusLevel::Info,
            content: content.into(),
            duration_ms: Message::MSG_DURATION_INFO,
            closable: true,
        })
    }

    pub fn warning(&self, content: impl Into<String>) -> u64 {
        self.add(MessageItem {
            type_: StatusLevel::Warning,
            content: content.into(),
            duration_ms: Message::MSG_DURATION_WARNING,
            closable: true,
        })
    }

    pub fn error(&self, content: impl Into<String>) -> u64 {
        self.add(MessageItem {
            type_: StatusLevel::Error,
            content: content.into(),
            duration_ms: Message::MSG_DURATION_ERROR,
            closable: true,
        })
    }

    /// 请求移除指定提示；离场动画完成后才从画面消失。
    pub fn dismiss(&self, id: u64) -> bool {
        self.queue.remove_local(id)
    }

    /// 请求移除全部提示。
    pub fn clear(&self) {
        self.queue.clear();
    }

    pub fn items(&self) -> Vec<MessageItem> {
        self.queue.values()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

component! {
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
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_motion();
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                let frame = self.last_frame.get();
                let clicked = {
                    let motion = self.motion.borrow();
                    let clicked = self.message_rects(frame, motion.entries()).find_map(
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

        let radius = Some(crate::draw::Radius::uniform(ctx.tokens().border_radius_lg()));
        let shadow = ctx.tokens().box_shadow();

        for (base_rect, entry) in self.message_rects(frame, motion.entries()) {
            let opacity = entry.opacity().clamp(0.0, 1.0);
            let msg_rect = transitioned_rect(base_rect, entry.offset(), entry.scale());
            let item = entry.item();
            let bg = fade_color(ctx.tokens().color_bg_elevated(), opacity);
            let text_c = fade_color(ctx.tokens().color_text(), opacity);
            let (icon, accent) = match item.type_ {
                StatusLevel::Success => ("+", ctx.tokens().color_success()),
                StatusLevel::Info => ("i", ctx.tokens().color_info()),
                StatusLevel::Warning => ("!", ctx.tokens().color_warning()),
                StatusLevel::Error => ("x", ctx.tokens().color_error()),
            };
            let accent = fade_color(accent, opacity);
            if shadow.layer_1.2 > 0.0 {
                ctx.draw_box_shadow(
                    msg_rect,
                    shadow.layer_1.2,
                    shadow.layer_1.0,
                    shadow.layer_1.1,
                    fade_color(shadow.layer_1.3, opacity),
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
            ctx.draw_text(icon, Point::new(msg_rect.x + 14.0, icon_y), accent, 14.0);

            let content_y = ctx.visual_center_y(msg_rect, 13.0);
            ctx.draw_text(
                &item.content,
                Point::new(msg_rect.x + 36.0, content_y),
                text_c,
                13.0,
            );
            if item.closable {
                let close_y = ctx.visual_center_y(msg_rect, 12.0);
                ctx.draw_text(
                    "x",
                    Point::new(msg_rect.x + msg_rect.w - 22.0, close_y),
                    fade_color(ctx.tokens().color_text_quaternary(), opacity),
                    12.0,
                );
            }
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        self.sync_motion();
        let item_count = self.motion.borrow().len();
        self.motion_paint_bounds(frame, item_count)
    }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        self.hit_bounds(frame).unwrap_or_else(Rect::zero)
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        self.last_frame.set(frame);
        self.sync_motion();
        let item_count = self.motion.borrow().len();
        let bounds = self.motion_paint_bounds(frame, item_count);
        (bounds.w > 0.0 && bounds.h > 0.0).then(|| {
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Message)
                .bounds(bounds)
                .z_index(1200)
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
    const MSG_DURATION_SUCCESS: u64 = 3000;
    const MSG_DURATION_INFO: u64 = 3000;
    const MSG_DURATION_WARNING: u64 = 4000;
    const MSG_DURATION_ERROR: u64 = 5000;
    const MSG_WIDTH: f32 = 380.0;
    const MSG_HEIGHT: f32 = 40.0;
    const MSG_HEIGHT_PER_ITEM: f32 = 48.0;
    const MSG_INSET: f32 = 12.0;
    const SHADOW_MARGIN: f32 = 12.0;

    pub fn new() -> Self {
        Self {
            queue: ToastQueue::new(),
            motion: RefCell::new(ToastMotion::default()),
            placement: Placement::Top,
            enter_animation: None,
            leave_animation: None,
            last_frame: Cell::new(Rect::zero()),
            motion_dirty: Cell::new(false),
            dirty_item_count: Cell::new(0),
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

    pub fn handle(&self) -> MessageHandle {
        MessageHandle {
            queue: self.queue.clone(),
        }
    }

    pub fn add(&self, item: MessageItem) -> u64 {
        self.handle().add(item)
    }

    pub fn success(&self, content: impl Into<String>) -> u64 {
        self.handle().success(content)
    }

    pub fn info(&self, content: impl Into<String>) -> u64 {
        self.handle().info(content)
    }

    pub fn warning(&self, content: impl Into<String>) -> u64 {
        self.handle().warning(content)
    }

    pub fn error(&self, content: impl Into<String>) -> u64 {
        self.handle().error(content)
    }

    pub fn dismiss(&self, id: u64) -> bool {
        self.handle().dismiss(id)
    }

    pub fn clear(&self) {
        self.handle().clear();
    }

    pub fn items(&self) -> Vec<MessageItem> {
        self.queue.values()
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
        changed
    }

    fn intrinsic_size(&self) -> Size {
        Size::zero()
    }

    fn message_rects<'a>(
        &self,
        frame: Rect,
        entries: &'a [ToastMotionEntry<MessageItem>],
    ) -> impl Iterator<Item = (Rect, &'a ToastMotionEntry<MessageItem>)> + 'a {
        let message_width = (frame.w - 40.0).clamp(0.0, Self::MSG_WIDTH);
        let stack_height = Self::stack_height(entries.len());
        let start_x = frame.x
            + self
                .placement
                .horizontal_start(frame.w, message_width, Self::MSG_INSET);
        let start_y = frame.y
            + self
                .placement
                .vertical_start(frame.h, stack_height, Self::MSG_INSET);

        entries.iter().enumerate().map(move |(index, entry)| {
            let y = start_y + index as f32 * Self::MSG_HEIGHT_PER_ITEM;
            (
                Rect::new(start_x, y, message_width, Self::MSG_HEIGHT),
                entry,
            )
        })
    }

    fn stack_height(item_count: usize) -> f32 {
        if item_count == 0 {
            0.0
        } else {
            Self::MSG_HEIGHT + (item_count - 1) as f32 * Self::MSG_HEIGHT_PER_ITEM
        }
    }

    fn base_bounds_for_count(&self, frame: Rect, item_count: usize) -> Rect {
        if item_count == 0 {
            return Rect::zero();
        }
        let width = (frame.w - 40.0).clamp(0.0, Self::MSG_WIDTH);
        let height = Self::stack_height(item_count);
        Rect::new(
            frame.x
                + self
                    .placement
                    .horizontal_start(frame.w, width, Self::MSG_INSET),
            frame.y
                + self
                    .placement
                    .vertical_start(frame.h, height, Self::MSG_INSET),
            width,
            height,
        )
    }

    fn motion_paint_bounds(&self, frame: Rect, minimum_count: usize) -> Rect {
        let motion = self.motion.borrow();
        let mut bounds = self.base_bounds_for_count(frame, minimum_count.max(motion.len()));
        for (rect, entry) in self.message_rects(frame, motion.entries()) {
            let (from, to) = entry.offset_endpoints();
            bounds = bounds
                .union(&translated_rect(rect, from))
                .union(&translated_rect(rect, to));
        }
        expand_rect(bounds, Self::SHADOW_MARGIN)
    }

    fn close_rect(rect: Rect) -> Rect {
        Rect::new(rect.x + rect.w - 40.0, rect.y, 40.0, rect.h)
    }

    pub(crate) fn hit_bounds(&self, frame: Rect) -> Option<Rect> {
        self.last_frame.set(frame);
        self.sync_motion();
        let motion = self.motion.borrow();
        self.message_rects(frame, motion.entries())
            .map(|(rect, entry)| transitioned_rect(rect, entry.offset(), entry.scale()))
            .reduce(|acc, rect| acc.union(&rect))
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.placement = next.placement;
        self.enter_animation = next.enter_animation;
        self.leave_animation = next.leave_animation;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Message {
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
    if rect.w <= 0.0 || rect.h <= 0.0 {
        Rect::zero()
    } else {
        Rect::new(
            rect.x - margin,
            rect.y - margin,
            rect.w + margin * 2.0,
            rect.h + margin * 2.0,
        )
    }
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

//! 通知布局与绘制辅助。

use crate::core::{Point, Rect, Size};
use crate::draw::Color;
use crate::ui::SnapshotFields;
use crate::ui::widget_runtime::paint_context::PaintContext;

use super::{Notification, NotificationItem};
use crate::ui::widgets::feedback::toast_motion::{ToastKey, ToastMotionEntry};

impl Notification {
    pub(super) fn intrinsic_size(&self) -> Size {
        Size::zero()
    }

    pub(super) fn notification_rects<'a>(
        &self,
        frame: Rect,
        entries: &'a [ToastMotionEntry<NotificationItem>],
    ) -> impl Iterator<Item = (Rect, &'a ToastMotionEntry<NotificationItem>)> + 'a + use<'a> {
        let frame = Self::normalize_frame(frame);
        let layout = &self.visual.layout;
        // 先为两侧保留固定留白，再以通知设计宽度限制剩余可用区。
        let width = (frame.w - layout.horizontal_inset * 2.0).clamp(0.0, layout.max_width);
        let visible_start = self.visible_start(frame.h, entries);
        let visible = &entries[visible_start..];
        let stack_height = self.stack_height(visible, frame.h);
        // 迭代器只捕获复制后的 UIX 数值，不把 self 生命周期绑到条目切片。
        let compact_height = layout.compact_height;
        let detailed_height = layout.detailed_height;
        let item_gap = layout.item_gap;
        let start_x = frame.x
            + self
                .placement
                .horizontal_start(frame.w, width, layout.horizontal_inset)
            + self.offset.x;
        let mut y = frame.y
            + self
                .placement
                .vertical_start(frame.h, stack_height, layout.vertical_inset)
            + self.offset.y;

        visible.iter().map(move |entry| {
            let height = if entry.item().description.is_empty() {
                compact_height
            } else {
                detailed_height
            }
            .min(frame.h);
            let rect = Rect::new(start_x, y, width, height);
            y += height + item_gap;
            (rect, entry)
        })
    }

    fn stack_height(
        &self,
        entries: &[ToastMotionEntry<NotificationItem>],
        frame_height: f32,
    ) -> f32 {
        if entries.is_empty() || frame_height <= 0.0 {
            return 0.0;
        }
        entries
            .iter()
            .map(|entry| self.toast_height(entry.item()).min(frame_height))
            .sum::<f32>()
            + entries.len().saturating_sub(1) as f32 * self.visual.layout.item_gap
    }

    fn toast_height(&self, item: &NotificationItem) -> f32 {
        if item.description.is_empty() {
            self.visual.layout.compact_height
        } else {
            self.visual.layout.detailed_height
        }
    }

    pub(super) fn motion_paint_bounds(&self, frame: Rect) -> Rect {
        let motion = self.motion.borrow();
        self.motion_paint_bounds_for_entries(frame, motion.entries())
    }

    pub(super) fn motion_paint_bounds_for_entries(
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
            .map(|bounds| expand_rect(bounds, self.visual.layout.shadow_margin))
            .and_then(|bounds| bounds.intersect(&frame))
            .unwrap_or_else(Rect::zero)
    }

    fn close_rect(&self, rect: Rect, closable: bool) -> Rect {
        if !closable {
            return Rect::new(rect.x + rect.w, rect.y, 0.0, rect.h);
        }
        let layout = &self.visual.layout;
        let desired = self
            .close_label
            .as_deref()
            .map_or(layout.close_default_width, |label| {
                Self::measured_control_width(
                    label,
                    self.visual.typography.close_label,
                    layout.close_horizontal_padding,
                    layout.close_min_width,
                    layout.close_max_width,
                )
            });
        let width = desired.min(rect.w.max(0.0));
        Rect::new(rect.x + rect.w - width, rect.y, width, rect.h)
    }

    fn action_rect(&self, rect: Rect, closable: bool) -> Option<Rect> {
        let label = self.action_label.as_deref()?;
        let layout = &self.visual.layout;
        let close = self.close_rect(rect, closable);
        let gap = if close.w > 0.0 {
            layout.control_gap.min((close.x - rect.x).max(0.0))
        } else {
            layout.content_trailing_gap.min(rect.w.max(0.0))
        };
        let end = (close.x - gap).max(rect.x);
        let available = (end - rect.x).max(0.0);
        let width = Self::measured_control_width(
            label,
            self.visual.typography.action,
            layout.action_horizontal_padding,
            layout.action_min_width,
            layout.action_max_width,
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
            || !std::ptr::eq(self.visual, next.visual)
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
        self.visual = next.visual;
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

    fn visible_start(
        &self,
        frame_height: f32,
        entries: &[ToastMotionEntry<NotificationItem>],
    ) -> usize {
        let frame_height = Self::normalize_dimension(frame_height);
        if entries.is_empty() || frame_height <= 0.0 {
            return entries.len();
        }
        let mut start = entries.len();
        let mut height = 0.0;
        for index in (0..entries.len()).rev() {
            let item_height = self.toast_height(entries[index].item()).min(frame_height);
            let candidate = if start == entries.len() {
                item_height
            } else {
                item_height + self.visual.layout.item_gap + height
            };
            if candidate > frame_height && start < entries.len() {
                break;
            }
            start = index;
            height = candidate;
        }
        start
    }

    pub(super) fn remember_frame(&self, frame: Rect) -> Rect {
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

    pub(super) fn close_target_at(&self, local_pos: Point) -> Option<ToastKey> {
        let frame = self.last_frame.get();
        let pos = Point::new(local_pos.x + frame.x, local_pos.y + frame.y);
        if !frame.contains(pos) {
            return None;
        }
        let motion = self.motion.borrow();

        self.notification_rects(frame, motion.entries())
            .find_map(|(rect, entry)| {
                let rect = transitioned_rect(rect, entry.offset(), entry.scale());
                (entry.item().closable
                    && !entry.is_leaving()
                    && self.close_rect(rect, true).contains(pos))
                .then_some(entry.key())
            })
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

    pub(super) fn action_target_at(&self, local_pos: Point) -> Option<ToastKey> {
        let frame = self.last_frame.get();
        let pos = Point::new(local_pos.x + frame.x, local_pos.y + frame.y);
        if !frame.contains(pos) {
            return None;
        }
        let motion = self.motion.borrow();

        self.notification_rects(frame, motion.entries())
            .find_map(|(rect, entry)| {
                let rect = transitioned_rect(rect, entry.offset(), entry.scale());
                (!entry.is_leaving()
                    && self
                        .action_rect(rect, entry.item().closable)
                        .is_some_and(|action| action.contains(pos)))
                .then_some(entry.key())
            })
    }

    pub(super) fn item_geometry(
        &self,
        rect: Rect,
        item: &NotificationItem,
    ) -> NotificationGeometry {
        let close = self.close_rect(rect, item.closable);
        let action = self.action_rect(rect, item.closable);
        let layout = &self.visual.layout;
        let trailing_start = action.map_or(close.x, |action| action.x);
        let content_end = (trailing_start
            - layout
                .content_trailing_gap
                .min((trailing_start - rect.x).max(0.0)))
        .max(rect.x);
        let icon_x = rect.x + layout.icon_inset.min(rect.w);
        let icon_width = (content_end - icon_x).clamp(0.0, layout.icon_max_width);
        let icon = Rect::new(icon_x, rect.y, icon_width, rect.h);
        let content_x = (icon.x + icon.w + layout.icon_content_gap).min(content_end);
        let content_width = (content_end - content_x).max(0.0);
        let (title, description) = if item.description.is_empty() {
            (
                Rect::new(content_x, rect.y, content_width, rect.h),
                Rect::zero(),
            )
        } else {
            let title_height = rect.h.min(layout.title_height);
            (
                Rect::new(
                    content_x,
                    rect.y + layout.title_top_inset,
                    content_width,
                    title_height,
                ),
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

    pub(super) fn inset_rect(frame: Rect, inset: f32) -> Rect {
        let inset_x = inset.min(frame.w * 0.5);
        let inset_y = inset.min(frame.h * 0.5);
        Rect::new(
            frame.x + inset_x,
            frame.y + inset_y,
            (frame.w - inset_x * 2.0).max(0.0),
            (frame.h - inset_y * 2.0).max(0.0),
        )
    }

    pub(super) fn paint_elided_text(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
    ) {
        // 复用 UI 绘制上下文拥有的保守单行省略算法。
        let Some(value) = ctx.elide_single_line(value, font_size, frame.w) else {
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

    pub(super) fn paint_centered_elided_text(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
    ) {
        // 复用 UI 绘制上下文拥有的保守单行省略算法。
        let Some(value) = ctx.elide_single_line(value, font_size, frame.w) else {
            return;
        };
        if frame.h <= 0.0 {
            return;
        }
        ctx.push_clip(frame);
        ctx.text_center(&value, frame, color, font_size);
        ctx.pop_clip();
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NotificationGeometry {
    pub(crate) icon: Rect,
    pub(crate) title: Rect,
    pub(crate) description: Rect,
    pub(crate) action: Option<Rect>,
    pub(crate) close: Rect,
}

pub(super) fn transitioned_rect(rect: Rect, offset: Point, scale: f32) -> Rect {
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

pub(super) fn union_nonempty(first: Rect, second: Rect) -> Rect {
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

pub(super) fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

pub(super) fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

// 仅在单元测试构建中编译通知几何契约。
#[cfg(test)]
// 将窄窗口回归测试收拢在布局实现旁。
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/feedback/notification/layout__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;

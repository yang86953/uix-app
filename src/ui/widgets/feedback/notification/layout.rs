//! 通知布局与绘制辅助。

use crate::core::{Point, Rect, Size};
use crate::draw::Color;
use crate::ui::SnapshotFields;
use crate::ui::component::paint_context::PaintContext;

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
        // 先为两侧保留固定留白，再以通知设计宽度限制剩余可用区。
        let width = (frame.w - Self::HORIZONTAL_INSET * 2.0).clamp(0.0, Self::WIDTH);
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

    pub(super) fn action_target_at(&self, local_pos: Point) -> Option<ToastKey> {
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

    pub(super) fn item_geometry(
        &self,
        rect: Rect,
        item: &NotificationItem,
    ) -> NotificationGeometry {
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

    pub(super) fn paint_centered_elided_text(
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
mod tests {
    // 复用通知布局实现与父模块导入的组件类型。
    use super::*;
    // 引入通知状态级别以构造最小条目。
    use crate::platform::capabilities::StatusLevel;

    // 标记窄窗口双侧留白回归契约。
    #[test]
    // 验证固定宽度通知在不足 432 像素时主动缩窄。
    fn notification_preserves_horizontal_insets_in_narrow_frame() {
        // 构造默认右上角通知容器。
        let notification = Notification::new();
        // 放入一条不自动消失的单行通知。
        notification.add(NotificationItem {
            // 使用信息级别避免业务差异影响几何。
            type_: StatusLevel::Info,
            // 提供最小标题内容。
            title: "窄窗口".to_owned(),
            // 保持描述为空以固定 48 像素条目高度。
            description: String::new(),
            // 禁用自动超时以保持测试条目稳定。
            duration_ms: 0,
            // 保留默认关闭控件以覆盖真实几何路径。
            closable: true,
        });
        // 选择无法同时容纳 384 像素通知与双侧留白的窗口。
        let frame = Rect::new(0.0, 0.0, 400.0, 200.0);
        // 读取绘制与交互共同消费的通知矩形。
        let rects = notification.interaction_rects_for_test(frame);
        // 确认唯一条目没有被可见性裁剪淘汰。
        assert_eq!(rects.len(), 1);
        // 取出动画变换后的实际条目矩形。
        let toast = rects[0].0;
        // 双侧各保留 24 像素，条目因此缩窄为 352 像素。
        assert_eq!(toast, Rect::new(24.0, 12.0, 352.0, 48.0));
        // 命中边界必须复用同一个动画后条目矩形。
        assert_eq!(notification.hit_bounds(frame), Some(toast));
        // 结束窄窗口通知几何契约。
    }
    // 结束通知布局单元测试模块。
}

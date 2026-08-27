//! 气泡确认框行为与几何。

use super::*;

// 构造、声明式配置和可见性生命周期保持在同一私有实现边界内。
mod builder;

impl Popconfirm {
    pub(crate) fn sync_from(&mut self, next: Self) {
        let geometry_changed = self.placement != next.placement
            || self.arrow != next.arrow
            || !std::ptr::eq(self.visual, next.visual);
        self.title = next.title;
        self.confirm_text = next.confirm_text;
        self.cancel_text = next.cancel_text;
        self.placement = next.placement;
        self.arrow = next.arrow;
        self.icon = next.icon;
        // 同步声明期组合触发器模式。
        self.custom_trigger = next.custom_trigger;
        // 下一声明提供新的完整 trigger ViewNode 供组件树 reconcile。
        self.custom_trigger_view = next.custom_trigger_view;
        // 同步最新确认业务回调。
        self.confirm_callback = next.confirm_callback;
        // 同步最新取消业务回调。
        self.cancel_callback = next.cancel_callback;
        // UIX 静态视觉项按共享引用替换，不复制完整视觉表。
        self.visual = next.visual;
        if geometry_changed {
            self.cancel_pending_activation();
        }
    }

    fn trigger_rect(&self) -> Rect {
        let frame = self.last_frame.get();
        // 组合模式优先消费本轮布局测得的真实子树尺寸。
        let trigger_size = self.trigger_size.get();
        let width = if self.custom_trigger && trigger_size.w > 0.0 {
            trigger_size.w
        } else if frame.w > 0.0 {
            frame.w
        } else {
            self.visual.defaults.trigger_width
        };
        let height = if self.custom_trigger && trigger_size.h > 0.0 {
            trigger_size.h
        } else if frame.h > 0.0 {
            frame.h
        } else {
            self.visual.defaults.trigger_height
        };
        Rect::new(0.0, 0.0, width, height)
    }

    // 返回当前组合模式是否由真实子 trigger 拥有交互。
    pub(crate) fn uses_custom_trigger(&self) -> bool {
        // 只有已经挂载唯一子树时才启用代理语义。
        self.custom_trigger && self.trigger_child_count == 1
    }

    // 把组件 frame 收敛为实际 trigger 的绝对 border-box。
    pub(super) fn absolute_trigger_frame(&self, frame: Rect) -> Rect {
        // 组合模式使用本轮布局事实，兼容模式沿用组件 frame。
        let size = self.trigger_size.get();
        // 只有正尺寸组合 trigger 才覆盖包装节点尺寸。
        if self.uses_custom_trigger() && size.w > 0.0 && size.h > 0.0 {
            // trigger 与包装节点共享布局原点。
            Rect::new(frame.x, frame.y, size.w, size.h)
        } else {
            // 兼容或布局尚未完成时使用归一化包装 frame。
            Self::normalize_frame(frame)
        }
    }

    fn button_rects(&self) -> (Rect, Rect) {
        button_rects_for_popup(self.popup_rect.get())
    }

    pub(super) fn target_at(&self, pos: Point) -> Option<PopconfirmTarget> {
        if self.trigger_rect().contains(pos) {
            return Some(PopconfirmTarget::Trigger);
        }
        if !self.visible {
            return None;
        }
        let (confirm, cancel) = self.button_rects();
        if confirm.contains(pos) {
            Some(PopconfirmTarget::Confirm)
        } else if cancel.contains(pos) {
            Some(PopconfirmTarget::Cancel)
        } else {
            None
        }
    }

    pub(super) fn cancel_pending_activation(&mut self) {
        self.pressed_target = None;
        self.pressed_key = None;
    }

    // 执行唯一确认动作并保留兼容 Submit 语义。
    pub(crate) fn confirm_action(&mut self) {
        // 只允许稳定打开且未提交动作的实例进入回调。
        if !self.visible || self.closing || self.action_committed {
            // 离场或重复输入不得再次执行业务动作。
            return;
        }
        // 先锁定本轮动作，防止回调或后续事件重入。
        self.action_committed = true;
        // 克隆窄回调句柄以避免同步调用期间借用 self。
        if let Some(callback) = self.confirm_callback.clone() {
            // 在当前调用线程同步执行确认副作用。
            callback();
        }
        // 继续发布既有 Submit(id, "confirm") 兼容事实。
        self.pending_submit.set(true);
        // 回调返回后进入正常离场。
        self.close();
    }

    // 执行所有用户取消入口共用的唯一动作。
    pub(crate) fn cancel_action(&mut self) {
        // 只允许稳定打开且未提交动作的实例进入回调。
        if !self.visible || self.closing || self.action_committed {
            // 离场或重复输入不得再次执行业务动作。
            return;
        }
        // 先锁定本轮动作，防止 PointerUp、FocusOut 或动画重入。
        self.action_committed = true;
        // 克隆窄回调句柄以避免同步调用期间借用 self。
        if let Some(callback) = self.cancel_callback.clone() {
            // 在当前调用线程同步执行取消副作用。
            callback();
        }
        // 用户取消不发布 Submit 或 Change，只进入正常离场。
        self.close();
    }

    pub(super) fn intrinsic_size(&self) -> Size {
        Size::new(
            self.visual.defaults.trigger_width,
            self.visual.defaults.trigger_height,
        )
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Popconfirm(SnapshotPopconfirm {
            title: self.title.clone(),
            confirm_text: self.confirm_text.clone(),
            cancel_text: self.cancel_text.clone(),
            placement: self.placement,
            arrow: self.arrow,
            icon: self.icon,
            visible: self.visible,
            focused_action: self.focused_action(),
        })
    }

    pub(super) fn absolute_popup_rect(&self, frame: Rect) -> Rect {
        // 始终以当前表面重新解析，避免触发器不动时沿用旧窗口边界下的缓存。
        resolve_popconfirm_geometry(
            // 传入当前触发器矩形。
            self.absolute_trigger_frame(frame),
            // 传入布局阶段或绘制阶段记录的最新表面。
            self.surface_or_fallback(frame),
            // 保留作者指定位置。
            self.placement,
            // 保留箭头间距配置。
            self.arrow,
            // 使用标准确认气泡宽度。
            self.visual.defaults.popup_width,
            // 使用标准确认气泡高度。
            self.visual.defaults.popup_height,
        )
        // 返回同一解析器生成的最终矩形。
        .popup
    }

    pub(super) fn surface_or_fallback(&self, frame: Rect) -> Rect {
        let surface = self.surface_rect.get();
        if surface.w > 0.0 && surface.h > 0.0 {
            surface
        } else {
            let defaults = &self.visual.defaults;
            let layout = &self.visual.layout;
            frame.union(&Rect::new(
                frame.x - defaults.popup_width * layout.fallback_offset_popups,
                frame.y - defaults.popup_height * layout.fallback_offset_popups,
                defaults.popup_width * layout.fallback_span_popups + frame.w,
                defaults.popup_height * layout.fallback_span_popups + frame.h,
            ))
        }
    }

    pub(super) fn dirty_rect_for_frame(&self, frame: Rect) -> Rect {
        let frame = Self::normalize_frame(frame);
        self.absolute_trigger_frame(frame)
            .union(&expand_popconfirm_rect(
                self.absolute_popup_rect(frame),
                self.visual.layout.shadow_expand,
            ))
            .intersect(&self.surface_or_fallback(frame))
            .unwrap_or_default()
    }

    pub(super) fn normalize_frame(frame: Rect) -> Rect {
        Rect::new(
            if frame.x.is_finite() { frame.x } else { 0.0 },
            if frame.y.is_finite() { frame.y } else { 0.0 },
            if frame.w.is_finite() {
                frame.w.max(0.0)
            } else {
                0.0
            },
            if frame.h.is_finite() {
                frame.h.max(0.0)
            } else {
                0.0
            },
        )
    }

    pub(super) fn paint_elided_text(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
        centered: bool,
    ) {
        // 复用 UI 绘制上下文拥有的保守单行省略算法。
        let Some(value) = ctx.elide_single_line(value, font_size, frame.w) else {
            return;
        };
        if frame.h <= 0.0 {
            return;
        }
        ctx.push_clip(frame);
        if centered {
            ctx.text_center(&value, frame, color, font_size);
        } else {
            let y = ctx.visual_center_y(frame, font_size);
            ctx.draw_text(&value, Point::new(frame.x, y), color, font_size);
        }
        ctx.pop_clip();
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PopconfirmGeometry {
    pub(crate) popup: Rect,
    pub(crate) placement: PopconfirmPlacement,
}

pub(super) fn resolve_popconfirm_geometry(
    trigger: Rect,
    surface: Rect,
    placement: PopconfirmPlacement,
    arrow: bool,
    preferred_width: f32,
    preferred_height: f32,
) -> PopconfirmGeometry {
    let width = preferred_width.min(surface.w).max(0.0);
    let height = preferred_height.min(surface.h).max(0.0);
    if width <= 0.0 || height <= 0.0 {
        return PopconfirmGeometry {
            popup: Rect::zero(),
            placement,
        };
    }
    let flipped = match placement {
        PopconfirmPlacement::Top => PopconfirmPlacement::Bottom,
        PopconfirmPlacement::TopLeft => PopconfirmPlacement::BottomLeft,
        PopconfirmPlacement::TopRight => PopconfirmPlacement::BottomRight,
        PopconfirmPlacement::Bottom => PopconfirmPlacement::Top,
        PopconfirmPlacement::BottomLeft => PopconfirmPlacement::TopLeft,
        PopconfirmPlacement::BottomRight => PopconfirmPlacement::TopRight,
    };
    let authored = rect_for_popconfirm_placement(trigger, placement, arrow, width, height);
    let alternate = rect_for_popconfirm_placement(trigger, flipped, arrow, width, height);
    let (candidate, resolved) = if popconfirm_overflow_score(alternate, surface)
        < popconfirm_overflow_score(authored, surface)
    {
        (alternate, flipped)
    } else {
        (authored, placement)
    };
    let max_x = surface.x + surface.w - width;
    let max_y = surface.y + surface.h - height;
    PopconfirmGeometry {
        popup: Rect::new(
            candidate.x.clamp(surface.x, max_x),
            candidate.y.clamp(surface.y, max_y),
            width,
            height,
        ),
        placement: resolved,
    }
}

pub(super) fn rect_for_popconfirm_placement(
    frame: Rect,
    placement: PopconfirmPlacement,
    arrow: bool,
    width: f32,
    height: f32,
) -> Rect {
    let (x, y) = popconfirm_position(frame, placement, arrow, width, height);
    Rect::new(x, y, width, height)
}

pub(super) fn popconfirm_overflow_score(rect: Rect, surface: Rect) -> f32 {
    (surface.x - rect.x).max(0.0)
        + (surface.y - rect.y).max(0.0)
        + (rect.x + rect.w - surface.x - surface.w).max(0.0)
        + (rect.y + rect.h - surface.y - surface.h).max(0.0)
}

pub(super) fn button_rects_for_popup(popup: Rect) -> (Rect, Rect) {
    let layout = &POPCONFIRM_VISUAL_REF.layout;
    let inset = layout.content_inset.min(popup.w * 0.5);
    let gap = layout.button_gap.min(popup.w);
    let available = (popup.w - inset * 2.0 - gap).max(0.0);
    let button_width = available * 0.5;
    let button_height = layout
        .button_height
        .min((popup.h - layout.button_bottom_inset).max(0.0));
    let y = (popup.y + popup.h - layout.button_bottom_inset - button_height).max(popup.y);
    (
        Rect::new(popup.x + inset, y, button_width, button_height),
        Rect::new(
            popup.x + inset + button_width + gap,
            y,
            button_width,
            button_height,
        ),
    )
}

pub(super) fn popconfirm_position(
    frame: Rect,
    placement: PopconfirmPlacement,
    arrow: bool,
    pw: f32,
    ph: f32,
) -> (f32, f32) {
    let layout = &POPCONFIRM_VISUAL_REF.layout;
    let gap = if arrow {
        layout.arrow_gap
    } else {
        layout.plain_gap
    };
    match placement {
        PopconfirmPlacement::Top | PopconfirmPlacement::TopLeft => (frame.x, frame.y - ph - gap),
        PopconfirmPlacement::TopRight => (frame.x + frame.w - pw, frame.y - ph - gap),
        PopconfirmPlacement::Bottom | PopconfirmPlacement::BottomLeft => {
            (frame.x, frame.y + frame.h + gap)
        }
        PopconfirmPlacement::BottomRight => (frame.x + frame.w - pw, frame.y + frame.h + gap),
    }
}

pub(super) fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

pub(super) fn expand_popconfirm_rect(rect: Rect, amount: f32) -> Rect {
    if rect.w <= 0.0 || rect.h <= 0.0 {
        Rect::zero()
    } else {
        Rect::new(
            rect.x - amount,
            rect.y - amount,
            rect.w + amount * 2.0,
            rect.h + amount * 2.0,
        )
    }
}

pub(super) fn draw_popconfirm_arrow(
    ctx: &mut PaintContext,
    trigger: Rect,
    popup: Rect,
    placement: PopconfirmPlacement,
    color: Color,
) {
    let arrow_sz = POPCONFIRM_VISUAL_REF.layout.arrow_size;
    let (x1, y1, x2, y2, x3, y3) = match placement {
        PopconfirmPlacement::Top | PopconfirmPlacement::TopLeft | PopconfirmPlacement::TopRight => {
            let cx =
                popconfirm_arrow_anchor(trigger.x + trigger.w * 0.5, popup.x, popup.w, arrow_sz);
            (
                cx - arrow_sz,
                popup.y + popup.h,
                cx + arrow_sz,
                popup.y + popup.h,
                cx,
                popup.y + popup.h + arrow_sz,
            )
        }
        PopconfirmPlacement::Bottom
        | PopconfirmPlacement::BottomLeft
        | PopconfirmPlacement::BottomRight => {
            let cx =
                popconfirm_arrow_anchor(trigger.x + trigger.w * 0.5, popup.x, popup.w, arrow_sz);
            (
                cx - arrow_sz,
                popup.y,
                cx + arrow_sz,
                popup.y,
                cx,
                popup.y - arrow_sz,
            )
        }
    };
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    pb.line_to(x3, y3);
    pb.close();
    ctx.fill_path(&pb.build(), color, FillRule::NonZero);
}

pub(super) fn popconfirm_arrow_anchor(desired: f32, start: f32, length: f32, inset: f32) -> f32 {
    if length <= inset * 2.0 {
        start + length * 0.5
    } else {
        desired.clamp(start + inset, start + length - inset)
    }
}

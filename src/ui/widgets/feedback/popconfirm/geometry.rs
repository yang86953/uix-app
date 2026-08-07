//! 气泡确认框行为与几何。

use super::*;

impl Popconfirm {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            confirm_text: "OK".to_string(),
            cancel_text: "Cancel".to_string(),
            visible: false,
            placement: PopconfirmPlacement::Top,
            arrow: true,
            icon: true,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            focused: false,
            focused_action: 0,
            pending_submit: Cell::new(false),
            hovered_target: None,
            pressed_target: None,
            pressed_key: None,
            last_frame: Cell::new(Rect::zero()),
            popup_rect: Cell::new(Rect::new(
                0.0,
                -POPCONFIRM_HEIGHT - 10.0,
                POPCONFIRM_WIDTH,
                POPCONFIRM_HEIGHT,
            )),
            surface_rect: Cell::new(Rect::zero()),
        }
    }
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = t.into();
        self
    }
    pub fn confirm_text(mut self, t: impl Into<String>) -> Self {
        self.confirm_text = t.into();
        self
    }
    pub fn cancel_text(mut self, t: impl Into<String>) -> Self {
        self.cancel_text = t.into();
        self
    }
    pub fn placement(mut self, p: PopconfirmPlacement) -> Self {
        self.placement = p;
        self
    }
    pub fn arrow(mut self, v: bool) -> Self {
        self.arrow = v;
        self
    }
    pub fn icon(mut self, v: bool) -> Self {
        self.icon = v;
        self
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn is_present(&self) -> bool {
        self.visible || self.closing
    }

    pub fn open(&mut self) {
        self.cancel_pending_activation();
        self.pending_submit.set(false);
        self.focused_action = 0;
        self.visible = true;
        self.closing = false;
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    pub(crate) fn focused_action(&self) -> Option<usize> {
        self.visible.then_some(self.focused_action)
    }

    pub fn close(&mut self) {
        self.cancel_pending_activation();
        if !self.is_present() {
            self.visible = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }
        self.visible = false;
        self.closing = true;
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let geometry_changed = self.placement != next.placement || self.arrow != next.arrow;
        self.title = next.title;
        self.confirm_text = next.confirm_text;
        self.cancel_text = next.cancel_text;
        self.placement = next.placement;
        self.arrow = next.arrow;
        self.icon = next.icon;
        if geometry_changed {
            self.cancel_pending_activation();
        }
    }

    fn trigger_rect(&self) -> Rect {
        let frame = self.last_frame.get();
        let width = if frame.w > 0.0 {
            frame.w
        } else {
            TRIGGER_WIDTH
        };
        let height = if frame.h > 0.0 {
            frame.h
        } else {
            TRIGGER_HEIGHT
        };
        Rect::new(0.0, 0.0, width, height)
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

    pub(super) fn confirm(&mut self) {
        self.pending_submit.set(true);
        self.close();
    }

    pub(super) fn intrinsic_size(&self) -> Size {
        Size::new(TRIGGER_WIDTH, TRIGGER_HEIGHT)
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
            frame,
            // 传入布局阶段或绘制阶段记录的最新表面。
            self.surface_or_fallback(frame),
            // 保留作者指定位置。
            self.placement,
            // 保留箭头间距配置。
            self.arrow,
            // 使用标准确认气泡宽度。
            POPCONFIRM_WIDTH,
            // 使用标准确认气泡高度。
            POPCONFIRM_HEIGHT,
        )
        // 返回同一解析器生成的最终矩形。
        .popup
    }

    pub(super) fn surface_or_fallback(&self, frame: Rect) -> Rect {
        let surface = self.surface_rect.get();
        if surface.w > 0.0 && surface.h > 0.0 {
            surface
        } else {
            frame.union(&Rect::new(
                frame.x - POPCONFIRM_WIDTH * 2.0,
                frame.y - POPCONFIRM_HEIGHT * 2.0,
                POPCONFIRM_WIDTH * 5.0 + frame.w,
                POPCONFIRM_HEIGHT * 5.0 + frame.h,
            ))
        }
    }

    pub(super) fn dirty_rect_for_frame(&self, frame: Rect) -> Rect {
        let frame = Self::normalize_frame(frame);
        frame
            .union(&expand_popconfirm_rect(
                self.absolute_popup_rect(frame),
                12.0,
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
        let Some(value) = Self::elide_single_line(ctx, value, font_size, frame.w) else {
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
    let inset = 12.0_f32.min(popup.w * 0.5);
    let gap = 8.0_f32.min(popup.w);
    let available = (popup.w - inset * 2.0 - gap).max(0.0);
    let button_width = available * 0.5;
    let button_height = 26.0_f32.min((popup.h - 10.0).max(0.0));
    let y = (popup.y + popup.h - 10.0 - button_height).max(popup.y);
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
    let gap = if arrow { 10.0 } else { 4.0 };
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
    let arrow_sz = 6.0;
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

// 仅在测试构建中编译确认气泡几何契约。
#[cfg(test)]
// 将测试放在同模块内以核验私有缓存状态。
mod tests {
    // 复用被测模块中的组件与几何辅助函数。
    use super::*;

    // 标记表面缩放时缓存几何必须失效的回归契约。
    #[test]
    // 触发器不动时，缩小表面也必须重新约束确认气泡。
    fn popup_cache_does_not_survive_surface_resize() {
        // 构造顶部展开、会在窄表面内水平收敛的确认气泡。
        let mut popconfirm = Popconfirm::new().placement(PopconfirmPlacement::Top);
        // 打开确认气泡以覆盖真实的浮层登记路径。
        popconfirm.open();
        // 固定触发器位置以隔离表面尺寸这一项变量。
        let frame = Rect::new(200.0, 140.0, 40.0, 20.0);
        // 大表面允许气泡保持作者指定的水平位置。
        let large_surface = Rect::new(0.0, 0.0, 500.0, 300.0);
        // 小表面要求气泡向左约束到可见范围内。
        let small_surface = Rect::new(0.0, 0.0, 260.0, 200.0);
        // 计算大表面下已绘制并写入缓存的确认气泡矩形。
        let cached = resolve_popconfirm_geometry(
            // 传入固定触发器矩形。
            frame,
            // 传入初始大表面。
            large_surface,
            // 沿用组件作者指定位置。
            PopconfirmPlacement::Top,
            // 保留箭头间距。
            true,
            // 使用组件标准宽度。
            POPCONFIRM_WIDTH,
            // 使用组件标准高度。
            POPCONFIRM_HEIGHT,
        )
        // 只取最终确认气泡矩形。
        .popup;
        // 模拟上一帧绘制留下的触发器缓存键。
        popconfirm.last_frame.set(frame);
        // 模拟上一帧绘制留下的相对气泡缓存。
        popconfirm.popup_rect.set(Rect::new(
            // 保存相对触发器的横坐标。
            cached.x - frame.x,
            // 保存相对触发器的纵坐标。
            cached.y - frame.y,
            // 保存缓存宽度。
            cached.w,
            // 保存缓存高度。
            cached.h,
        ));
        // 先记录与缓存一致的大表面。
        popconfirm.surface_rect.set(large_surface);
        // 通过布局阶段的新能力注入缩小后的当前表面。
        let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
            // 传入被测确认气泡组件。
            &popconfirm,
            // 使用稳定的测试组件标识。
            crate::core::ComponentId::new(2),
            // 保持触发器 frame 不变。
            frame,
            // 仅改变当前逻辑表面。
            small_surface,
        );
        // 打开状态必须生成使用新表面的浮层登记。
        assert!(overlay.is_some());
        // 以新表面直接计算当前帧应使用的几何。
        let expected = resolve_popconfirm_geometry(
            // 触发器保持不变。
            frame,
            // 表面改为缩小后的尺寸。
            small_surface,
            // 位置配置保持不变。
            PopconfirmPlacement::Top,
            // 箭头配置保持不变。
            true,
            // 宽度配置保持不变。
            POPCONFIRM_WIDTH,
            // 高度配置保持不变。
            POPCONFIRM_HEIGHT,
        )
        // 只比较最终确认气泡矩形。
        .popup;

        // 缓存读取必须与新表面下的绘制几何一致。
        assert_eq!(popconfirm.absolute_popup_rect(frame), expected);
    }
}

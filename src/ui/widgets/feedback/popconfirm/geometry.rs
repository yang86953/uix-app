//! 气泡确认框行为与几何。

use super::*;

// 复用浮层共享定位机制：方向候选、翻转、溢出评分与表面钳制的单一实现。
use crate::ui::widgets::overlay::{
    OverlayArrowVisual, OverlayBubbleGeometry, OverlayPlacement, draw_overlay_arrow,
    normalize_rect, resolve_overlay_bubble,
};

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
        // 返回同一解析器生成的最终气泡矩形。
        .bubble
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
            .union(&expand_rect(
                self.absolute_popup_rect(frame),
                self.visual.layout.shadow_expand,
            ))
            .intersect(&self.surface_or_fallback(frame))
            .unwrap_or_default()
    }

    pub(super) fn normalize_frame(frame: Rect) -> Rect {
        // 复用浮层共享归一化：非有限坐标回退原点、尺寸收敛为有限非负。
        normalize_rect(frame)
    }
}

// 保存确认气泡经过翻转与表面约束后的最终几何。
// 复用共享气泡几何：最终矩形与实际方向绑定。
pub(crate) type PopconfirmGeometry = OverlayBubbleGeometry<PopconfirmPlacement>;

pub(super) fn resolve_popconfirm_geometry(
    trigger: Rect,
    surface: Rect,
    placement: PopconfirmPlacement,
    arrow: bool,
    preferred_width: f32,
    preferred_height: f32,
) -> PopconfirmGeometry {
    let layout = &POPCONFIRM_VISUAL_REF.layout;
    // 箭头开启时使用箭头间距，否则使用无箭头间距。
    let gap = if arrow {
        layout.arrow_gap
    } else {
        layout.plain_gap
    };
    // 复用共享气泡定位解析：尺寸收敛、翻转与钳制在单一实现内完成。
    // 确认气泡只声明垂直变体，中心比率不参与其正交对齐。
    resolve_overlay_bubble(
        placement,
        trigger,
        surface,
        preferred_width,
        preferred_height,
        gap,
        0.5,
    )
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

pub(super) fn draw_popconfirm_arrow(
    ctx: &mut PaintContext,
    trigger: Rect,
    popup: Rect,
    placement: PopconfirmPlacement,
    color: Color,
) {
    let layout = POPCONFIRM_VISUAL_REF.layout;
    // 复用共享箭头绘制：确认气泡箭头不嵌入气泡边缘、尖端恒居中。
    draw_overlay_arrow(
        ctx,
        trigger,
        popup,
        placement.decompose().0,
        color,
        OverlayArrowVisual {
            size: layout.arrow_size,
            edge_overlap: 0.0,
            tip_ratio: 0.5,
            center_ratio: 0.5,
        },
    );
}
